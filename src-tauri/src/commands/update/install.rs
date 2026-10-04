//! 更新包安装：按平台与发行形态分派（Windows 静默/脚本、Linux 系统包与便携、macOS dmg 与便携），
//! 另提供「打开更新包」作为自动安装失败的兜底出口

use std::path::{Path, PathBuf};

use super::temp_update_dir;

/// 系统包安装（pkexec 提权）的授权与执行超时秒数
#[cfg(all(unix, not(target_os = "macos")))]
const PKEXEC_TIMEOUT_SECS: u64 = 300;

/// 应用已下载的更新包：下载完成后自动调用，按平台与发行形态分派安装；
/// 成功路径下当前进程会退出或重启，正常不返回
#[tauri::command]
pub async fn apply_update(_app: tauri::AppHandle, file_path: String) -> Result<(), String> {
    let package = validate_package_path(&file_path)?;
    #[cfg(target_os = "windows")]
    {
        windows_apply(&package)?;
        std::process::exit(0);
    }
    #[cfg(all(unix, target_os = "macos"))]
    {
        if super::is_portable_runtime() {
            install_portable_unix(&package).await
        } else {
            install_macos_dmg(&package).await
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if super::is_portable_runtime() {
            install_portable_unix(&package).await
        } else {
            install_system_package(&package).await
        }
    }
}

/// 打开已下载的更新包（自动安装失败的兜底）：安装形态经系统打开器拉起安装器，
/// 便携形态无系统安装器，返回手动替换指引
#[tauri::command]
pub async fn open_update_package(app: tauri::AppHandle, file_path: String) -> Result<(), String> {
    let package = validate_package_path(&file_path)?;
    if super::is_portable_runtime() {
        return Err(format!(
            "当前为便携形态，无系统安装器：更新包已下载到 {}，请按包内说明手动替换当前程序后重新打开",
            package.display()
        ));
    }
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_path(package.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| format!("打开更新包失败: {e}"))
}

/// 校验待安装/打开的更新包路径：必须存在且位于临时更新目录内（canonicalize 防路径穿越）
fn validate_package_path(file_path: &str) -> Result<PathBuf, String> {
    let dir = temp_update_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {e}"))?;
    let dir = dir.canonicalize().map_err(|e| format!("读取临时目录失败: {e}"))?;
    let target = PathBuf::from(file_path);
    if !target.exists() {
        return Err("更新包不存在或已被清理，请重新下载".to_string());
    }
    let target = target.canonicalize().map_err(|_| "更新包不存在或已被清理，请重新下载".to_string())?;
    if !target.starts_with(&dir) {
        return Err("非法的更新包路径".to_string());
    }
    Ok(target)
}

/// Windows 安装分派：NSIS 包静默安装到原目录并随模板重启；zip 便携包写延迟替换脚本执行
#[cfg(target_os = "windows")]
fn windows_apply(package: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let exe = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;
    let dir = exe.parent().ok_or_else(|| "获取安装目录失败".to_string())?;
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

/// Windows 便携替换脚本：等待进程退出后解压覆盖原目录、拉起新版本并清理临时目录；
/// UTF-8 BOM 保证中文路径不乱码
#[cfg(target_os = "windows")]
fn write_portable_script(package: &Path, dir: &Path, exe: &Path) -> Result<PathBuf, String> {
    let dir_tmp = temp_update_dir();
    std::fs::create_dir_all(&dir_tmp).map_err(|e| format!("创建临时目录失败: {e}"))?;
    let path = dir_tmp.join("apply-portable-update.ps1");
    let body = format!(
        "Start-Sleep -Seconds 2\nExpand-Archive -Force -Path '{}' -DestinationPath '{}'\nRemove-Item -LiteralPath '{}\\README.txt' -ErrorAction SilentlyContinue\nStart-Process -FilePath '{}'\nRemove-Item -Recurse -Force '{}' -ErrorAction SilentlyContinue\n",
        package.display(),
        dir.display(),
        dir.display(),
        exe.display(),
        dir_tmp.display()
    );
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend_from_slice(body.as_bytes());
    std::fs::write(&path, bytes).map_err(|e| format!("写入更新脚本失败: {e}"))?;
    Ok(path)
}

/// Linux 系统包安装：pkexec 提权后按包类型 dpkg -i / rpm -Uvh，成功删包并重启应用
#[cfg(all(unix, not(target_os = "macos")))]
async fn install_system_package(package: &Path) -> Result<(), String> {
    let (tool, args): (&str, Vec<&str>) = match super::detect_linux_package() {
        "rpm" => ("rpm", vec!["-Uvh"]),
        _ => ("dpkg", vec!["-i"]),
    };
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(PKEXEC_TIMEOUT_SECS),
        tokio::process::Command::new("pkexec")
            .arg(tool)
            .args(&args)
            .arg(package)
            .output(),
    )
    .await
    .map_err(|_| "安装超时，请检查系统授权窗口状态".to_string())?
    .map_err(|e| format!("无法启动 pkexec: {e}（请确认系统已安装 polkit）"))?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("dismissed") || stderr.contains("cancelled") {
        return Err("已取消安装授权".to_string());
    }
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let line = stderr.lines().last().unwrap_or("").trim();
        return Err(format!(
            "安装更新失败: {}",
            if line.is_empty() { format!("{tool} 退出码 {code}") } else { line.to_string() }
        ));
    }
    let _ = tokio::fs::remove_file(package).await;
    restart_self()
}

