use super::{
    notify_account_data_changed,
    request_log::{self, QuotaRequestReason},
    AppState,
};
use crate::{
    account::{model::AccountView, service::QuotaResult},
    error::Result,
};
use tauri::{AppHandle, State};
#[tauri::command]
pub async fn refresh_account(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
    reason: Option<QuotaRequestReason>,
) -> Result<AccountView> {
    let service = state.service()?;
    let account = service
        .list()
        .await
        .into_iter()
        .find(|account| account.id == account_id)
        .ok_or_else(|| crate::error::AppError::new("ACCOUNT_NOT_FOUND", "账号不存在"))?;
    let result = service.refresh_quota(&account_id).await;
    request_log::record(
        &app,
        &account,
        reason.unwrap_or_default(),
        result.as_ref().err(),
    );
    if result.is_ok() {
        notify_account_data_changed(&app, Some(&account_id));
    }
    result
}
#[tauri::command]
pub async fn refresh_all_quotas(
    app: AppHandle,
    state: State<'_, AppState>,
    reason: Option<QuotaRequestReason>,
) -> Result<Vec<QuotaResult>> {
    let service = state.service()?;
    let accounts = service.list().await;
    let results = service.refresh_all().await;
    let reason = reason.unwrap_or_default();
    for result in &results {
        if let Some(account) = accounts
            .iter()
            .find(|account| account.id == result.account_id)
        {
            request_log::record(&app, account, reason, result.error.as_ref());
        }
    }
    notify_account_data_changed(&app, None);
    Ok(results)
}
