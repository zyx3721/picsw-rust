//! 更新包下载：SHA-256 完整性校验、.part 中转防半包、Channel 进度推送

use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};
use tauri::ipc::Channel;

use super::{temp_update_dir, PROGRESS_EMIT_STEP_BYTES};

/// 下载进度（total 为 0 表示服务端未返回长度）
#[derive(Clone, serde::Serialize)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
}

/// 下载专用客户端：不限总时长（大包由网络速度决定），仅限制连接与单次读超时防挂死
fn dl_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建下载客户端失败: {e}"))
}

/// 从校验和清单文本中提取目标文件的 SHA-256（容忍 sha256sum 二进制模式的 * 前缀与安装包名空格/点号差异）
fn extract_checksum(content: &str, asset_name: &str) -> Result<String, String> {
    let normalize = |value: &str| value.replace('.', " ");
    let target = normalize(asset_name);
    for line in content.lines() {
        let line = line.trim();
        if line.len() < 66 {
            continue;
        }
        let (hash, name) = line.split_at(64);
        let name = name.trim_start().trim_start_matches('*');
        if hash.chars().all(|c| c.is_ascii_hexdigit()) && normalize(name) == target {
            return Ok(hash.to_ascii_lowercase());
        }
    }
    Err("校验信息中未找到更新包，已取消更新".to_string())
}

/// 流式下载更新包到临时目录（.part 中转，完成后 rename 为正式名），经 SHA256SUMS 清单比对完整性，
/// 进度经 Channel 推送；返回落盘路径交由安装命令按平台分派
#[tauri::command]
pub async fn download_update(
    url: String,
    checksum_url: String,
    asset_name: String,
    on_progress: Channel<DownloadProgress>,
) -> Result<String, String> {
    if asset_name.is_empty()
        || asset_name.contains('\\')
        || asset_name.contains('/')
        || asset_name.contains("..")
        || !url.rsplit('/').next().is_some_and(|tail| tail == asset_name)
    {
        return Err("更新包地址不正确".to_string());
    }
    let expected = fetch_expected_checksum(&checksum_url, &asset_name).await?;

    let dir = temp_update_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {e}"))?;
    let dest = dir.join(&asset_name);
    let part = dir.join(format!("{asset_name}.part"));

    let mut response = dl_client()?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("连接下载源失败: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "下载更新包失败: HTTP {}",
            response.status().as_u16()
        ));
    }
    let total = response.content_length().unwrap_or(0);

    let actual = match stream_to_file(&mut response, &part, &on_progress, total).await {
        Ok(digest) => digest,
        Err(error) => {
            let _ = std::fs::remove_file(&part);
            return Err(error);
        }
    };
    // .part → 正式名：保证后续安装与打开拿到的必然是完整文件
    if actual != expected {
        let _ = std::fs::remove_file(&part);
        return Err("更新包完整性校验失败，已取消更新".to_string());
    }
    std::fs::rename(&part, &dest).map_err(|e| format!("更新包落盘失败: {e}"))?;
    Ok(dest.to_string_lossy().into_owned())
}

/// 获取并解析校验和清单中的期望值
async fn fetch_expected_checksum(checksum_url: &str, asset_name: &str) -> Result<String, String> {
    let response = dl_client()?
        .get(checksum_url)
        .send()
        .await
        .map_err(|e| format!("获取校验信息失败: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "获取校验信息失败: HTTP {}",
            response.status().as_u16()
        ));
    }
    let content = response
        .text()
        .await
        .map_err(|e| format!("读取校验信息失败: {e}"))?;
    extract_checksum(&content, asset_name)
}

/// 流式写盘并推送下载进度，返回内容 SHA-256 摘要；失败由调用方负责删除 .part 残片
async fn stream_to_file(
    response: &mut reqwest::Response,
    part: &Path,
    on_progress: &Channel<DownloadProgress>,
    total: u64,
) -> Result<String, String> {
    let mut file = std::fs::File::create(part).map_err(|e| format!("写入更新包失败: {e}"))?;
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut last_emit: u64 = 0;
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                hasher.update(&chunk);
                file.write_all(&chunk)
                    .map_err(|e| format!("写入更新包失败: {e}"))?;
                downloaded += chunk.len() as u64;
                if downloaded - last_emit >= PROGRESS_EMIT_STEP_BYTES {
                    last_emit = downloaded;
                    let _ = on_progress.send(DownloadProgress { downloaded, total });
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("下载更新包中断: {e}")),
        }
    }
    file.flush().map_err(|e| format!("写入更新包失败: {e}"))?;
    let _ = on_progress.send(DownloadProgress { downloaded, total });
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_extraction_handles_ci_formats() {
        let hash = "a".repeat(64);
        let asset = "picbed-switcher_1.2.0_linux_amd64.tar.gz";
        // 标准空格分隔
        let plain = format!("{hash}  {asset}");
        assert_eq!(extract_checksum(&plain, asset).unwrap(), hash);
        // sha256sum 二进制模式 * 前缀
        let binary = format!("{hash} *{asset}");
        assert_eq!(extract_checksum(&binary, asset).unwrap(), hash);
        // 安装包名点号与清单空格归一化匹配
        let dotted = format!("{hash}  PicBed.Switcher_1.2.0_x64-setup.exe");
        assert_eq!(
            extract_checksum(&dotted, "PicBed Switcher_1.2.0_x64-setup.exe").unwrap(),
            hash
        );
        // 哈希大小写归一
        let upper = format!("{}  {asset}", hash.to_uppercase());
        assert_eq!(extract_checksum(&upper, asset).unwrap(), hash);
    }

    #[test]
    fn checksum_extraction_rejects_missing_or_malformed() {
        let hash = "a".repeat(64);
        assert!(extract_checksum("short line", "a.tar.gz").is_err());
        assert!(extract_checksum(&format!("{hash}  a.tar.gz"), "b.tar.gz").is_err());
        assert!(extract_checksum(&format!("zzzz  a.tar.gz"), "a.tar.gz").is_err());
        assert!(extract_checksum("", "a.tar.gz").is_err());
    }
}
