//! 敏感凭据（GitHub 访问令牌）的本地存储。
//!
//! 优先写入系统钥匙串（macOS Keychain / Windows 凭据管理器 / Linux Secret Service）；
//! 无钥匙串环境（无桌面的 Linux 等）自动回退到机器绑定的 AES-256-GCM 加密文件
//! （`app_config_dir()/secrets.bin`），密钥由 machine-uid 经 SHA-256 派生。
//! 回退属于混淆级防护：可防同机其他普通用户直读，不防本用户与 root，换机/重装后不可解密。
//!
//! 实际落点由调用方记录（前端保存 backend 字符串），读取时按记录直取，不做猜测。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Manager;

/// 凭据实际存储位置
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretBackend {
    /// 系统钥匙串
    Keyring,
    /// 机器绑定加密文件（回退）
    File,
}

impl SecretBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            SecretBackend::Keyring => "keyring",
            SecretBackend::File => "file",
        }
    }

    pub fn parse(s: &str) -> Option<SecretBackend> {
        match s {
            "keyring" => Some(SecretBackend::Keyring),
            "file" => Some(SecretBackend::File),
            _ => None,
        }
    }
}

/// keyring 条目：同一 service 下按 key_id 隔离
const KEYRING_SERVICE: &str = "com.picsw.desktop";

pub fn config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {e}"))
}

/// 保存密钥：优先钥匙串，失败自动落加密文件，返回实际落点
pub fn save_secret(dir: &Path, key_id: &str, secret: &str) -> Result<SecretBackend, String> {
    match save_keyring(key_id, secret) {
        Ok(()) => Ok(SecretBackend::Keyring),
        // 无钥匙串环境（无桌面 Linux 等）：回退加密文件
        Err(_) => {
            save_file(dir, key_id, secret)?;
            Ok(SecretBackend::File)
        }
    }
}

pub fn load_secret(dir: &Path, key_id: &str, backend: SecretBackend) -> Result<Option<String>, String> {
    match backend {
        SecretBackend::Keyring => load_keyring(key_id),
        SecretBackend::File => load_file(dir, key_id),
    }
}

pub fn delete_secret(dir: &Path, key_id: &str, backend: SecretBackend) -> Result<(), String> {
    let result = match backend {
        SecretBackend::Keyring => delete_keyring(key_id),
        SecretBackend::File => delete_file(dir, key_id),
    };
    match result {
        Ok(()) => Ok(()),
        // keyring 的"条目不存在"与文件的密钥缺失都视为已清除
        Err(e) if e.contains("NoEntry") || e.contains("不存在") || e.contains("找不到") => Ok(()),
        Err(e) => Err(e),
    }
}

fn keyring_entry(key_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, key_id).map_err(|e| format!("创建钥匙串条目失败: {e}"))
}

fn save_keyring(key_id: &str, secret: &str) -> Result<(), String> {
    keyring_entry(key_id)?
        .set_password(secret)
        .map_err(|e| format!("写入钥匙串失败: {e}"))
}

fn load_keyring(key_id: &str) -> Result<Option<String>, String> {
    match keyring_entry(key_id)?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("读取钥匙串失败: {e}")),
    }
}

fn delete_keyring(key_id: &str) -> Result<(), String> {
    keyring_entry(key_id)?
        .delete_credential()
        .map_err(|e| format!("删除钥匙串条目失败: {e}"))
}

// ---------------------------------------------------------------------------
// 加密文件后端：app_config_dir()/secrets.bin
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct VaultEntry {
    /// base64(nonce, 12 字节，每条独立)
    nonce: String,
    /// base64(AES-256-GCM 密文)
    cipher: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct VaultFile {
    version: u32,
    entries: HashMap<String, VaultEntry>,
}

fn vault_path(dir: &Path) -> PathBuf {
    dir.join("secrets.bin")
}

/// 密钥派生：machine-uid + 固定盐 → SHA-256（32 字节 AES-256 key）
fn derive_key() -> Result<[u8; 32], String> {
    let uid = machine_uid::get().map_err(|e| format!("获取机器标识失败: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(b"picbed-switcher-v1:secret-store:");
    hasher.update(uid.as_bytes());
    Ok(hasher.finalize().into())
}

fn encrypt(secret: &str) -> Result<(String, String), String> {
    let key = derive_key()?;
    let cipher = Aes256Gcm::new((&key).into());
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, secret.as_bytes())
        .map_err(|e| format!("加密失败: {e}"))?;
    Ok((B64.encode(nonce), B64.encode(ct)))
}

fn decrypt(nonce_b64: &str, cipher_b64: &str) -> Result<String, String> {
    let key = derive_key()?;
    let nonce = B64.decode(nonce_b64).map_err(|e| format!("解码 nonce 失败: {e}"))?;
    let cipher = B64.decode(cipher_b64).map_err(|e| format!("解码密文失败: {e}"))?;
    let cipher_obj = Aes256Gcm::new((&key).into());
    let plaintext = cipher_obj
        .decrypt(Nonce::from_slice(&nonce), cipher.as_ref())
        .map_err(|e| format!("解密失败: {e}"))?;
    String::from_utf8(plaintext).map_err(|e| format!("解码明文失败: {e}"))
}

fn load_vault(dir: &Path) -> Result<VaultFile, String> {
    let path = vault_path(dir);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("解析密钥文件失败: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(VaultFile {
            version: 1,
            entries: HashMap::new(),
        }),
        Err(e) => Err(format!("读取密钥文件失败: {e}")),
    }
}

/// tmp + rename 原子写
fn save_vault(dir: &Path, vault: &VaultFile) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    let path = vault_path(dir);
    let text = serde_json::to_vec_pretty(vault).map_err(|e| format!("序列化密钥文件失败: {e}"))?;
    let tmp = path.with_extension("bin.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("写入密钥文件失败: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("保存密钥文件失败: {e}"))?;
    Ok(())
}

fn save_file(dir: &Path, key_id: &str, secret: &str) -> Result<(), String> {
    let (nonce, cipher) = encrypt(secret)?;
    let mut vault = load_vault(dir)?;
    vault.entries.insert(
        key_id.to_string(),
        VaultEntry {
            nonce,
            cipher,
        },
    );
    save_vault(dir, &vault)
}

fn load_file(dir: &Path, key_id: &str) -> Result<Option<String>, String> {
    let vault = load_vault(dir)?;
    match vault.entries.get(key_id) {
        Some(e) => Ok(Some(decrypt(&e.nonce, &e.cipher)?)),
        None => Ok(None),
    }
}

fn delete_file(dir: &Path, key_id: &str) -> Result<(), String> {
    let mut vault = load_vault(dir)?;
    if vault.entries.remove(key_id).is_some() {
        save_vault(dir, &vault)?;
    }
    Ok(())
}
