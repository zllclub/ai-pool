use crate::error::{AppError, Result};
use std::path::PathBuf;
// UI configuration belongs here, never in the encrypted credential vault.
pub struct Paths {
    pub codex: PathBuf,
    pub pi: PathBuf,
}
impl Paths {
    pub fn local() -> Result<Self> {
        let home =
            dirs::home_dir().ok_or_else(|| AppError::new("HOME_MISSING", "找不到用户目录"))?;
        Ok(Self {
            codex: home.join(".codex/auth.json"),
            pi: home.join(".pi/agent/auth.json"),
        })
    }
}
