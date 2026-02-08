use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, AeadCore, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;

const HKDF_SALT: &[u8] = b"webhooker-v1";
const HKDF_INFO: &[u8] = b"aes256gcm-key";

fn derive_key(key: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), key.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(HKDF_INFO, &mut okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    okm
}

/// Encrypt plaintext using AES-256-GCM. Returns nonce (12 bytes) prepended to ciphertext.
pub fn encrypt(plaintext: &str, key: &str) -> Result<Vec<u8>, String> {
    let key_bytes = derive_key(key);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("Invalid key: {e}"))?;

    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let mut result = nonce.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Decrypt ciphertext (nonce prepended) using AES-256-GCM.
pub fn decrypt(data: &[u8], key: &str) -> Result<String, String> {
    if data.len() < 12 {
        return Err("Ciphertext too short".to_string());
    }

    let key_bytes = derive_key(key);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("Invalid key: {e}"))?;

    let nonce = Nonce::from_slice(&data[..12]);
    let plaintext = cipher
        .decrypt(nonce, &data[12..])
        .map_err(|e| format!("Decryption failed: {e}"))?;

    String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8: {e}"))
}
