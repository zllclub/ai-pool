use super::atomic;
use crate::{
    account::model::Account,
    error::{AppError, Result},
};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

pub const CREDENTIALS_FILE: &str = "credentials.v1.json";
const LEGACY_FILE: &str = "credentials.v1.enc";

// Keep the replaceable storage boundary. This implementation intentionally stores plaintext;
// the historical trait name does NOT imply encryption. No OS credential service is accessed.
pub trait SecureStore: Send + Sync {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>>;
    fn save(&self, bytes: &[u8]) -> Result<()>;
}
pub struct PlaintextStore {
    path: PathBuf,
}
impl PlaintextStore {
    pub fn open(path: PathBuf) -> Result<Self> {
        check_path(&path)?;
        Ok(Self { path })
    }
}
fn check_path(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(AppError::new(
                    "UNSAFE_PATH",
                    "凭据库必须为普通文件，不能是符号链接",
                ));
            }
            // Tighten existing user-edited files as well as newly written tempfiles.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| AppError::io(path, e))?;
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(AppError::io(path, e)),
    }
}
impl SecureStore for PlaintextStore {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        check_path(&self.path)?;
        atomic::read_optional(&self.path).map(|bytes| bytes.map(Zeroizing::new))
    }
    fn save(&self, bytes: &[u8]) -> Result<()> {
        // Validate schema before staging. JSON parser errors may contain tokens: never expose them.
        let _: Vec<Account> = serde_json::from_slice(bytes)
            .map_err(|_| AppError::new("JSON_CORRUPT", "凭据库 JSON 结构无效；未修改原文件"))?;
        check_path(&self.path)?;
        atomic::write(&self.path, bytes)
    }
}
pub fn remove_legacy_store(root: &Path) -> Result<()> {
    // Explicitly requested destructive cleanup. Delete only the exact legacy file,
    // never recurse or access Keychain. A symlink is unlinked, not followed.
    let path = root.join(LEGACY_FILE);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(AppError::io(&path, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plaintext_round_trip_and_invalid_write_preserves_file() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(CREDENTIALS_FILE);
        let store = PlaintextStore::open(p.clone()).unwrap();
        assert!(store.load().unwrap().is_none());
        store.save(b"[]").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"[]");
        assert_eq!(&**store.load().unwrap().as_ref().unwrap(), b"[]");
        assert!(store.save(b"{broken").is_err());
        assert!(store.save(b"{}").is_err());
        assert_eq!(std::fs::read(p).unwrap(), b"[]");
    }
    #[test]
    fn legacy_cleanup_is_idempotent_and_preserves_plaintext() {
        let d = tempfile::tempdir().unwrap();
        let legacy = d.path().join(LEGACY_FILE);
        std::fs::write(&legacy, b"opaque-legacy-data").unwrap();
        let store = PlaintextStore::open(d.path().join(CREDENTIALS_FILE)).unwrap();
        store.save(b"[]").unwrap();
        remove_legacy_store(d.path()).unwrap();
        remove_legacy_store(d.path()).unwrap();
        assert!(!legacy.exists());
        assert_eq!(
            std::fs::read(d.path().join(CREDENTIALS_FILE)).unwrap(),
            b"[]"
        );
    }
    #[cfg(unix)]
    #[test]
    fn private_permissions_and_symlink_rejection() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(CREDENTIALS_FILE);
        std::fs::write(&p, b"[]").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        let store = PlaintextStore::open(p.clone()).unwrap();
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        store.save(b"[]").unwrap();
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let link = d.path().join("link.json");
        symlink(&p, &link).unwrap();
        assert!(PlaintextStore::open(link).is_err());
    }
}
