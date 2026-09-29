mod easyimage;
mod gitee;
mod github;
mod s3;
mod upyun;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use once_cell::sync::Lazy;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use regex::Captures;
use serde_json::Value;
use sha1::{Digest as Sha1Digest, Sha1};
use std::time::Duration;

pub const DEFAULT_FILENAME_FORMAT: &str = "{y}/{m}/{d}/{origin}{ext}";

const MAX_IMAGE_SIZE: usize = 20 << 20;

static RAND_TOKEN_PATTERN: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"\{rand:(\d+)\}").unwrap());

const PATH_SEGMENT_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~')
    .remove(b'$')
    .remove(b'&')
    .remove(b'+')
    .remove(b',')
    .remove(b';')
    .remove(b':')
    .remove(b'=')
    .remove(b'?')
    .remove(b'@');

#[derive(Debug, Clone)]
pub struct ImageFile {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct UploadResult {
    pub url: String,
}

pub fn http_client() -> &'static reqwest::Client {
    static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
        reqwest::Client::builder()
            .user_agent("picbed-switcher")
            .build()
            .expect("failed to build http client")
    });
    &CLIENT
}

pub async fn download_image(raw_url: &str) -> Result<ImageFile, String> {
    let trimmed = raw_url.trim();
    let parsed = url::Url::parse(trimmed).map_err(|_| "图片地址不正确".to_string())?;
    if parsed.host_str().unwrap_or("").is_empty() {
        return Err("图片地址不正确".to_string());
    }
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err("图片地址仅支持 HTTP 或 HTTPS".to_string());
    }

    let resp = http_client()
        .get(parsed.clone())
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("图片下载失败：{}", resp.status()));
    }
    let content_type_header = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    let data = read_limited_body(resp).await?;
    if data.len() > MAX_IMAGE_SIZE {
        return Err("图片大小不能超过 20MB".to_string());
    }

    let content_type = if content_type_header.is_empty() {
        detect_content_type(&data, "")
    } else {
        content_type_header
    };
    if !content_type.starts_with("image/") {
        return Err("该地址不是有效图片".to_string());
    }

    let filename = build_filename(&parsed, &content_type, &data);
    Ok(ImageFile { filename, content_type, data })
}

async fn read_limited_body(resp: reqwest::Response) -> Result<Vec<u8>, String> {
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_IMAGE_SIZE + 1 {
            return Err("图片大小不能超过 20MB".to_string());
        }
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > MAX_IMAGE_SIZE + 1 {
        return Err("图片大小不能超过 20MB".to_string());
    }
    Ok(bytes.to_vec())
}

pub fn new_image_file(filename: &str, data: Vec<u8>) -> Result<ImageFile, String> {
    if data.len() > MAX_IMAGE_SIZE {
        return Err("图片大小不能超过 20MB".to_string());
    }
    let content_type = detect_content_type(&data, filename);
    if !content_type.starts_with("image/") {
        return Err("上传文件不是有效图片".to_string());
    }
    Ok(ImageFile {
        filename: build_local_filename(filename, &content_type),
        content_type,
        data,
    })
}

pub fn detect_content_type(data: &[u8], filename: &str) -> String {
    let trimmed = trim_bom(data);
    let lowered_head: Vec<u8> = trimmed.iter().take(256).map(|b| b.to_ascii_lowercase()).collect();
    let head = String::from_utf8_lossy(&lowered_head);
    if head.trim_start().starts_with("<svg") || (head.contains("<?xml") && head.contains("<svg")) {
        return "image/svg+xml".to_string();
    }
    if let Some(kind) = infer::get(data) {
        return kind.mime_type().to_string();
    }
    let ext = file_extension(filename).to_lowercase();
    return match ext.as_str() {
        ".svg" => "image/svg+xml".to_string(),
        ".png" => "image/png".to_string(),
        ".jpg" | ".jpeg" => "image/jpeg".to_string(),
        ".gif" => "image/gif".to_string(),
        ".webp" => "image/webp".to_string(),
        ".bmp" => "image/bmp".to_string(),
        ".avif" => "image/avif".to_string(),
        ".ico" => "image/x-icon".to_string(),
        _ => "application/octet-stream".to_string(),
    };
}

fn trim_bom(data: &[u8]) -> &[u8] {
    data.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(data)
}

