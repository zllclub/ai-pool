pub mod account;
pub mod quota;
pub mod request_log;
pub mod switch;
pub mod widget;
use crate::{account::service::AccountService, error::Result};
use std::sync::Arc;
use tauri::Emitter;

pub const ACCOUNT_DATA_CHANGED: &str = "account-data-changed";
pub fn notify_account_data_changed(app: &tauri::AppHandle, account_id: Option<&str>) {
    // The repository is authoritative; listeners reload their own credential-free DTOs.
    let _ = app.emit(ACCOUNT_DATA_CHANGED, account_id.map(String::from));
}

pub struct AppState(pub Result<Arc<AccountService>>);
impl AppState {
    pub fn service(&self) -> Result<&AccountService> {
        self.0.as_ref().map(|s| s.as_ref()).map_err(Clone::clone)
    }
}
