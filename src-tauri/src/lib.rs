mod account;
mod commands;
mod error;
mod oauth;
mod quota;
mod runtime;
mod storage;
mod switch;
#[cfg(test)]
mod tests;
use account::{repository::Repository, service::AccountService};
use commands::{account::*, quota::*, switch::*, widget::*, AppState};
use error::{AppError, Result};
use std::{sync::Arc, time::Duration};
use tauri::Manager;

fn initialize(app: &tauri::App) -> Result<Arc<AccountService>> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::new("APP_DATA", "找不到应用数据目录"))?;
    storage::atomic::private_dir(&root)?;
    let lock_path = root.join("manager.lock");
    let instance = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| AppError::io(&lock_path, e))?;
    fs2::FileExt::try_lock_exclusive(&instance)
        .map_err(|_| AppError::new("INSTANCE_BUSY", "另一个账号管理器正在运行，请关闭重复窗口"))?;
    let store = storage::secure_store::PlaintextStore::open(
        root.join(storage::secure_store::CREDENTIALS_FILE),
    )?;
    let repo = Repository::new(Box::new(store))?;
    storage::secure_store::remove_legacy_store(&root)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("codex-account-pool/0.1.0")
        .build()
        .map_err(|_| AppError::new("HTTP_INIT", "无法初始化 HTTP 客户端"))?;
    let paths = storage::config::Paths::local()?;
    let quota = Box::new(quota::codex::CodexQuotaProvider {
        client: client.clone(),
    });
    let runtimes: Vec<Box<dyn runtime::RuntimeAdapter>> = vec![
        Box::new(runtime::codex::CodexAdapter { path: paths.codex }),
        Box::new(runtime::pi_agent::PiAdapter { path: paths.pi }),
    ];
    Ok(Arc::new(AccountService::new(
        repo, client, quota, runtimes, instance,
    )))
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let result = initialize(app);
            app.manage(AppState(result));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_accounts,
            start_oauth_login,
            cancel_oauth_login,
            delete_account,
            refresh_account,
            refresh_all_quotas,
            switch_codex_account,
            switch_pi_account,
            switch_both,
            get_runtime_status,
            import_current_codex_account,
            import_current_pi_account,
            show_account_widget,
            close_account_widget,
            set_widget_expanded,
            snap_account_widget
        ])
        .run(tauri::generate_context!())
        .expect("Tauri runtime initialization failed");
}
