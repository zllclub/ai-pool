use super::AppState;
use crate::{
    account::model::AccountView,
    error::{AppError, Result},
    runtime::RuntimeStatus,
};
use serde::Serialize;
use tauri::State;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResult {
    pub account: AccountView,
    pub quota_error: Option<AppError>,
}
#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<AccountView>> {
    Ok(state.service()?.list().await)
}
#[tauri::command]
pub async fn start_oauth_login(
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> Result<LoginResult> {
    let s = state.service()?;
    if let Some(id) = &account_id {
        s.repo.get(id).await?;
    }
    let account = s.login.login(&s.client).await?;
    let account = s.add(account, account_id).await?;
    // A quota outage must never discard a successful OAuth login.
    match s.refresh_quota(&account.id).await {
        Ok(account) => Ok(LoginResult {
            account,
            quota_error: None,
        }),
        Err(e) => Ok(LoginResult {
            account,
            quota_error: Some(e),
        }),
    }
}
#[tauri::command]
pub async fn cancel_oauth_login(state: State<'_, AppState>) -> Result<()> {
    state.service()?.login.cancel().await;
    Ok(())
}
#[tauri::command]
pub async fn delete_account(state: State<'_, AppState>, account_id: String) -> Result<()> {
    state.service()?.delete(&account_id).await
}
#[tauri::command]
pub async fn get_runtime_status(state: State<'_, AppState>) -> Result<Vec<RuntimeStatus>> {
    Ok(state.service()?.status().await)
}
#[tauri::command]
pub async fn import_current_codex_account(state: State<'_, AppState>) -> Result<AccountView> {
    state.service()?.import(0).await
}
#[tauri::command]
pub async fn import_current_pi_account(state: State<'_, AppState>) -> Result<AccountView> {
    state.service()?.import(1).await
}
