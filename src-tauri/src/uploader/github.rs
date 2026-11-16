use std::time::Duration;

use super::repo::{git_blob_sha1, write_guard};
use super::{cfg_str, encode_base64, http_client, join_url, object_path, path_escape_segment, trim_cfg, ImageFile, UploadResult};

const CREATE_ATTEMPTS: usize = 3;
const RETRY_BASE_DELAY_MS: u64 = 300;
const API_VERSION: &str = "2022-11-28";

struct GithubError {
    status: u16,
    message: String,
    transport: bool,
}

// GitHub 目标路径的查询结果
enum GithubObject {
    Missing,
    File { sha: String, download_url: String },
    Directory,
    Unknown,
}

pub async fn upload_github(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let repo = trim_cfg(cfg, "repository").trim_matches('/').to_string();
    let branch = trim_cfg(cfg, "branch");
    let token = trim_cfg(cfg, "token");
    if repo.is_empty() || branch.is_empty() || token.is_empty() {
        return Err("GitHub 配置缺少仓库、分支或 Token".to_string());
    }
    let object = object_path(cfg_str(cfg, "storage_path"), &image.filename);
    let _guard = write_guard("github", &format!("{}/{}", repo, branch)).await;
    let blob_sha = git_blob_sha1(&image.data);
    let body = serde_json::json!({
        "message": format!("upload {}", image.filename),
        "content": encode_base64(&image.data),
        "branch": branch,
    });
    let api_url = format!(
        "https://api.github.com/repos/{}/contents/{}",
        repo,
        path_escape_segment(&object).replace("%2F", "/")
    );
    for attempt in 0..CREATE_ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(RETRY_BASE_DELAY_MS * attempt as u64)).await;
        }
        match create_file(&api_url, &token, &body).await {
            Ok(response) => {
                let download_url = response
                    .pointer("/content/download_url")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                if download_url.is_empty() {
                    return Err("GitHub 上传响应缺少图片地址".to_string());
                }
                return Ok(UploadResult { url: download_url });
            }
            Err(err) if is_duplicate_error(err.status, &err.message) => {
                return resolve_duplicate(&repo, &branch, &token, &object, &blob_sha, err.message).await;
            }
            Err(err) if is_transient_error(err.status, &err.message, err.transport) => {
                if attempt + 1 == CREATE_ATTEMPTS {
                    return Err(format!("{}（已自动重试 {} 次）", err.message, CREATE_ATTEMPTS - 1));
                }
            }
            Err(err) => return Err(err.message),
        }
    }
    Err("GitHub 上传失败".to_string())
}

async fn create_file(api_url: &str, token: &str, body: &serde_json::Value) -> Result<serde_json::Value, GithubError> {
    let response = http_client()
        .put(api_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .header("Content-Type", "application/json")
        .json(body)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|err| GithubError { status: 0, message: err.to_string(), transport: true })?;
    let status = response.status().as_u16();
    let bytes = response
        .bytes()
        .await
        .map_err(|err| GithubError { status, message: err.to_string(), transport: true })?;
    if !(200..300).contains(&status) {
        let body = String::from_utf8_lossy(&bytes);
        return Err(GithubError { status, message: format!("上传失败：{}", body.trim()), transport: false });
    }
    if bytes.is_empty() {
        return Err(GithubError { status, message: "上传响应内容为空".to_string(), transport: false });
    }
    serde_json::from_slice(&bytes).map_err(|err| GithubError { status, message: err.to_string(), transport: false })
}

async fn resolve_duplicate(
    repo: &str,
    branch: &str,
    token: &str,
    object: &str,
    blob_sha: &str,
    original_error: String,
) -> Result<UploadResult, String> {
    match lookup_object(repo, branch, token, object).await {
        GithubObject::File { sha, download_url } if !sha.is_empty() => {
            if sha == blob_sha {
                Ok(UploadResult { url: prefer_download_url(download_url, repo, branch, object) })
            } else {
                Err(collision_error(object))
            }
        }
        GithubObject::Directory => Err(directory_error(object)),
        _ => Err(original_error),
    }
}

async fn lookup_object(repo: &str, branch: &str, token: &str, object: &str) -> GithubObject {
    let escaped = path_escape_segment(object).replace("%2F", "/");
    let Ok(mut url) = url::Url::parse(&format!("https://api.github.com/repos/{}/contents/{}", repo, escaped)) else {
        return GithubObject::Unknown;
    };
    url.query_pairs_mut().append_pair("ref", branch);
    let Ok(response) = http_client()
        .get(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .timeout(Duration::from_secs(30))
        .send()
        .await
    else {
        return GithubObject::Unknown;
    };
    match response.status().as_u16() {
        200 => {
            let Ok(value) = response.json::<serde_json::Value>().await else {
                return GithubObject::Unknown;
            };
            if value.is_array() {
                return GithubObject::Directory;
            }
            let sha = value.get("sha").and_then(|item| item.as_str()).unwrap_or("").to_string();
            let download_url = value.get("download_url").and_then(|item| item.as_str()).unwrap_or("").to_string();
            GithubObject::File { sha, download_url }
        }
        404 => GithubObject::Missing,
        _ => GithubObject::Unknown,
    }
}

fn prefer_download_url(download_url: String, repo: &str, branch: &str, object: &str) -> String {
    if download_url.is_empty() {
        join_url("https://raw.githubusercontent.com", &[repo, branch, object])
    } else {
        download_url
    }
}

fn collision_error(object: &str) -> String {
    format!("GitHub 目标文件已存在且内容不同：{}，请调整文件命名格式或存储路径", object)
}

fn directory_error(object: &str) -> String {
    format!("GitHub 目标路径已被同名目录占用：{}，请更换存储路径或文件命名格式", object)
}

fn is_duplicate_error(status: u16, message: &str) -> bool {
    if message.contains("文件名已存在") || message.contains("文件已存在") || message.contains("文件已经存在") {
        return true;
    }
    if status != 422 {
        return false;
    }
    let lower = message.to_lowercase();
    (lower.contains("sha") && lower.contains("supplied")) || lower.contains("already exists")
}

fn is_transient_error(status: u16, message: &str, transport: bool) -> bool {
    if transport {
        return true;
    }
    if status == 409 {
        return !message.to_lowercase().contains("empty");
    }
    status >= 500
}

#[cfg(test)]
mod tests {
    use super::{is_duplicate_error, is_transient_error};

    #[test]
    fn duplicate_errors_are_detected() {
        assert!(is_duplicate_error(422, "上传失败：{\"message\":\"Invalid request.\\n\\n\\\"sha\\\" wasn't supplied.\"}"));
        assert!(is_duplicate_error(422, "上传失败：{\"message\":\"A file with this name already exists\"}"));
        assert!(!is_duplicate_error(422, "上传失败：{\"message\":\"Invalid request\"}"));
        assert!(!is_duplicate_error(500, "upload failed"));
    }

    #[test]
    fn transient_errors_are_detected() {
        assert!(is_transient_error(0, "timed out", true));
        assert!(is_transient_error(409, "Update is not a fast forward", false));
        assert!(!is_transient_error(409, "Git Repository is empty.", false));
        assert!(is_transient_error(502, "Bad Gateway", false));
        assert!(!is_transient_error(422, "sha wasn't supplied", false));
        assert!(!is_transient_error(403, "rate limit exceeded", false));
    }
}
