//! AES-256-GCM 加解密（E2E 同步 F-21 与加密备份预留）。
//!
//! 格式：base64( nonce(12B) || ciphertext+tag )。

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key};
use base64::Engine;

use crate::error::AppError;

/// 生成随机 256-bit 密钥（hex 编码，便于经钥匙串存储）。
pub fn generate_key_hex() -> String {
    let key = Aes256Gcm::generate_key(OsRng);
    crate::db::hex_encode(&key)
}

/// 加密：输入 hex 密钥与明文，输出 base64 密文包。
pub fn encrypt(key_hex: &str, plaintext: &[u8]) -> Result<String, AppError> {
    let key_bytes = decode_hex(key_hex)?;
    if key_bytes.len() != 32 {
        return Err(AppError::internal("AES-256 密钥长度必须为 32 字节"));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let nonce = Aes256Gcm::generate_nonce(OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| AppError::internal(format!("加密失败: {e}")))?;
    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&ciphertext);
    Ok(base64::engine::general_purpose::STANDARD.encode(blob))
}

/// 解密：输入 hex 密钥与 base64 密文包，输出明文。
pub fn decrypt(key_hex: &str, blob_b64: &str) -> Result<Vec<u8>, AppError> {
    let key_bytes = decode_hex(key_hex)?;
    // 与 encrypt 对称：先校验密钥长度，避免 GenericArray::from_slice panic
    if key_bytes.len() != 32 {
        return Err(AppError::invalid_param("AES-256 密钥长度必须为 32 字节"));
    }
    let blob = base64::engine::general_purpose::STANDARD
        .decode(blob_b64)
        .map_err(|e| AppError::internal(format!("密文 base64 解码失败: {e}")))?;
    if blob.len() < 12 {
        return Err(AppError::internal("密文长度非法"));
    }
    let (nonce_bytes, ciphertext) = blob.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let nonce = aes_gcm::Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| AppError::internal(format!("解密失败: {e}")))
}

/// hex 解码。
fn decode_hex(hex: &str) -> Result<Vec<u8>, AppError> {
    if hex.len() % 2 != 0 {
        return Err(AppError::internal("hex 长度非法"));
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|e| AppError::internal(format!("hex 解码失败: {e}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let key = generate_key_hex();
        let blob = encrypt(&key, "学生错题内容 secret".as_bytes()).unwrap();
        let plain = decrypt(&key, &blob).unwrap();
        assert_eq!(plain, "学生错题内容 secret".as_bytes());
    }

    #[test]
    fn ciphertext_contains_no_plaintext() {
        let key = generate_key_hex();
        let blob = encrypt(&key, b"top-secret-payload").unwrap();
        assert!(!blob.contains("top-secret"), "密文不得含明文片段");
    }

    #[test]
    fn wrong_key_fails() {
        let k1 = generate_key_hex();
        let k2 = generate_key_hex();
        let blob = encrypt(&k1, b"data").unwrap();
        assert!(decrypt(&k2, &blob).is_err(), "错误密钥必须解密失败（GCM 认证）");
    }

    #[test]
    fn tampered_ciphertext_fails() {
        use base64::engine::general_purpose::STANDARD;
        let key = generate_key_hex();
        let blob = encrypt(&key, b"integrity matters").unwrap();
        let mut bytes = STANDARD.decode(&blob).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01; // 翻转最后一字节
        let tampered = STANDARD.encode(&bytes);
        assert!(decrypt(&key, &tampered).is_err(), "篡改密文必须被 GCM 检出");
    }

    #[test]
    fn encrypt_rejects_bad_key_length() {
        let short = crate::db::hex_encode(&[0u8; 16]);
        assert!(encrypt(&short, b"x").is_err(), "128-bit 密钥应被拒绝");
    }

    /// 已知缺陷复现（已路由工程师修复）：decrypt 缺少密钥长度校验，
    /// `Key::from_slice` 在非 32 字节密钥上 panic 而非返回 Err。
    #[test]
    fn decrypt_rejects_bad_key_length_gracefully() {
        let short = crate::db::hex_encode(&[0u8; 16]);
        let blob = encrypt(&generate_key_hex(), b"x").unwrap();
        let result = std::panic::catch_unwind(|| decrypt(&short, &blob));
        match result {
            Ok(Err(_)) => {} // 修复后的正确行为
            Ok(Ok(_)) => panic!("错误长度密钥不应解密成功"),
            Err(_) => panic!("缺陷复现：decrypt 在短密钥上 panic（应返回 Err）"),
        }
    }

    #[test]
    fn invalid_base64_rejected() {
        let key = generate_key_hex();
        assert!(decrypt(&key, "%%%not-base64%%%").is_err());
    }
}
