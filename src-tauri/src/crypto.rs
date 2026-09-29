use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};

const KEY_SEED: &[u8] = b"picbed-switcher:v1:config-secret:aes-gcm";

fn normalize_key() -> [u8; 32] {
    Sha256::digest(KEY_SEED).into()
}

pub fn encrypt_string(plaintext: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(&normalize_key()).map_err(|e| e.to_string())?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let sealed = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(12 + sealed.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&sealed);
    Ok(BASE64.encode(out))
}

pub fn decrypt_string(ciphertext: &str) -> Result<String, String> {
    let raw = BASE64
        .decode(ciphertext.as_bytes())
        .map_err(|_| "invalid ciphertext".to_string())?;
    if raw.len() < 12 {
        return Err("invalid ciphertext".to_string());
    }
    let cipher = Aes256Gcm::new_from_slice(&normalize_key()).map_err(|e| e.to_string())?;
    let plain = cipher
        .decrypt(Nonce::from_slice(&raw[..12]), &raw[12..])
        .map_err(|_| "invalid ciphertext".to_string())?;
    Ok(String::from_utf8_lossy(&plain).to_string())
}