pub async fn upload(picbed_type: &str, cfg: &serde_json::Map<String, Value>, image: &mut ImageFile) -> Result<UploadResult, String> {
    if picbed_type != "easyimage" {
        image.filename = format_filename(cfg.get("filename_format").and_then(|v| v.as_str()).unwrap_or(""), image);
    }
    match picbed_type {
        "github" => github::upload_github(cfg, image).await,
        "gitee" => gitee::upload_gitee(cfg, image).await,
        "tencent" => s3::upload_tencent_cos(cfg, image).await,
        "aliyun" => s3::upload_aliyun_oss(cfg, image).await,
        "qiniu" => s3::upload_qiniu_kodo(cfg, image).await,
        "baidu_bos" => s3::upload_baidu_bos(cfg, image).await,
        "huawei_obs" => s3::upload_huawei_obs(cfg, image).await,
        "upyun" => upyun::upload_upyun(cfg, image).await,
        "minio" => s3::upload_minio(cfg, image).await,
        "easyimage" | "other" => easyimage::upload_easy_image(cfg, image).await,
        other => Err(format!("暂不支持上传到{}", other)),
    }
}

pub async fn test_config(picbed_type: &str, cfg: &serde_json::Map<String, Value>) -> Result<(), String> {
    let mut image = ImageFile {
        filename: "picbed-switcher-test.png".to_string(),
        content_type: "image/png".to_string(),
        data: vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49,
            0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00,
            0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ],
    };
    upload(picbed_type, cfg, &mut image).await.map(|_| ())
}

pub fn cfg_str<'a>(cfg: &'a serde_json::Map<String, Value>, key: &str) -> &'a str {
    cfg.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

pub fn trim_cfg(cfg: &serde_json::Map<String, Value>, key: &str) -> String {
    cfg_str(cfg, key).trim().to_string()
}

pub fn format_filename(format: &str, image: &ImageFile) -> String {
    let mut format = format.trim().to_string();
    if format.is_empty() {
        format = DEFAULT_FILENAME_FORMAT.to_string();
    }
    let now = chrono::Local::now();
    let ext = file_extension(&image.filename);
    let base = basename(&image.filename);
    let mut origin = base.trim_end_matches(&ext).to_string();
    if origin.is_empty() || origin == "." || origin == "/" {
        origin = "image".to_string();
    }
    let mut hasher = Sha1::new();
    hasher.update(&image.data);
    let hash_value = hex::encode(hasher.finalize());

    format = RAND_TOKEN_PATTERN
        .replace_all(&format, |caps: &Captures| {
            let length: usize = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            random_string(length)
        })
        .to_string();

    let replacements: Vec<(&str, String)> = vec![
        ("{timestamp}", now.timestamp().to_string()),
        ("{y}", now.format("%Y").to_string()),
        ("{m}", now.format("%m").to_string()),
        ("{d}", now.format("%d").to_string()),
        ("{hash}", hash_value),
        ("{origin}", origin.clone()),
        ("{random}", random_string(8)),
        ("{ext}", ext),
        ("{name}", origin),
        ("{filename}", base),
    ];
    let mut filename = format;
    for (token, value) in replacements {
        filename = filename.replace(token, &value);
    }
    let filename = sanitize_object_name(&filename);
    if filename.is_empty() {
        return image.filename.clone();
    }
    filename
}

fn random_string(length: usize) -> String {
    if length == 0 {
        return String::new();
    }
    use rand::RngCore;
    let mut buf = vec![0u8; (length + 1) / 2];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    hex::encode(buf)[..length].to_string()
}

pub fn sanitize_object_name(value: &str) -> String {
    let segments: Vec<String> = value
        .split(|c| c == '/' || c == '\\')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let cleaned = crate::markdown::sanitize_safe_filename(segment);
            cleaned.trim_matches(|c| c == '.' || c == '-').to_string()
        })
        .filter(|segment| !segment.is_empty() && segment != "." && segment != "..")
        .collect();
    segments.join("/")
}

pub fn object_path(storage_path: &str, filename: &str) -> String {
    let joined = format!("/{}/{}", storage_path.trim(), filename);
    joined.trim_matches('/').to_string()
}

