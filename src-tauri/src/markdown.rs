use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use std::cell::RefCell;

pub static DEFAULT_FILENAME_FORMAT: &str = "{y}/{m}/{d}/{origin}{ext}";

static IMAGE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"!\[([^\]]*)\]\(([^\s)]+)(?:\s+"[^"]*")?\)|<img[^>]+src=["']([^"']+)["'][^>]*>"#)
        .expect("invalid image pattern")
});

static SAFE_FILENAME_CHARS: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-zA-Z0-9._-]+").unwrap());

#[derive(Debug, Clone, serde::Serialize)]
pub struct MarkdownImage {
    pub raw: String,
    pub url: String,
    pub alt: String,
    pub picbed: String,
}

pub fn extract_markdown_images(content: &str) -> Vec<MarkdownImage> {
    IMAGE_PATTERN
        .captures_iter(content)
        .map(|caps| {
            let alt = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let markdown_url = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let html_url = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            let url = if !markdown_url.is_empty() { markdown_url } else { html_url };
            MarkdownImage {
                raw: caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string(),
                url: url.to_string(),
                alt: alt.to_string(),
                picbed: detect_pic_bed(url),
            }
        })
        .collect()
}

pub fn detect_pic_bed(raw: &str) -> String {
    let (host, path, query) = loose_parse(raw.trim());
    let detected = detect_pic_bed_by_host_and_path(&host, &path);
    if detected != "other" {
        return detected;
    }
    for value in [&path, &query] {
        let embedded = first_embedded_url(value);
        if !embedded.is_empty() {
            return detect_pic_bed(&embedded);
        }
    }
    "other".to_string()
}

fn detect_pic_bed_by_host_and_path(host_value: &str, path_value: &str) -> String {
    let host = host_value.to_lowercase();
    let path = path_value.to_lowercase();
    if host.contains("githubusercontent.com") || host.contains("github.com") {
        "github".to_string()
    } else if host.contains("gitee.com") {
        "gitee".to_string()
    } else if host.contains("myqcloud.com") || host.contains("tencent") {
        "tencent".to_string()
    } else if host.contains("aliyuncs.com") || host.contains("aliyun") {
        "aliyun".to_string()
    } else if host.contains("qiniucdn.com")
        || host.contains("qiniucs.com")
        || host.contains("clouddn.com")
        || host.contains("qiniu")
    {
        "qiniu".to_string()
    } else if host.contains("bcebos.com") || host.contains("baidubce.com") {
        "baidu_bos".to_string()
    } else if host.contains("myhuaweicloud.com") || host.contains("huaweicloud") {
        "huawei_obs".to_string()
    } else if host.contains("upaiyun.com") || host.contains("upyun") {
        "upyun".to_string()
    } else if host.contains("minio") {
        "minio".to_string()
    } else if host.contains("easyimage") || path.contains("easyimage") || path.contains("/i/") {
        "easyimage".to_string()
    } else {
        "other".to_string()
    }
}

fn loose_parse(raw: &str) -> (String, String, String) {
    match url::Url::parse(raw) {
        Ok(parsed) => {
            let host = match parsed.host_str() {
                Some(host) => match parsed.port() {
                    Some(port) => format!("{}:{}", host, port),
                    None => host.to_string(),
                },
                None => String::new(),
            };
            (host, parsed.path().to_string(), parsed.query().unwrap_or("").to_string())
        }
        Err(_) => (String::new(), raw.to_string(), String::new()),
    }
}

fn first_embedded_url(value: &str) -> String {
    let mut current = value.to_string();
    for _ in 0..3 {
        let embedded = first_http_url(&current);
        if !embedded.is_empty() {
            return embedded;
        }
        let unescaped = percent_encoding::percent_decode_str(&current).decode_utf8_lossy().to_string();
        if unescaped == current {
            break;
        }
        current = unescaped;
    }
    String::new()
}

fn first_http_url(value: &str) -> String {
    let lower = value.to_lowercase();
    let mut start: Option<usize> = None;
    for marker in ["https://", "http://"] {
        if let Some(index) = lower.find(marker) {
            if start.is_none() || index < start.unwrap() {
                start = Some(index);
            }
        }
    }
    let Some(start) = start else { return String::new() };
    let candidate = &value[start..];
    let candidate = match candidate.find(|c: char| c == ' ' || c == '\t' || c == '\r' || c == '\n' || c == '"' || c == '\'' || c == '<' || c == '>' || c == ')' || c == '&') {
        Some(end) => &candidate[..end],
        None => candidate,
    };
    candidate.trim().to_string()
}

pub fn is_http_image_url(raw: &str) -> bool {
    match url::Url::parse(raw.trim()) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            (scheme == "http" || scheme == "https") && parsed.host_str().is_some()
        }
        Err(_) => false,
    }
}

pub fn normalize_local_image_source(raw: &str) -> String {
    let mut value = raw.trim().to_string();
    if let Ok(decoded) = urlencoding_decode(&value) {
        value = decoded;
    }
    if let Some(stripped) = value.strip_prefix("file:///") {
        value = stripped.to_string();
    } else if let Some(stripped) = value.strip_prefix("file://") {
        value = stripped.to_string();
    }
    value = value.replace('\\', "/");
    while let Some(stripped) = value.strip_prefix("./") {
        value = stripped.to_string();
    }
    value.trim_matches('/').to_lowercase()
}

fn urlencoding_decode(value: &str) -> Result<String, ()> {
    Ok(percent_encoding::percent_decode_str(value).decode_utf8_lossy().to_string())
}

/// 替换图片地址；返回 None 的图片保留原地址并继续处理，避免单张失败中断整篇文档
pub fn replace_image_urls<F>(content: &str, mut replace: F) -> (String, usize)
where
    F: FnMut(&str) -> Option<String>,
{
    let changed = RefCell::new(0usize);
    let result = IMAGE_PATTERN.replace_all(content, |caps: &Captures| {
        let raw = caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string();
        let Some(current_url) = capture_url(caps) else {
            return raw;
        };
        match replace(&current_url) {
            Some(new_url) if !new_url.trim().is_empty() && new_url != current_url => {
                *changed.borrow_mut() += 1;
                raw.replacen(&current_url, &new_url, 1)
            }
            _ => raw,
        }
    });
    (result.into_owned(), changed.into_inner())
}

fn capture_url(caps: &Captures) -> Option<String> {
    let markdown_url = caps.get(2).map(|m| m.as_str()).unwrap_or("");
    let html_url = caps.get(3).map(|m| m.as_str()).unwrap_or("");
    if !markdown_url.is_empty() {
        Some(markdown_url.to_string())
    } else if !html_url.is_empty() {
        Some(html_url.to_string())
    } else {
        None
    }
}

pub fn sanitize_safe_filename(value: &str) -> String {
    SAFE_FILENAME_CHARS.replace_all(value, "-").to_string()
}

#[cfg(test)]
mod tests {
    use super::replace_image_urls;

    #[test]
    fn keeps_unreplaced_images_and_continues() {
        let content = "![a](https://a.com/1.png)\n![b](https://a.com/2.png)";
        let (output, changed) = replace_image_urls(content, |url| {
            if url.ends_with("1.png") {
                Some("https://b.com/1.png".to_string())
            } else {
                None
            }
        });
        assert_eq!(changed, 1);
        assert!(output.contains("https://b.com/1.png"));
        assert!(output.contains("https://a.com/2.png"));
    }
}
