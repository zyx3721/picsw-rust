//! 网络图片批量下载命令

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

use crate::uploader;

#[derive(Serialize)]
pub struct DownloadedImage {
    pub url: String,
    pub file_name: String,
    pub saved_path: String,
    pub status: String,
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct DownloadDocumentRequest {
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub urls: Vec<String>,
}

#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    pub document_index: usize,
    pub filename: String,
    pub processed: usize,
    pub total: usize,
    pub status: String,
    pub error: String,
}

/// 按文档分组下载网络图片，逐张下载并通过 Channel 推送文档级图片进度
#[tauri::command]
pub async fn download_remote_images(
    documents: Vec<DownloadDocumentRequest>,
    target_dir: String,
    on_progress: Channel<DownloadProgress>,
) -> Result<Vec<DownloadedImage>, String> {
    let dir = target_dir.trim();
    if dir.is_empty() {
        return Err("下载目录不能为空".to_string());
    }
    let target = PathBuf::from(dir);
    if !target.is_dir() {
        return Err("下载目录不存在，请重新选择".to_string());
    }
    if documents.iter().all(|document| document.urls.is_empty()) {
        return Err("没有可下载的图片地址".to_string());
    }

    let mut results = Vec::new();
    for (index, document) in documents.into_iter().enumerate() {
        let total = document.urls.len();
        let mut processed = 0usize;
        let mut failed = 0usize;
        send_progress(&on_progress, index, &document.filename, 0, total, "running", "");
        for raw in document.urls {
            let url = raw.trim().to_string();
            let mut item = DownloadedImage {
                url: url.clone(),
                file_name: String::new(),
                saved_path: String::new(),
                status: "failed".to_string(),
                error: String::new(),
            };
            match uploader::download_image(&url).await {
                Ok(image) => {
                    item.file_name = image.filename.clone();
                    let path = unique_target(&target, &image.filename);
                    match std::fs::write(&path, &image.data) {
                        Ok(()) => {
                            item.saved_path = path.to_string_lossy().to_string();
                            item.status = "success".to_string();
                        }
                        Err(error) => item.error = format!("写入文件失败：{error}"),
                    }
                }
                Err(error) => item.error = error,
            }
            processed += 1;
            if item.status != "success" {
                failed += 1;
            }
            send_progress(&on_progress, index, &document.filename, processed, total, "running", "");
            results.push(item);
        }
        let status = if failed > 0 { "failed" } else { "success" };
        let error = if failed > 0 {
            format!("{failed} 张图片下载失败")
        } else {
            String::new()
        };
        send_progress(&on_progress, index, &document.filename, total, total, status, &error);
    }
    Ok(results)
}

fn send_progress(
    on_progress: &Channel<DownloadProgress>,
    document_index: usize,
    filename: &str,
    processed: usize,
    total: usize,
    status: &str,
    error: &str,
) {
    let _ = on_progress.send(DownloadProgress {
        document_index,
        filename: filename.to_string(),
        processed,
        total,
        status: status.to_string(),
        error: error.to_string(),
    });
}

/// 同名文件自动追加序号，避免覆盖已下载内容
fn unique_target(dir: &Path, filename: &str) -> PathBuf {
    let trimmed = filename.trim();
    let name = if trimmed.is_empty() { "image.png" } else { trimmed };
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image")
        .to_string();
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{s}"))
        .unwrap_or_default();
    let mut index = 1u32;
    let mut candidate = dir.join(name);
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({index}){ext}"));
        index += 1;
    }
    candidate
}