/// Unix 便携包安装（Linux tar.gz 裸二进制 / macOS tar.gz 内含 .app）：
/// 解压到临时目录后按当前运行形态改名换入对应产物，任一步失败回滚
#[cfg(unix)]
async fn install_portable_unix(package: &Path) -> Result<(), String> {
    let extract_dir = temp_update_dir().join("extract");
    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir).map_err(|e| format!("创建临时目录失败: {e}"))?;

    let output = tokio::process::Command::new("tar")
        .args(["-xzf"])
        .arg(package)
        .arg("-C")
        .arg(&extract_dir)
        .output()
        .await
        .map_err(|e| format!("无法启动 tar: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("解压更新包失败: {}", stderr.trim()));
    }

    let (staged, target) = resolve_portable_target(&extract_dir)?;
    replace_path(&staged, &target).await?;
    restart_self()
}

/// 决定便携包换入产物与目标：macOS 运行于 .app 内时替换整个 .app（tar 内同名包），
/// 其余情形替换可执行文件本体
#[cfg(unix)]
fn resolve_portable_target(extract_dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    let exe = std::env::current_exe().map_err(|e| format!("定位当前程序失败: {e}"))?;
    if cfg!(target_os = "macos") {
        if let Some(bundle) = super::current_app_bundle() {
            let staged = find_extracted_dir(extract_dir, ".app")
                .ok_or_else(|| "解压产物中未找到应用包".to_string())?;
            return Ok((staged, bundle));
        }
    }
    let staged = find_extracted_file(extract_dir, "picbed-switcher")
        .ok_or_else(|| "解压产物中未找到程序文件".to_string())?;
    Ok((staged, exe))
}

/// 在解压目录（含一层子目录）中查找指定扩展名目录（macOS .app）
#[cfg(unix)]
fn find_extracted_dir(dir: &Path, extension: &str) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.extension().and_then(|e| e.to_str()) == Some(extension) {
                    return Some(path);
                }
                stack.push(path);
            }
        }
    }
    None
}

/// 在解压目录（含一层子目录）中查找指定名称文件（便携二进制）
#[cfg(unix)]
fn find_extracted_file(dir: &Path, file_name: &str) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some(file_name) {
                return Some(path);
            }
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
    None
}

