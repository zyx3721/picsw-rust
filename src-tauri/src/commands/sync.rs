//! 云同步凭据存储与配置数据加密命令

use crate::crypto;
use crate::secret_store::{self, SecretBackend};

/// GitHub 访问令牌在凭据存储中的条目标识
const TOKEN_KEY_ID: &str = "sync/github_token";

/// 令牌写入系统钥匙串（无钥匙串环境回退机器绑定加密文件），返回实际落点
#[tauri::command]
pub fn sync_save_credential(app: tauri::AppHandle, plaintext: String) -> Result<String, String> {
    let dir = secret_store::config_dir(&app)?;
    let backend = secret_store::save_secret(&dir, TOKEN_KEY_ID, &plaintext)?;
    Ok(backend.as_str().to_string())
}

/// 按记录的落点读取令牌
#[tauri::command]
pub fn sync_load_credential(app: tauri::AppHandle, backend: String) -> Result<Option<String>, String> {
    let parsed = SecretBackend::parse(&backend).ok_or("令牌存储位置未知，请重新连接 GitHub")?;
    let dir = secret_store::config_dir(&app)?;
    secret_store::load_secret(&dir, TOKEN_KEY_ID, parsed)
}

/// 清除钥匙串/回退文件中的令牌
#[tauri::command]
pub fn sync_delete_credential(app: tauri::AppHandle, backend: String) -> Result<(), String> {
    let parsed = SecretBackend::parse(&backend).ok_or("令牌存储位置未知")?;
    let dir = secret_store::config_dir(&app)?;
    secret_store::delete_secret(&dir, TOKEN_KEY_ID, parsed)
}

/// 解密旧版本遗留在 WebView 本地存储中的令牌密文（固定种子加密），用于迁移到钥匙串
#[tauri::command]
pub fn sync_load_legacy_credential(ciphertext: String) -> Result<String, String> {
    crypto::decrypt_string(&ciphertext)
}
