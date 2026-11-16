use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use once_cell::sync::Lazy;
use sha1::{Digest, Sha1};
use tokio::sync::{Mutex, OwnedMutexGuard};

use super::{cfg_str, do_json, encode_base64, http_client, join_url, object_path, path_escape_segment, trim_cfg, ImageFile, UploadResult};

const CREATE_ATTEMPTS: usize = 3;
const RETRY_BASE_DELAY_MS: u64 = 300;

// 同一仓库分支的写操作串行化，避免并发建文件返回「文件新建失败」
static WRITE_LOCKS: Lazy<std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>> = Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

// Gitee 目标路径的查询结果
enum GiteeObject {
    Missing,
    File { sha: String, download_url: String },
    Directory,
    Unknown,
}

pub async fn upload_gitee(cfg: &serde_json::Map<String, serde_json::Value>, image: &ImageFile) -> Result<UploadResult, String> {
    let repo = trim_cfg(cfg, "repository").trim_matches('/').to_string();
    let branch = trim_cfg(cfg, "branch");
    let token = trim_cfg(cfg, "token");
    if repo.is_empty() || branch.is_empty() || token.is_empty() {
        return Err("Gitee 配置缺少仓库、分支或 Token".to_string());
    }
    let object = object_path(cfg_str(cfg, "storage_path"), &image.filename);
    let _guard = write_guard(&format!("{}/{}", repo, branch)).await;
    let blob_sha = git_blob_sha1(&image.data);
    match lookup_object(&repo, &branch, &token, &object).await {
        GiteeObject::File { sha, download_url } if !sha.is_empty() => {
            if sha == blob_sha {
                return Ok(UploadResult { url: prefer_download_url(download_url, &repo, &branch, &object) });
            }
            return Err(collision_error(&object));
        }
        GiteeObject::Directory => return Err(directory_error(&object)),
        _ => {}
    }

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
    for attempt in 0..CREATE_ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(RETRY_BASE_DELAY_MS * attempt as u64)).await;
        }
        match create_file(&api_url, &body).await {
            Ok(response) => {
                let download_url = response
                    .pointer("/content/download_url")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                return Ok(UploadResult { url: prefer_download_url(download_url, &repo, &branch, &object) });
            }
            Err(err) if is_duplicate_error(&err) => {
                return resolve_duplicate(&repo, &branch, &token, &object, &blob_sha, err).await;
            }
            Err(err) if is_transient_create_error(&err) => {
                if attempt + 1 == CREATE_ATTEMPTS {
                    return Err(format!("{}（已自动重试 {} 次）", err, CREATE_ATTEMPTS - 1));
                }
            }
            Err(err) => return Err(err),
        }
    }
    Err("Gitee 上传失败".to_string())
}

async fn create_file(api_url: &str, body: &serde_json::Value) -> Result<serde_json::Value, String> {
    do_json(http_client().post(api_url).header("Content-Type", "application/json").json(body), true)
        .await?
        .ok_or_else(|| "Gitee 上传响应缺少图片地址".to_string())
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
        GiteeObject::File { sha, download_url } if !sha.is_empty() => {
            if sha == blob_sha {
                Ok(UploadResult { url: prefer_download_url(download_url, repo, branch, object) })
            } else {
                Err(collision_error(object))
            }
        }
        GiteeObject::Directory => Err(directory_error(object)),
        _ => Err(original_error),
    }
}

async fn lookup_object(repo: &str, branch: &str, token: &str, object: &str) -> GiteeObject {
    let escaped = path_escape_segment(object).replace("%2F", "/");
    let Ok(mut url) = url::Url::parse(&format!("https://gitee.com/api/v5/repos/{}/contents/{}", repo, escaped)) else {
        return GiteeObject::Unknown;
    };
    url.query_pairs_mut().append_pair("access_token", token).append_pair("ref", branch);
    let Ok(response) = http_client().get(url).timeout(Duration::from_secs(30)).send().await else {
        return GiteeObject::Unknown;
    };
    match response.status().as_u16() {
        200 => {
            let Ok(value) = response.json::<serde_json::Value>().await else {
                return GiteeObject::Unknown;
            };
            if value.is_array() {
                return GiteeObject::Directory;
            }
            let sha = value.get("sha").and_then(|item| item.as_str()).unwrap_or("").to_string();
            let download_url = value.get("download_url").and_then(|item| item.as_str()).unwrap_or("").to_string();
            GiteeObject::File { sha, download_url }
        }
        404 => GiteeObject::Missing,
        _ => GiteeObject::Unknown,
    }
}

async fn write_guard(key: &str) -> OwnedMutexGuard<()> {
    let lock = {
        let mut locks = WRITE_LOCKS.lock().expect("gitee write lock poisoned");
        locks.entry(key.to_string()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
    };
    lock.lock_owned().await
}

fn prefer_download_url(download_url: String, repo: &str, branch: &str, object: &str) -> String {
    if download_url.is_empty() {
        join_url("https://gitee.com", &[repo, "raw", branch, object])
    } else {
        download_url
    }
}

fn collision_error(object: &str) -> String {
    format!("Gitee 目标文件已存在且内容不同：{}，请调整文件命名格式或存储路径", object)
}

fn directory_error(object: &str) -> String {
    format!("Gitee 目标路径已被同名目录占用：{}，请更换存储路径或文件命名格式", object)
}

// 计算与 Git 一致的 blob SHA1，用于判断已存在文件与待上传内容是否一致
fn git_blob_sha1(data: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", data.len()).as_bytes());
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn is_duplicate_error(message: &str) -> bool {
    message.contains("文件名已存在")
        || message.contains("文件已存在")
        || message.contains("文件已经存在")
        || message.to_lowercase().contains("already exists")
}

fn is_transient_create_error(message: &str) -> bool {
    message.contains("文件新建失败")
        || message.contains("系统繁忙")
        || message.contains("服务不可用")
        || message.contains("稍后再试")
        || message.contains("timed out")
        || message.contains("error sending request")
}

#[cfg(test)]
mod tests {
    use super::{git_blob_sha1, is_duplicate_error, is_transient_create_error};

    #[test]
    fn git_blob_sha1_matches_git() {
        assert_eq!(git_blob_sha1(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
        assert_eq!(git_blob_sha1(b"hello world\n"), "3b18e512dba79e4c8300dd08aeb37f8e728b8dad");
    }

    #[test]
    fn duplicate_errors_are_detected() {
        assert!(is_duplicate_error("上传失败：{\"message\":\"文件名已存在\"}"));
        assert!(is_duplicate_error("上传失败：{\"message\":\"文件已经存在\"}"));
        assert!(is_duplicate_error("A file with this name already exists"));
        assert!(!is_duplicate_error("上传失败：{\"message\":\"文件新建失败\"}"));
    }

    #[test]
    fn create_conflicts_are_retryable() {
        assert!(is_transient_create_error("上传失败：{\"message\":\"文件新建失败\"}"));
        assert!(!is_transient_create_error("上传失败：{\"message\":\"文件名已存在\"}"));
    }
}
