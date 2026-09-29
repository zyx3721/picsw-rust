use super::{cfg_str, do_json, encode_base64, http_client, join_url, object_path, path_escape_segment, trim_cfg, ImageFile, UploadResult};

pub async fn upload_gitee(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let repo = trim_cfg(cfg, "repository").trim_matches('/').to_string();
    let branch = trim_cfg(cfg, "branch");
    let token = trim_cfg(cfg, "token");
    if repo.is_empty() || branch.is_empty() || token.is_empty() {
        return Err("Gitee 配置缺少仓库、分支或 Token".to_string());
    }
    let object = object_path(cfg_str(cfg, "storage_path"), &image.filename);
    let body = serde_json::json!({
        "access_token": token,
        "message": format!("upload {}", image.filename),
        "content": encode_base64(&image.data),
        "branch": branch,
    });
    let api_url = format!(
        "https://gitee.com/api/v5/repos/{}/contents/{}",
        repo,
        path_escape_segment(&object).replace("%2F", "/")
    );
    let response = do_json(
        http_client().post(&api_url).header("Content-Type", "application/json").json(&body),
        true,
    )
    .await?
    .ok_or_else(|| "Gitee 上传响应缺少图片地址".to_string())?;

    let download_url = response
        .pointer("/content/download_url")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    if !download_url.is_empty() {
        return Ok(UploadResult { url: download_url });
    }
    Ok(UploadResult {
        url: join_url("https://gitee.com", &[&repo, "raw", &branch, &object]),
    })
}
