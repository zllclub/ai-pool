use crate::error::{AppError, Result};
use std::{fs, io::Write, path::Path};

pub fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    if let Ok(m) = fs::symlink_metadata(path) {
        if m.file_type().is_symlink() {
            return Err(AppError::new("UNSAFE_PATH", "拒绝读写符号链接认证文件"));
        }
    }
    match fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AppError::io(path, e)),
    }
}
pub fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| AppError::io(path, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|e| AppError::io(path, e))?;
    }
    Ok(())
}
// Stage on the same filesystem; fsync before rename. Tempfiles are 0600 on Unix.
pub fn stage(path: &Path, bytes: &[u8]) -> Result<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new("FILE_PATH", "无效文件路径"))?;
    if !parent.exists() {
        private_dir(parent)?;
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| AppError::io(path, e))?;
    temp.write_all(bytes)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|e| AppError::io(path, e))?;
    Ok(temp)
}
pub fn commit(path: &Path, temp: tempfile::NamedTempFile) -> Result<()> {
    temp.persist(path)
        .map_err(|e| AppError::io(path, e.error))?;
    // Rename already committed; directory sync is best effort (unsupported on some OS).
    #[cfg(unix)]
    {
        if let Some(p) = path.parent() {
            if let Ok(f) = fs::File::open(p) {
                let _ = f.sync_all();
            }
        }
    }
    Ok(())
}
pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    commit(path, stage(path, bytes)?)
}
pub fn json(bytes: &[u8]) -> Result<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| AppError::new("JSON_CORRUPT", "JSON 内容损坏；未修改原文件"))?;
    if !v.is_object() {
        return Err(AppError::new("JSON_CORRUPT", "认证文件必须是 JSON 对象"));
    }
    Ok(v)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_replacement() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("auth.json");
        write(&p, b"old").unwrap();
        let t = stage(&p, b"new").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"old");
        commit(&p, t).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"new");
    }
    #[test]
    fn broken_json_rejected() {
        assert!(json(b"{broken").is_err());
        assert!(json(b"[]").is_err());
    }
}
