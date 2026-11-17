use std::path::{Path, PathBuf};

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif"];
const MAX_SCAN_FILES: usize = 5000;
const MAX_SCAN_DEPTH: usize = 12;

#[derive(serde::Serialize)]
struct ScannedImage {
    name: String,
    path: String,
    relative_path: String,
}

/// 将转换后的文本内容写入用户通过保存对话框选择的路径
#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("保存路径不能为空".to_string());
    }
    let target = PathBuf::from(&path);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&target, content.as_bytes()).map_err(|e| e.to_string())
}

/// 递归扫描用户选择的图片目录，供本地上传按路径加入图片
#[tauri::command]
pub fn scan_image_directory(path: String) -> Result<serde_json::Value, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("请选择图片目录".to_string());
    }
    let root = PathBuf::from(trimmed);
    if !root.is_dir() {
        return Err("图片目录不存在，请重新选择".to_string());
    }
    let mut images = Vec::new();
    let root_name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_string();
    collect_images(&root, &root, &root_name, 0, &mut images)?;
    images.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let truncated = images.len() >= MAX_SCAN_FILES;
    Ok(serde_json::json!({ "images": images, "truncated": truncated }))
}

fn collect_images(
    root: &Path,
    dir: &Path,
    root_name: &str,
    depth: usize,
    images: &mut Vec<ScannedImage>,
) -> Result<(), String> {
    if depth > MAX_SCAN_DEPTH || images.len() >= MAX_SCAN_FILES {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|error| format!("读取图片目录失败：{error}"))?;
    for entry in entries.flatten() {
        if images.len() >= MAX_SCAN_FILES {
            break;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_images(root, &path, root_name, depth + 1, images)?;
            continue;
        }
        if !is_image_file(&path) {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("image")
            .to_string();
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let relative_path = if root_name.is_empty() {
            relative
        } else {
            format!("{root_name}/{relative}")
        };
        images.push(ScannedImage {
            name,
            path: path.to_string_lossy().to_string(),
            relative_path,
        });
    }
    Ok(())
}

fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| IMAGE_EXTENSIONS.contains(&extension.to_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::is_image_file;
    use std::path::Path;

    #[test]
    fn recognizes_common_image_extensions_case_insensitive() {
        assert!(is_image_file(Path::new("a.png")));
        assert!(is_image_file(Path::new("a.JPEG")));
        assert!(is_image_file(Path::new("a.webp")));
        assert!(!is_image_file(Path::new("a.txt")));
        assert!(!is_image_file(Path::new("noext")));
    }
}
