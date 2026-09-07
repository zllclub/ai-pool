use super::AppState;
use crate::error::Result;
use tauri::State;
#[tauri::command]
pub async fn switch_codex_account(state: State<'_, AppState>, account_id: String) -> Result<()> {
    state.service()?.switch(&account_id, &[0]).await
}
#[tauri::command]
pub async fn switch_pi_account(state: State<'_, AppState>, account_id: String) -> Result<()> {
    state.service()?.switch(&account_id, &[1]).await
}
#[tauri::command]
pub async fn switch_both(state: State<'_, AppState>, account_id: String) -> Result<()> {
    state.service()?.switch(&account_id, &[0, 1]).await
}
