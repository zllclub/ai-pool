use crate::{
    account::model::{now, AccountView},
    error::{AppError, Result},
    storage::atomic,
};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use tauri::{Emitter, Manager};

const LOGS_FILE: &str = "quota-request-logs.v1.json";
const RETENTION_MILLIS: i64 = 24 * 60 * 60 * 1000;
static LOG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QuotaRequestReason {
    Manual,
    ManualAll,
    Automatic,
    LowQuotaAutomatic,
    AppStartup,
    WidgetManual,
    AutoSwitchCheck,
    Import,
    OauthLogin,
}

impl Default for QuotaRequestReason {
    fn default() -> Self {
        Self::Manual
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaRequestLog {
    pub timestamp: i64,
    pub account_id: String,
    pub account_label: String,
    pub reason: QuotaRequestReason,
    pub success: bool,
    pub error_code: Option<String>,
}

fn logs_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .map(|path| path.join(LOGS_FILE))
        .map_err(|_| AppError::new("APP_DATA", "找不到应用数据目录"))
}

fn read_logs(app: &tauri::AppHandle) -> Result<Vec<QuotaRequestLog>> {
    match atomic::read_optional(&logs_path(app)?)? {
        Some(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| AppError::new("REQUEST_LOG_CORRUPT", "请求日志文件损坏")),
        None => Ok(Vec::new()),
    }
}

pub fn record(
    app: &tauri::AppHandle,
    account: &AccountView,
    reason: QuotaRequestReason,
    error: Option<&AppError>,
) {
    let Ok(_guard) = LOG_LOCK.get_or_init(|| Mutex::new(())).lock() else {
        return;
    };
    let cutoff = now() - RETENTION_MILLIS;
    let mut logs = read_logs(app).unwrap_or_default();
    logs.retain(|log| log.timestamp >= cutoff);
    logs.push(QuotaRequestLog {
        timestamp: now(),
        account_id: account.id.clone(),
        account_label: account
            .email
            .clone()
            .unwrap_or_else(|| account.account_id.clone()),
        reason,
        success: error.is_none(),
        error_code: error.map(|value| value.code.clone()),
    });
    if let (Ok(bytes), Ok(path)) = (serde_json::to_vec(&logs), logs_path(app)) {
        if atomic::write(&path, &bytes).is_ok() {
            let _ = app.emit_to("main", "quota-request-logged", ());
        }
    }
}

#[tauri::command]
pub fn list_quota_request_logs(app: tauri::AppHandle) -> Result<Vec<QuotaRequestLog>> {
    let _guard = LOG_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| AppError::new("REQUEST_LOG_LOCK", "无法读取请求日志"))?;
    let cutoff = now() - RETENTION_MILLIS;
    let mut logs = read_logs(&app)?;
    logs.retain(|log| log.timestamp >= cutoff);
    logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(logs)
}
