//! Offline integration tests. Only tempfile directories are touched; never the user's auth files.
use crate::{
    account::{model::*, repository::Repository, service::AccountService},
    error::{AppError, Result},
    oauth::{protocol, refresh::TokenRefresher},
    quota::provider::QuotaProvider,
    runtime::{codex::CodexAdapter, pi_agent::PiAdapter},
    storage::{atomic, secure_store::SecureStore},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use zeroize::Zeroizing;

struct MemoryStore {
    fail: Arc<AtomicBool>,
}
impl SecureStore for MemoryStore {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        Ok(None)
    }
    fn save(&self, _: &[u8]) -> Result<()> {
        if self.fail.load(Ordering::SeqCst) {
            Err(AppError::new("DISK_FULL", "test"))
        } else {
            Ok(())
        }
    }
}
struct MockQuota;
#[async_trait::async_trait]
impl QuotaProvider for MockQuota {
    async fn get_quota(&self, _: &Account) -> Result<(AccountQuota, Option<String>)> {
        Ok((
            AccountQuota {
                five_hour_remaining: Some(65.),
                weekly_remaining: Some(82.),
                updated_at: now(),
                ..Default::default()
            },
            Some("plus".into()),
        ))
    }
}
struct MockRefresh {
    count: Arc<AtomicUsize>,
}
#[async_trait::async_trait]
impl TokenRefresher for MockRefresh {
    async fn refresh(&self, old: &Credentials) -> Result<Credentials> {
        self.count.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let id = protocol::account(old.clone(), None)?.view.account_id;
        Ok(credentials(&id, false))
    }
}
fn credentials(id: &str, expired: bool) -> Credentials {
    let expiry = if expired {
        now() - 10_000
    } else {
        now() + 3_600_000
    };
    let payload = json!({"exp":expiry/1000,"iat":if expired{1}else{now()/1000},"https://api.openai.com/auth":{"chatgpt_account_id":id},"email":format!("{id}@example.test")});
    Credentials {
        access_token: format!(
            "e30.{}.signature",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap())
        ),
        refresh_token: if expired { "old" } else { "rotated" }.into(),
        id_token: Some("test-id-token".into()),
        expires_at: expiry,
    }
}
fn account(id: &str, expired: bool) -> Account {
    let mut a = protocol::account(credentials(id, expired), None).unwrap();
    a.view.id = id.into();
    a
}
async fn setup(
    expired: bool,
) -> (
    tempfile::TempDir,
    AccountService,
    Arc<AtomicUsize>,
    Arc<AtomicBool>,
) {
    let dir = tempfile::tempdir().unwrap();
    let fail = Arc::new(AtomicBool::new(false));
    let repo = Repository::new(Box::new(MemoryStore { fail: fail.clone() })).unwrap();
    repo.update(|all| {
        all.extend([account("a", expired), account("b", false)]);
        Ok(())
    })
    .await
    .unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let mut service = AccountService::new(
        repo,
        reqwest::Client::new(),
        Box::new(MockQuota),
        vec![
            Box::new(CodexAdapter {
                path: dir.path().join("codex/auth.json"),
            }),
            Box::new(PiAdapter {
                path: dir.path().join("pi/auth.json"),
            }),
        ],
        std::fs::File::create(dir.path().join("instance")).unwrap(),
    );
    service.refresher = Box::new(MockRefresh {
        count: count.clone(),
    });
    (dir, service, count, fail)
}
#[tokio::test]
async fn concurrent_queries_refresh_only_once() {
    let (_d, s, count, _) = setup(true).await;
    let (a, b) = tokio::join!(s.refresh_quota("a"), s.refresh_quota("a"));
    assert!(a.is_ok() && b.is_ok());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        s.repo.get("a").await.unwrap().credentials.refresh_token,
        "rotated"
    );
    assert!(!s.runtimes[0].path().exists());
    assert!(!s.runtimes[1].path().exists());
}
#[tokio::test]
async fn failed_rotation_save_never_reuses_old_refresh_token() {
    let (_d, s, count, fail) = setup(true).await;
    fail.store(true, Ordering::SeqCst);
    assert_eq!(
        s.refresh_quota("a").await.unwrap_err().code,
        "ROTATION_SAVE_FAILED"
    );
    fail.store(false, Ordering::SeqCst);
    s.refresh_quota("a").await.unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        s.repo.get("a").await.unwrap().credentials.refresh_token,
        "rotated"
    );
}
#[tokio::test]
async fn invalid_second_file_keeps_first_unchanged() {
    let (_d, s, _, _) = setup(false).await;
    s.switch("a", &[0]).await.unwrap();
    let before = std::fs::read(s.runtimes[0].path()).unwrap();
    atomic::write(s.runtimes[1].path(), b"{corrupt").unwrap();
    assert_eq!(
        s.switch("b", &[0, 1]).await.unwrap_err().code,
        "JSON_CORRUPT"
    );
    assert_eq!(std::fs::read(s.runtimes[0].path()).unwrap(), before);
}
#[tokio::test]
async fn concurrent_switches_remain_consistent_and_preserve_providers() {
    let (_d, s, _, _) = setup(false).await;
    atomic::write(
        s.runtimes[1].path(),
        br#"{"anthropic":{"type":"api_key","key":"fixture"}}"#,
    )
    .unwrap();
    let (a, b) = tokio::join!(s.switch("a", &[0, 1]), s.switch("b", &[0, 1]));
    assert!(a.is_ok() && b.is_ok());
    let status = s.status().await;
    assert_eq!(status[0].managed_id, status[1].managed_id);
    let pi = s.runtimes[1].read().unwrap();
    assert_eq!(pi.value["anthropic"]["key"], "fixture");
}
#[tokio::test]
async fn independent_runtime_accounts_and_delete_becomes_unmanaged() {
    let (_d, s, _, _) = setup(false).await;
    s.switch("a", &[0]).await.unwrap();
    s.switch("b", &[1]).await.unwrap();
    let status = s.status().await;
    assert_eq!(status[0].managed_id.as_deref(), Some("a"));
    assert_eq!(status[1].managed_id.as_deref(), Some("b"));
    assert_eq!(s.import(0).await.unwrap_err().code, "ACCOUNT_EXISTS");
    s.delete("a").await.unwrap();
    let status = s.status().await;
    assert_eq!(status[0].managed_id, None);
    assert_eq!(status[0].account_id.as_deref(), Some("a"));
}
#[tokio::test]
async fn incoming_cli_rotation_is_saved_without_network_refresh() {
    let (_d, s, count, _) = setup(true).await;
    let external = account("a", false);
    let json = s.runtimes[0].apply_account(&json!({}), &external).unwrap();
    atomic::write(s.runtimes[0].path(), &serde_json::to_vec(&json).unwrap()).unwrap();
    s.refresh_quota("a").await.unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(
        s.repo.get("a").await.unwrap().credentials.refresh_token,
        "rotated"
    );
}
#[tokio::test]
async fn dto_cannot_leak_credentials_and_failed_delete_is_rolled_back() {
    let (_d, s, _, fail) = setup(false).await;
    let text = serde_json::to_string(&s.list().await).unwrap();
    assert!(
        !text.contains("accessToken")
            && !text.contains("refreshToken")
            && !text.contains("idToken")
    );
    fail.store(true, Ordering::SeqCst);
    assert!(s.delete("a").await.is_err());
    assert!(s.repo.get("a").await.is_ok());
}
