use std::path::PathBuf;

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
