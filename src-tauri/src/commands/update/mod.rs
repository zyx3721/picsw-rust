//! 应用自动更新：运行时形态检测、最新版查询、带进度下载与多平台安装分派

pub mod download;
pub mod fetch;
pub mod install;

/// GitHub 仓库最新 Release 的官方接口地址
pub(crate) const RELEASE_API_URL: &str = "https://api.github.com/repos/zyx3721/picsw-rust/releases/latest";

/// 发布页最新版地址，官方接口异常时经其 302 跳转解析版本号（不受接口限流约束）
pub(crate) const RELEASE_LATEST_URL: &str = "https://github.com/zyx3721/picsw-rust/releases/latest";

/// 更新相关 HTTP 请求使用的 User-Agent（GitHub 接口要求请求必须携带）
pub(crate) const UPDATE_USER_AGENT: &str = concat!("picbed-switcher/", env!("CARGO_PKG_VERSION"));

/// 更新检查类 HTTP 请求的超时秒数
pub(crate) const UPDATE_HTTP_TIMEOUT_SECS: u64 = 6;

/// 下载进度推送的节流字节数（避免高频 IPC 刷屏）
pub(crate) const PROGRESS_EMIT_STEP_BYTES: u64 = 128 * 1024;

/// 临时下载目录（更新包、解压与替换脚本所在；可随时重下，启动时整体清理）
pub(crate) fn temp_update_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("picbed-switcher-update")
}

/// 清理上一轮更新残留：临时下载目录与旧程序改名备份（应用启动时调用，幂等尽力而为）
pub fn cleanup_update_temp() {
    let _ = std::fs::remove_dir_all(temp_update_dir());
    cleanup_old_backup();
}

/// 删除旧程序改名备份：文件（Linux/Windows 便携 exe.old）与目录（macOS `.app.old`）统一尽力清理
fn cleanup_old_backup() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut name = exe.clone().into_os_string();
    name.push(".old");
    let _ = std::fs::remove_file(std::path::PathBuf::from(name));
    // macOS .app 目录形态：可执行文件位于 Foo.app/Contents/MacOS/<bin>，备份目录与其同名加 .old
    if let Some(bundle) = current_app_bundle() {
        let mut name = bundle.clone().into_os_string();
        name.push(".old");
        let _ = std::fs::remove_dir_all(std::path::PathBuf::from(name));
    }
}

#[derive(serde::Serialize)]
pub struct UpdateRuntime {
    pub os: String,
    pub arch: String,
    pub portable: bool,
    /// Linux 系统包类型（deb / rpm），其他平台为空串
    pub package: String,
}

/// 当前运行环境的平台架构与安装形态（安装版 / 便携版）
#[tauri::command]
pub fn get_update_runtime() -> UpdateRuntime {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };
    let os = std::env::consts::OS.to_string();
    let package = if os == "linux" { detect_linux_package().to_string() } else { String::new() };
    UpdateRuntime {
        os,
        arch: arch.to_string(),
        portable: !is_installed_build(),
        package,
    }
}

/// 运行时是否为便携形态（安装分派与打开兜底共用）
pub(crate) fn is_portable_runtime() -> bool {
    !is_installed_build()
}

/// 安装版判定：Windows 看 NSIS 卸载程序，macOS 看是否运行于 .app 内，Linux 看二进制是否在系统目录
pub(crate) fn is_installed_build() -> bool {
    if cfg!(target_os = "windows") {
        return windows_installed();
    }
    if cfg!(target_os = "macos") {
        return current_app_bundle().is_some();
    }
    exe_in_system_dir()
}

/// NSIS 安装特征：同目录存在卸载程序，且注册表卸载项能搜到应用名——
/// 整体拷走使用的安装版目录有卸载程序但无注册表记录，按便携处理（走自替换而非向系统重装）
#[cfg(target_os = "windows")]
fn windows_installed() -> bool {
    let has_uninstaller = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("uninstall.exe").is_file()))
        .unwrap_or(false);
    has_uninstaller && nsis_registry_record_exists()
}

/// 注册表卸载项搜索：Tauri NSIS 默认按当前用户安装（HKCU），兼容查 HKLM；查询失败按未安装处理
#[cfg(target_os = "windows")]
fn nsis_registry_record_exists() -> bool {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const ROOTS: [&str; 2] = [
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall",
    ];
    for root in ROOTS {
        let hit = std::process::Command::new("reg")
            .args(["query", root, "/s", "/f", "PicBed Switcher", "/d"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|output| output.status.success() && !output.stdout.is_empty())
            .unwrap_or(false);
        if hit {
            return true;
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn windows_installed() -> bool {
    false
}

/// 可执行文件是否位于系统安装目录（Linux 安装版特征：/usr/bin、/usr/local/bin、/opt）
fn exe_in_system_dir() -> bool {
    let Some(exe) = std::env::current_exe().ok() else {
        return false;
    };
    let Some(path) = exe.to_str() else {
        return false;
    };
    ["/usr/bin/", "/usr/local/bin/", "/opt/"].iter().any(|prefix| path.starts_with(prefix))
}

/// 当前进程所在 .app 包根目录（macOS 专用：路径含 Foo.app/Contents/MacOS/ 时回溯到 Foo.app）
pub(crate) fn current_app_bundle() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.to_str()?;
    let marker = ".app/Contents/";
    let index = path.find(marker)?;
    Some(std::path::PathBuf::from(&path[..index + 4]))
}

/// Linux 系统包类型检测：Debian 系用 deb，RedHat/SUSE 系用 rpm，默认 deb
pub(crate) fn detect_linux_package() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        linux_package_from(
            std::path::Path::new("/etc/debian_version").exists(),
            std::path::Path::new("/etc/redhat-release").exists()
                || std::path::Path::new("/etc/SuSE-release").exists(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        linux_package_from(false, false)
    }
}

/// 包类型判定的纯函数核心（便于单测）：Debian 系优先，其余发行版按 rpm，兜底 deb
fn linux_package_from(debian_based: bool, redhat_based: bool) -> &'static str {
    if debian_based {
        "deb"
    } else if redhat_based {
        "rpm"
    } else {
        "deb"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_package_detection_prefers_debian() {
        assert_eq!(linux_package_from(true, true), "deb");
        assert_eq!(linux_package_from(true, false), "deb");
        assert_eq!(linux_package_from(false, true), "rpm");
        assert_eq!(linux_package_from(false, false), "deb");
    }

    #[test]
    fn app_bundle_path_detection() {
        // macOS 安装形态：路径含 .app/Contents/ 时回溯 .app 根
        let path = "/Applications/PicBed Switcher.app/Contents/MacOS/picbed-switcher";
        let marker = ".app/Contents/";
        let index = path.find(marker).unwrap();
        assert_eq!(&path[..index + 4], "/Applications/PicBed Switcher.app");
        // 便携裸二进制路径不含标记
        assert!("/opt/tools/picbed-switcher".find(marker).is_none());
    }
}