pub fn custom_public_url(custom_domain: &str, object_path: &str) -> String {
    let custom_domain = custom_domain.trim();
    if custom_domain.is_empty() {
        return String::new();
    }
    join_url(custom_domain, &[object_path])
}

pub fn join_url(base: &str, parts: &[&str]) -> String {
    let base = base.trim().trim_end_matches('/').to_string();
    let mut clean_parts: Vec<String> = Vec::new();
    for part in parts {
        let cleaned = clean_url_path(part);
        if !cleaned.is_empty() && cleaned != "." {
            clean_parts.push(cleaned);
        }
    }
    if clean_parts.is_empty() {
        return base;
    }
    format!("{}/{}", base, clean_parts.join("/"))
}

pub fn clean_url_path(value: &str) -> String {
    let joined = format!("/{}", value.trim());
    let mut segments: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    segments.join("/")
}

fn build_filename(parsed: &url::Url, content_type: &str, _data: &[u8]) -> String {
    let base_raw = parsed
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .unwrap_or("");
    let mut base = base_raw.to_string();
    if base.is_empty() || base == "." || base == "/" {
        base = "image".to_string();
    }
    base = crate::markdown::sanitize_safe_filename(&base);
    base = base.trim_matches(|c| c == '.' || c == '-').to_string();
    if base.is_empty() {
        base = "image".to_string();
    }
    if file_extension(&base).is_empty() {
        base.push_str(mime_extension(content_type));
    }
    base
}

fn build_local_filename(filename: &str, content_type: &str) -> String {
    let normalized = filename.trim().replace('\\', "/");
    let mut base = basename(&normalized);
    if base.is_empty() || base == "." || base == "/" {
        base = "image".to_string();
    }
    base = crate::markdown::sanitize_safe_filename(&base);
    base = base.trim_matches(|c| c == '.' || c == '-').to_string();
    if base.is_empty() {
        base = "image".to_string();
    }
    if file_extension(&base).is_empty() {
        base.push_str(mime_extension(content_type));
    }
    base
}

fn basename(value: &str) -> String {
    let trimmed = value.trim_end_matches('/');
    match trimmed.rsplit('/').next() {
        Some(base) => base.to_string(),
        None => trimmed.to_string(),
    }
}

fn file_extension(value: &str) -> String {
    let base = basename(value);
    match base.rfind('.') {
        Some(index) => base[index..].to_string(),
        None => String::new(),
    }
}

fn mime_extension(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => ".png",
        "image/jpeg" | "image/jpg" => ".jpg",
        "image/gif" => ".gif",
        "image/webp" => ".webp",
        "image/bmp" => ".bmp",
        "image/svg+xml" => ".svg",
        "image/avif" => ".avif",
        "image/tiff" => ".tiff",
        "image/x-icon" | "image/vnd.microsoft.icon" => ".ico",
        _ => "",
    }
}

pub fn path_escape_segment(segment: &str) -> String {
    utf8_percent_encode(segment, PATH_SEGMENT_SET).to_string()
}

pub async fn do_json(req: reqwest::RequestBuilder, output_required: bool) -> Result<Option<Value>, String> {
    let resp = req
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    let bytes = if bytes.len() > 4 << 20 { &bytes[..4 << 20] } else { &bytes[..] };
    if !status.is_success() {
        let body = String::from_utf8_lossy(bytes);
        return Err(format!("上传失败：{}", body.trim()));
    }
    if bytes.is_empty() {
        if output_required {
            return Err("上传响应内容为空".to_string());
        }
        return Ok(None);
    }
    serde_json::from_slice(bytes).map(Some).map_err(|e| e.to_string())
}

pub fn find_url(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            for key in ["url", "src", "path", "image", "links"] {
                if let Some(found) = map.get(key) {
                    let found = find_url(found);
                    if !found.is_empty() {
                        return found;
                    }
                }
            }
            for (_key, nested) in map {
                let found = find_url(nested);
                if !found.is_empty() {
                    return found;
                }
            }
            String::new()
        }
        Value::Array(items) => {
            for nested in items {
                let found = find_url(nested);
                if !found.is_empty() {
                    return found;
                }
            }
            String::new()
        }
        Value::String(text) => {
            if text.starts_with("http://") || text.starts_with("https://") {
                text.clone()
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

pub fn encode_base64(data: &[u8]) -> String {
    BASE64.encode(data)
}
