//! 检查更新的运行时信息、下载更新包与自替换安装

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// GitHub 仓库最新 Release 的官方接口地址
const RELEASE_API_URL: &str = "https://api.github.com/repos/zyx3721/picsw-rust/releases/latest";

/// 发布页最新版地址，官方接口异常时经其 302 跳转解析版本号（不受接口限流约束）
const RELEASE_LATEST_URL: &str = "https://github.com/zyx3721/picsw-rust/releases/latest";

/// 更新相关 HTTP 请求使用的 User-Agent（GitHub 接口要求请求必须携带）
const UPDATE_USER_AGENT: &str = concat!("picbed-switcher/", env!("CARGO_PKG_VERSION"));

/// 更新相关 HTTP 请求的超时秒数
const UPDATE_HTTP_TIMEOUT_SECS: u64 = 6;

/// 临时下载目录常量名（更新包、校验与替换脚本所在）
fn temp_update_dir() -> PathBuf {
    std::env::temp_dir().join("picbed-switcher-update")
}

/// 清理上一轮更新残留的临时下载目录（应用启动时调用，幂等）
pub fn cleanup_update_temp() {
    let _ = std::fs::remove_dir_all(temp_update_dir());
}

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

/// 最新 Release 查询结果（tag 含 v 前缀；assets 为 None 表示清单缺失，由前端按命名规律直拼地址）
#[derive(Serialize)]
pub struct UpdateLatest {
    pub tag: String,
    pub assets: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: Option<String>,
    assets: Option<Vec<ApiAsset>>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
}

/// 查询 GitHub 最新 Release 的版本号与资产名清单，官方接口异常时回退发布页跳转解析
#[tauri::command]
pub async fn get_update_latest() -> Result<UpdateLatest, String> {
    let api_error = match fetch_release_api().await {
        Ok(latest) => return Ok(latest),
        Err(error) => error,
    };
    match fetch_release_redirect().await {
        Ok(latest) => Ok(latest),
        Err(error) => Err(format!("{api_error}；{error}")),
    }
}

/// 经官方接口获取最新 Release，返回版本号与资产名清单
async fn fetch_release_api() -> Result<UpdateLatest, String> {
    let client = update_http_client(reqwest::redirect::Policy::default())?;
    let response = client
        .get(RELEASE_API_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("连接 GitHub 接口失败: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub 接口响应异常（{}）",
            response.status().as_u16()
        ));
    }
    let text = response
        .text()
        .await
        .map_err(|e| format!("读取版本信息失败: {e}"))?;
    let release: ApiRelease = serde_json::from_str(&text).map_err(|_| "版本信息解析失败".to_string())?;
    let tag = release.tag_name.unwrap_or_default();
    if tag.is_empty() {
        return Err("版本信息缺少版本号".to_string());
    }
    let assets = release
        .assets
        .map(|list| list.into_iter().map(|asset| asset.name).collect());
    Ok(UpdateLatest { tag, assets })
}

/// 经发布页 302 跳转地址解析最新版本号（无资产清单）
async fn fetch_release_redirect() -> Result<UpdateLatest, String> {
    let client = update_http_client(reqwest::redirect::Policy::none())?;
    let response = client
        .get(RELEASE_LATEST_URL)
        .send()
        .await
        .map_err(|e| format!("连接发布页失败: {e}"))?;
    let location = response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| "发布页未返回跳转地址".to_string())?;
    let tag = location.rsplit('/').next().unwrap_or_default();
    if tag.is_empty() || !(tag.starts_with('v') || tag.starts_with(|c: char| c.is_ascii_digit())) {
        return Err("未从发布页解析到版本号".to_string());
    }
    Ok(UpdateLatest {
        tag: tag.to_string(),
        assets: None,
    })
}

/// 创建更新检查专用 HTTP 客户端（统一 UA 与超时）
fn update_http_client(redirect: reqwest::redirect::Policy) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UPDATE_USER_AGENT)
        .timeout(std::time::Duration::from_secs(UPDATE_HTTP_TIMEOUT_SECS))
        .redirect(redirect)
        .build()
        .map_err(|e| format!("创建网络客户端失败: {e}"))
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
    let dir = temp_update_dir();
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
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("启动替换脚本失败: {e}"))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn write_portable_script(package: &Path, dir: &Path, exe: &Path) -> Result<PathBuf, String> {
    let path = script_path()?;
    let temp_dir = path
        .parent()
        .ok_or_else(|| "获取临时目录失败".to_string())?
        .to_path_buf();
    let body = format!(
        "Start-Sleep -Seconds 2\nExpand-Archive -Force -Path '{}' -DestinationPath '{}'\nRemove-Item -LiteralPath '{}\\README.txt' -ErrorAction SilentlyContinue\nStart-Process -FilePath '{}'\nRemove-Item -Recurse -Force '{}' -ErrorAction SilentlyContinue\n",
        package.display(),
        dir.display(),
        dir.display(),
        exe.display(),
        temp_dir.display()
    );
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend_from_slice(body.as_bytes());
    std::fs::write(&path, bytes).map_err(|e| format!("写入更新脚本失败: {e}"))?;
    Ok(path)
}

#[cfg(target_os = "windows")]
fn script_path() -> Result<PathBuf, String> {
    let dir = temp_update_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {e}"))?;
    Ok(dir.join("apply-portable-update.ps1"))
}
