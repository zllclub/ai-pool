use super::{notify_account_data_changed, AppState};
use crate::error::Result;
use tauri::{AppHandle, State};

async fn switch_and_notify(
    app: &AppHandle,
    state: &State<'_, AppState>,
    account_id: &str,
    targets: &[usize],
) -> Result<()> {
    state.service()?.switch(account_id, targets).await?;
    notify_account_data_changed(app, None);
    Ok(())
}

#[tauri::command]
pub async fn switch_codex_account(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
) -> Result<()> {
    switch_and_notify(&app, &state, &account_id, &[0]).await
}
#[tauri::command]
pub async fn switch_pi_account(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
) -> Result<()> {
    switch_and_notify(&app, &state, &account_id, &[1]).await
}
#[tauri::command]
pub async fn switch_both(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
) -> Result<()> {
    switch_and_notify(&app, &state, &account_id, &[0, 1]).await
}
