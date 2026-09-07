use super::AppState;
use crate::{
    account::{model::AccountView, service::QuotaResult},
    error::Result,
};
use tauri::State;
#[tauri::command]
pub async fn refresh_account(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<AccountView> {
    state.service()?.refresh_quota(&account_id).await
}
#[tauri::command]
pub async fn refresh_all_quotas(state: State<'_, AppState>) -> Result<Vec<QuotaResult>> {
    Ok(state.service()?.refresh_all().await)
}
