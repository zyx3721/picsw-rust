//! 网络图片下载任务创建与下载文件写入辅助

use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;
use tauri::State;

use crate::commands::convert::{insert_task_documents, load_task_json, lock_db};
use crate::engine::{DownloadDocumentSpec, DownloadTaskPayload};
use crate::models::now_string;
use crate::task_queue;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct DownloadDocumentRequest {
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub urls: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDownloadTaskRequest {
    #[serde(default)]
    pub documents: Vec<DownloadDocumentRequest>,
    #[serde(default)]
    pub target_dir: String,
}

/// 创建网络图片下载任务，按文档与图片写入进度行后加入队列
#[tauri::command]
pub async fn create_download_task(state: State<'_, AppState>, req: CreateDownloadTaskRequest) -> Result<serde_json::Value, String> {
    let target_dir = req.target_dir.trim().to_string();
    if target_dir.is_empty() {
        return Err("下载目录不能为空".to_string());
    }
    if !Path::new(&target_dir).is_dir() {
        return Err("下载目录不存在，请重新选择".to_string());
    }
    if req.documents.is_empty() {
        return Err("请至少上传一个 Markdown 文档".to_string());
    }
    if req.documents.len() > 20 {
        return Err("单次最多处理 20 个 Markdown 文档".to_string());
    }

    let mut seen = HashSet::new();
    let mut documents: Vec<(String, Vec<String>)> = Vec::new();
    let mut total_images = 0usize;
    for document in req.documents {
        let mut urls = Vec::new();
        for raw in document.urls {
            let url = raw.trim().to_string();
            if !url.starts_with("http://") && !url.starts_with("https://") {
                continue;
            }
            if !seen.insert(url.clone()) {
                continue;
            }
            urls.push(url);
        }
        if urls.is_empty() {
            continue;
        }
        total_images += urls.len();
        documents.push((document.filename, urls));
    }
    if total_images == 0 {
        return Err("没有可下载的图片地址".to_string());
    }

    let payload = serde_json::to_string(&DownloadTaskPayload {
        documents: documents
            .iter()
            .map(|(filename, urls)| DownloadDocumentSpec {
                filename: filename.clone(),
                urls: urls.clone(),
            })
            .collect(),
        target_dir: target_dir.clone(),
    })
    .map_err(|_| "下载任务载荷生成失败".to_string())?;

    let task_id = {
        let conn = lock_db(&state)?;
        conn.execute(
            "INSERT INTO conversion_tasks (task_type, status, total, success, partial, failed, message, payload, created_at, updated_at) VALUES ('download', 'queued', ?1, 0, 0, 0, '下载任务已加入队列', ?2, ?3, ?4)",
            rusqlite::params![documents.len() as i64, payload, now_string(), now_string()],
        )
        .map_err(|_| "创建下载任务失败".to_string())?;
        let task_id = conn.last_insert_rowid();
        let document_rows: Vec<(String, i64)> = documents
            .iter()
            .map(|(filename, urls)| (filename.clone(), urls.len() as i64))
            .collect();
        insert_task_documents(&conn, task_id, &document_rows).map_err(|_| "创建下载任务失败".to_string())?;
        for (index, (_, urls)) in documents.iter().enumerate() {
            for url in urls {
                conn.execute(
                    "INSERT INTO download_task_images (task_id, document_index, url, status, created_at, updated_at) VALUES (?1, ?2, ?3, 'pending', ?4, ?5)",
                    rusqlite::params![task_id, index as i64, url, now_string(), now_string()],
                )
                .map_err(|_| "创建下载任务失败".to_string())?;
            }
        }
        task_id
    };

    if let Err(err) = task_queue::enqueue(&state, task_id).await {
        let _ = lock_db(&state).map(|conn| {
            conn.execute(
                "UPDATE conversion_tasks SET status = 'failed', message = '下载任务入队失败', error = ?2, ended_at = ?3, updated_at = ?4 WHERE id = ?1",
                rusqlite::params![task_id, err, now_string(), now_string()],
            )
        });
        return Err("下载任务入队失败".to_string());
    }

    let task = {
        let conn = lock_db(&state)?;
        load_task_json(&conn, task_id).ok_or("创建下载任务失败")?
    };
    Ok(serde_json::json!({ "task": task }))
}

/// 先写同目录临时文件再重命名，避免应用退出时留下写了一半的图片
pub(crate) fn write_atomically(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("image.png");
    let temp = path.with_file_name(format!("{file_name}.part"));
    std::fs::write(&temp, data)?;
    match std::fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&temp);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::write_atomically;

    #[test]
    fn writes_file_through_temporary_name() {
        let dir = std::env::temp_dir().join(format!("picsw-download-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("image.png");

        write_atomically(&path, b"image-bytes").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"image-bytes");
        assert!(!dir.join("image.png.part").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
