//! 网络图片批量下载命令

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::uploader;

#[derive(Serialize)]
pub struct DownloadedImage {
    pub url: String,
    pub file_name: String,
    pub saved_path: String,
    pub status: String,
    pub error: String,
}

/// 将网络图片逐张下载到指定目录，返回逐图结果
#[tauri::command]
pub async fn download_remote_images(urls: Vec<String>, target_dir: String) -> Result<Vec<DownloadedImage>, String> {
    let dir = target_dir.trim();
    if dir.is_empty() {
        return Err("下载目录不能为空".to_string());
    }
    let target = PathBuf::from(dir);
    if !target.is_dir() {
        return Err("下载目录不存在，请重新选择".to_string());
    }
    if urls.is_empty() {
        return Err("没有可下载的图片地址".to_string());
    }

    let mut results = Vec::with_capacity(urls.len());
    for raw in urls {
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
        results.push(item);
    }
    Ok(results)
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
