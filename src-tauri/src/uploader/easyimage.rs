use super::{do_json, find_url, http_client, trim_cfg, ImageFile, UploadResult};

pub async fn upload_easy_image(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let api_url = trim_cfg(cfg, "api_url");
    let token = trim_cfg(cfg, "token");
    if api_url.is_empty() || token.is_empty() {
        return Err("EasyImage 配置缺少 API 地址或 Token".to_string());
    }
    let part = reqwest::multipart::Part::bytes(image.data.clone())
        .file_name(image.filename.clone())
        .mime_str(&image.content_type)
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new().text("token", token).part("image", part);
    let response = do_json(
        http_client().post(&api_url).multipart(form),
        true,
    )
    .await?
    .ok_or_else(|| "EasyImage 上传响应缺少图片地址".to_string())?;

    let uploaded_url = find_url(&response);
    if !uploaded_url.is_empty() {
        return Ok(UploadResult { url: uploaded_url });
    }
    Err("EasyImage 上传响应缺少图片地址".to_string())
}