/// 通用「改名换入」替换：先把产物拷贝到目标同目录暂存（保证最终 rename 同卷原子），
/// 目标改名 `.old` 备份后换入，失败回滚；`.old` 残留由启动清理兜底
#[cfg(unix)]
async fn replace_path(staged: &Path, target: &Path) -> Result<(), String> {
    let file_name = target
        .file_name()
        .ok_or_else(|| "目标路径不合法".to_string())?
        .to_string_lossy()
        .into_owned();
    let parent = target.parent().ok_or_else(|| "目标路径不合法".to_string())?;

    let staging = parent.join(format!(".{file_name}.update-staging"));
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_file(&staging);
    if staged.is_dir() {
        let output = tokio::process::Command::new("cp")
            .args(["-R"])
            .arg(staged)
            .arg(&staging)
            .output()
            .await
            .map_err(|e| format!("无法启动 cp: {e}"))?;
        if !output.status.success() {
            return Err(format!("拷贝新版本失败（目标目录可能只读）：{}", String::from_utf8_lossy(&output.stderr).trim()));
        }
    } else {
        tokio::fs::copy(staged, &staging)
            .await
            .map_err(|e| format!("拷贝新版本失败: {e}（目标目录可能只读）"))?;
    }

    let mut backup_name = std::ffi::OsString::from(&file_name);
    backup_name.push(".old");
    let backup = parent.join(backup_name);
    let _ = std::fs::remove_dir_all(&backup);
    let _ = std::fs::remove_file(&backup);

    tokio::fs::rename(target, &backup)
        .await
        .map_err(|e| format!("旧程序改名失败: {e}"))?;
    if let Err(e) = tokio::fs::rename(&staging, target).await {
        let _ = tokio::fs::rename(&backup, target).await;
        return Err(format!("换入新版本失败: {e}"));
    }
    Ok(())
}

/// macOS dmg 安装版更新：挂载镜像 → 拷贝 .app 暂存 → 改名换入 → 卸载镜像 → 重启应用
#[cfg(target_os = "macos")]
async fn install_macos_dmg(package: &Path) -> Result<(), String> {
    let bundle = super::current_app_bundle().ok_or_else(|| "未定位到当前应用包".to_string())?;
    let mount = temp_update_dir().join("dmg-mount");
    let _ = std::fs::remove_dir_all(&mount);
    std::fs::create_dir_all(&mount).map_err(|e| format!("创建临时目录失败: {e}"))?;

    let attach = tokio::process::Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
        .arg(&mount)
        .arg(package)
        .output()
        .await
        .map_err(|e| format!("无法启动 hdiutil: {e}"))?;
    if !attach.status.success() {
        return Err(format!("挂载更新镜像失败: {}", String::from_utf8_lossy(&attach.stderr).trim()));
    }

    let staged_in_volume = find_extracted_dir(&mount, ".app")
        .ok_or_else(|| "更新镜像中未找到应用包".to_string());
    let staged_in_volume = match staged_in_volume {
        Ok(path) => path,
        Err(error) => {
            let _ = detach_dmg(&mount).await;
            return Err(error);
        }
    };

    let result = replace_path(&staged_in_volume, &bundle).await;
    let _ = detach_dmg(&mount).await;
    result?;
    restart_self()
}

/// 卸载挂载的更新镜像（尽力而为）
#[cfg(target_os = "macos")]
async fn detach_dmg(mount: &Path) -> Result<(), String> {
    tokio::process::Command::new("hdiutil")
        .arg("detach")
        .arg(mount)
        .output()
        .await
        .map(|o| {
            if o.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
            }
        })
        .map_err(|e| format!("无法启动 hdiutil: {e}"))?
}

/// 重启应用：macOS .app 形态经 open（LaunchServices 接管），其余直接拉起自身后退出当前进程
#[cfg(unix)]
fn restart_self() -> Result<(), String> {
    if cfg!(target_os = "macos") {
        if let Some(bundle) = super::current_app_bundle() {
            std::process::Command::new("open")
                .arg(&bundle)
                .spawn()
                .map_err(|e| format!("拉起新版本失败: {e}"))?;
            std::process::exit(0);
        }
    }
    let exe = std::env::current_exe().map_err(|e| format!("定位当前程序失败: {e}"))?;
    std::process::Command::new(exe)
        .spawn()
        .map_err(|e| format!("拉起新版本失败: {e}"))?;
    std::process::exit(0);
}
