pub mod codex;
pub mod pi_agent;
use crate::{
    account::model::Account,
    error::{AppError, Result},
    storage::atomic,
};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

pub trait RuntimeAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn path(&self) -> &Path;
    fn detect_current_account(&self, value: &Value) -> Result<Option<Account>>;
    fn apply_account(&self, original: &Value, account: &Account) -> Result<Value>;
    fn read(&self) -> Result<Snapshot> {
        let bytes = atomic::read_optional(self.path())?;
        let value = match &bytes {
            Some(b) => atomic::json(b)?,
            None => serde_json::json!({}),
        };
        let account = self.detect_current_account(&value)?;
        Ok(Snapshot {
            bytes,
            value,
            account,
        })
    }
}
pub struct Snapshot {
    pub bytes: Option<Vec<u8>>,
    pub value: Value,
    pub account: Option<Account>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub runtime: String,
    pub path: String,
    pub account_id: Option<String>,
    pub managed_id: Option<String>,
    pub error: Option<AppError>,
}
pub fn required(v: &Value, key: &str) -> Result<String> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| AppError::new("RUNTIME_SCHEMA", "认证文件缺少必要 OAuth 字段"))
}
