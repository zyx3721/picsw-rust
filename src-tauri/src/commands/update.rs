//! 检查更新的运行时信息、下载更新包与自替换安装

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
pub struct UpdateRuntime {
    pub os: String,
    pub arch: String,
    pub portable: bool,
}

/// 当前运行环境的平台架构与安装形态（安装版 / 便携版）
#[tauri::command]
pub fn get_update_runtime() -> UpdateRuntime {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };
    let portable = !is_installed_build();
    UpdateRuntime {
        os: std::env::consts::OS.to_string(),
        arch: arch.to_string(),
        portable,
    }
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

/// 流式下载更新包到临时目录，经发布页校验和清单比对 SHA-256 后返回落盘路径
#[tauri::command]
pub async fn download_update(url: String, checksum_url: String, asset_name: String) -> Result<String, String> {
    if asset_name.is_empty()
        || asset_name.contains('\\')
        || asset_name.contains('/')
        || asset_name.contains("..")
        || !url.rsplit('/').next().is_some_and(|tail| tail == asset_name)
    {
        return Err("更新包地址不正确".to_string());
    }
    let checksum = reqwest::get(&checksum_url)
        .await
        .map_err(|e| format!("获取校验信息失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("获取校验信息失败: {e}"))?
        .text()
        .await
        .map_err(|e| format!("读取校验信息失败: {e}"))?;
    let expected = extract_checksum(&checksum, &asset_name)?;
    let dir = std::env::temp_dir().join("picbed-switcher-update");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {e}"))?;
    let dest = dir.join(&asset_name);
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("连接下载源失败: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "下载更新包失败: HTTP {}",
            response.status().as_u16()
        ));
    }
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::create(&dest).map_err(|e| format!("写入更新包失败: {e}"))?;
    let mut stream = response;
    loop {
        match stream.chunk().await {
            Ok(Some(chunk)) => {
                hasher.update(&chunk);
                file.write_all(&chunk).map_err(|e| {
                    let _ = std::fs::remove_file(&dest);
                    format!("写入更新包失败: {e}")
                })?;
            }
            Ok(None) => break,
            Err(e) => {
                let _ = std::fs::remove_file(&dest);
                return Err(format!("下载更新包中断: {e}"));
            }
        }
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected {
        let _ = std::fs::remove_file(&dest);
        return Err("更新包完整性校验失败，已取消更新".to_string());
    }
    Ok(dest.to_string_lossy().into_owned())
}

/// 启动更新安装：安装版静默安装到原目录并重启，便携版经延迟脚本解压覆盖后重启；成功后当前进程立即退出
#[tauri::command]
pub async fn apply_update(file_path: String) -> Result<(), String> {
    let package = PathBuf::from(&file_path);
    if !package.exists() {
        return Err("更新包不存在或已被清理".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        windows_apply(&package)?;
        std::process::exit(0);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = package;
        Err("当前平台暂不支持自动更新，请到发布页手动下载".to_string())
    }
}

/// NSIS 安装特征：可执行文件同目录存在卸载程序
#[cfg(target_os = "windows")]
fn is_installed_build() -> bool {
    current_install_dir()
        .map(|dir| dir.join("uninstall.exe").is_file())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn is_installed_build() -> bool {
    false
}

fn current_install_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

#[cfg(target_os = "windows")]
fn windows_apply(package: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "获取安装目录失败".to_string())?;
    if package.extension().and_then(|e| e.to_str()) == Some("exe") {
        Command::new(package)
            .arg("/S")
            .arg("/R")
            .arg(format!("/D={}", dir.display()))
            .spawn()
            .map_err(|e| format!("启动安装程序失败: {e}"))?;
    } else {
        let script = write_portable_script(package, dir, &exe)?;
        Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script)
            .spawn()
            .map_err(|e| format!("启动替换脚本失败: {e}"))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn write_portable_script(package: &Path, dir: &Path, exe: &Path) -> Result<PathBuf, String> {
    let path = script_path()?;
    let body = format!(
        "Start-Sleep -Seconds 2\nExpand-Archive -Force -Path '{}' -DestinationPath '{}'\nStart-Process -FilePath '{}'\nRemove-Item -LiteralPath '{}'\n",
        package.display(),
        dir.display(),
        exe.display(),
        path.display()
    );
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend_from_slice(body.as_bytes());
    std::fs::write(&path, bytes).map_err(|e| format!("写入更新脚本失败: {e}"))?;
    Ok(path)
}

#[cfg(target_os = "windows")]
fn script_path() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("picbed-switcher-update");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {e}"))?;
    Ok(dir.join("apply-portable-update.ps1"))
}
