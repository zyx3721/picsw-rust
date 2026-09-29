use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use std::time::Duration;

use super::{cfg_str, custom_public_url, http_client, object_path, trim_cfg, ImageFile, UploadResult};

pub async fn upload_upyun(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let bucket = trim_cfg(cfg, "bucket");
    let operator = trim_cfg(cfg, "operator");
    let password = trim_cfg(cfg, "password");
    if bucket.is_empty() || operator.is_empty() || password.is_empty() {
        return Err("又拍云 USS 配置缺少服务名、操作员或密码".to_string());
    }
    let object = object_path(cfg_str(cfg, "storage_path"), &image.filename);
    let upload_url = super::join_url("https://v0.api.upyun.com", &[&bucket, &object]);
    let credentials = BASE64.encode(format!("{}:{}", operator, password));
    let resp = http_client()
        .put(&upload_url)
        .header("Authorization", format!("Basic {}", credentials))
        .header("Content-Type", &image.content_type)
        .header("Content-Length", image.data.len().to_string())
        .body(image.data.clone())
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let body = body.trim();
        let truncated: String = if body.chars().count() > 512 {
            body.chars().take(512).collect()
        } else {
            body.to_string()
        };
        return Err(format!("上传失败：{}", truncated));
    }
    let custom = custom_public_url(cfg_str(cfg, "custom_domain"), &object);
    if !custom.is_empty() {
        return Ok(UploadResult { url: custom });
    }
    Err("又拍云 USS 配置缺少加速域名，无法生成公开访问地址".to_string())
}
