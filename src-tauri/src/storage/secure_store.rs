use super::atomic;
use crate::error::{AppError, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};
use std::path::PathBuf;
use zeroize::Zeroizing;

// Replaceable storage boundary: encrypted vault, master key held by OS credential manager.
// No silent plaintext fallback when Keychain/Secret Service is unavailable.
pub trait SecureStore: Send + Sync {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>>;
    fn save(&self, bytes: &[u8]) -> Result<()>;
}
pub struct EncryptedStore {
    path: PathBuf,
    key: Zeroizing<Vec<u8>>,
}
fn key_error() -> AppError {
    AppError::new(
        "SECURE_STORE",
        "系统凭据存储不可用或已锁定；请解锁 Keychain / Credential Manager / Secret Service",
    )
}
impl EncryptedStore {
    pub fn open(path: PathBuf) -> Result<Self> {
        let entry = keyring::Entry::new("dev.local.codex-accounts", "vault-key-v1")
            .map_err(|_| key_error())?;
        let key = match entry.get_password() {
            Ok(s) => {
                let s = Zeroizing::new(s);
                STANDARD.decode(s.as_bytes()).map_err(|_| key_error())?
            }
            Err(keyring::Error::NoEntry) if !path.exists() => {
                let mut k = vec![0; 32];
                OsRng.fill_bytes(&mut k);
                let encoded = Zeroizing::new(STANDARD.encode(&k));
                entry.set_password(&encoded).map_err(|_| key_error())?;
                k
            }
            _ => return Err(key_error()),
        };
        if key.len() != 32 {
            return Err(key_error());
        }
        Ok(Self {
            path,
            key: Zeroizing::new(key),
        })
    }
}
impl SecureStore for EncryptedStore {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        let Some(bytes) = atomic::read_optional(&self.path)? else {
            return Ok(None);
        };
        if bytes.len() < 24 {
            return Err(AppError::new("VAULT_CORRUPT", "凭据库损坏"));
        }
        let cipher = XChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| key_error())?;
        let plain = cipher
            .decrypt(XNonce::from_slice(&bytes[..24]), &bytes[24..])
            .map_err(|_| AppError::new("VAULT_CORRUPT", "凭据库无法解密；请勿删除系统密钥"))?;
        Ok(Some(Zeroizing::new(plain)))
    }
    fn save(&self, bytes: &[u8]) -> Result<()> {
        let mut nonce = [0; 24];
        OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new_from_slice(&self.key).map_err(|_| key_error())?;
        let encrypted = cipher
            .encrypt(XNonce::from_slice(&nonce), bytes)
            .map_err(|_| key_error())?;
        let mut out = nonce.to_vec();
        out.extend(encrypted);
        atomic::write(&self.path, &out)
    }
}
