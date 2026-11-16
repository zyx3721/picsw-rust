use std::time::Duration;

use super::{find_url, http_client, trim_cfg, ImageFile, UploadResult};

const MAX_EXCERPT_CHARS: usize = 300;
const URL_KEYS: [&str; 8] = ["url", "full_url", "src", "path", "image", "image_url", "imgurl", "links"];

pub async fn upload_easy_image(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let api_url = trim_cfg(cfg, "api_url");
    let token = trim_cfg(cfg, "token");
    if api_url.is_empty() || token.is_empty() {
        return Err("EasyImage 配置缺少 API 地址或 Token".to_string());
    }
    let base = url::Url::parse(&api_url).map_err(|_| "EasyImage API 地址不正确，请填写完整地址".to_string())?;
    if (base.scheme() != "http" && base.scheme() != "https") || base.host_str().unwrap_or("").is_empty() {
        return Err("EasyImage API 地址不正确，请填写完整地址".to_string());
    }
    let part = reqwest::multipart::Part::bytes(image.data.clone())
        .file_name(image.filename.clone())
        .mime_str(&image.content_type)
        .map_err(|err| err.to_string())?;
    let form = reqwest::multipart::Form::new().text("token", token).part("image", part);
    let response = http_client()
        .post(base.as_str())
        .multipart(form)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let bytes = response.bytes().await.map_err(|err| err.to_string())?;
    if status.as_u16() == 404 {
        return Err("EasyImage API 地址不存在，请检查是否填写为 /api/index.php".to_string());
    }
    if !status.is_success() {
        let body = String::from_utf8_lossy(&bytes);
        let body = excerpt(&body);
        if body.is_empty() {
            return Err(format!("上传失败：HTTP {}", status.as_u16()));
        }
        return Err(format!("上传失败：{}", body));
    }
    if bytes.is_empty() {
        return Err("EasyImage 上传响应内容为空".to_string());
    }
    let value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => {
            let body = String::from_utf8_lossy(&bytes);
            return Err(format!("EasyImage 返回内容不是 JSON，请检查 API 地址：{}", excerpt(&body)));
        }
    };
    if is_failed_result(&value) {
        return Err(failure_message(&value));
    }
    if let Some(url) = extract_image_url(&value, &base) {
        return Ok(UploadResult { url });
    }
    if let Some(message) = server_message(&value) {
        return Err(format!("EasyImage 上传失败：{}", message));
    }
    Err(format!("EasyImage 上传响应缺少图片地址：{}", excerpt(&value.to_string())))
}

fn is_failed_result(value: &serde_json::Value) -> bool {
    value
        .get("result")
        .and_then(|item| item.as_str())
        .map(|result| result.eq_ignore_ascii_case("failed") || result.eq_ignore_ascii_case("error"))
        .unwrap_or(false)
}

fn failure_message(value: &serde_json::Value) -> String {
    let message = server_message(value).unwrap_or_else(|| "服务端返回失败".to_string());
    let code = response_code(value).map(|code| format!("（code {}）", code)).unwrap_or_default();
    format!("EasyImage 上传失败：{}{}", message, code)
}

fn server_message(value: &serde_json::Value) -> Option<String> {
    for key in ["message", "msg", "error"] {
        let text = value.get(key).and_then(|item| item.as_str()).unwrap_or("").trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    None
}

fn response_code(value: &serde_json::Value) -> Option<String> {
    match value.get("code") {
        Some(serde_json::Value::Number(number)) => Some(number.to_string()),
        Some(serde_json::Value::String(text)) if !text.trim().is_empty() => Some(text.trim().to_string()),
        _ => None,
    }
}

fn excerpt(value: &str) -> String {
    value.trim().chars().take(MAX_EXCERPT_CHARS).collect()
}

fn extract_image_url(value: &serde_json::Value, base: &url::Url) -> Option<String> {
    if let serde_json::Value::String(text) = value {
        if let Some(url) = normalize_url(text, base) {
            return Some(url);
        }
    }
    find_keyed_url(value, base).or_else(|| {
        let found = find_url(value);
        if found.is_empty() {
            None
        } else {
            Some(found)
        }
    })
}

fn find_keyed_url(value: &serde_json::Value, base: &url::Url) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            for key in URL_KEYS {
                if let Some(found) = map.get(key).and_then(|item| resolve_candidate(item, base)) {
                    return Some(found);
                }
            }
            map.values().find_map(|nested| find_keyed_url(nested, base))
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| find_keyed_url(item, base)),
        _ => None,
    }
}

fn resolve_candidate(value: &serde_json::Value, base: &url::Url) -> Option<String> {
    match value {
        serde_json::Value::String(text) => normalize_url(text, base),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => find_keyed_url(value, base),
        _ => None,
    }
}

fn normalize_url(text: &str, base: &url::Url) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("//") {
        return Some(format!("{}://{}", base.scheme(), rest));
    }
    if trimmed.starts_with('/') {
        return base.join(trimmed).ok().map(|url| url.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{extract_image_url, is_failed_result, server_message};
    use serde_json::json;
    use url::Url;

    fn base() -> Url {
        Url::parse("http://pic.example.com/api/index.php").unwrap()
    }

    #[test]
    fn extracts_absolute_and_relative_urls() {
        let absolute = json!({"result":"success","code":200,"url":"https://cdn.example.com/i/a.png"});
        assert_eq!(extract_image_url(&absolute, &base()).as_deref(), Some("https://cdn.example.com/i/a.png"));
        let root_relative = json!({"result":"success","code":200,"url":"/i/2026/a.png"});
        assert_eq!(extract_image_url(&root_relative, &base()).as_deref(), Some("http://pic.example.com/i/2026/a.png"));
        let scheme_relative = json!({"url":"//cdn.example.com/i/a.png"});
        assert_eq!(extract_image_url(&scheme_relative, &base()).as_deref(), Some("http://cdn.example.com/i/a.png"));
        let nested = json!({"data":{"url":"/i/b.png"}});
        assert_eq!(extract_image_url(&nested, &base()).as_deref(), Some("http://pic.example.com/i/b.png"));
    }

    #[test]
    fn detects_failure_message() {
        let failure = json!({"result":"failed","code":202,"message":"Token Error"});
        assert!(is_failed_result(&failure));
        assert_eq!(server_message(&failure).as_deref(), Some("Token Error"));
    }
}
