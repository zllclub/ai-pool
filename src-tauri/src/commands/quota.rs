use super::{notify_account_data_changed, AppState};
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
) -> Result<AccountView> {
    let account = state.service()?.refresh_quota(&account_id).await?;
    notify_account_data_changed(&app, Some(&account_id));
    Ok(account)
}
#[tauri::command]
pub async fn refresh_all_quotas(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<QuotaResult>> {
    let results = state.service()?.refresh_all().await;
    notify_account_data_changed(&app, None);
    Ok(results)
}
