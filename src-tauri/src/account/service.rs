use super::{model::*, repository::Repository};
use crate::{
    error::{AppError, Result},
    oauth::{login::LoginManager, protocol, refresh},
    quota::provider::QuotaProvider,
    runtime::{RuntimeAdapter, RuntimeStatus},
};
use futures::{stream, StreamExt};
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, OwnedMutexGuard};

pub struct AccountService {
    pub repo: Repository,
    pub client: reqwest::Client,
    pub quota: Box<dyn QuotaProvider>,
    pub refresher: Box<dyn refresh::TokenRefresher>,
    // If disk persistence fails after rotation, retain the new token and retry saving, NOT refresh.
    pending_rotation: Mutex<HashMap<String, Account>>,
    pub runtimes: Vec<Box<dyn RuntimeAdapter>>,
    pub switch_lock: Mutex<()>,
    token_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    pub login: LoginManager,
    // Process-wide advisory lock: a second manager instance must not access this vault.
    pub _instance_lock: std::fs::File,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaResult {
    pub account_id: String,
    pub error: Option<AppError>,
}
impl AccountService {
    pub fn new(
        repo: Repository,
        client: reqwest::Client,
        quota: Box<dyn QuotaProvider>,
        runtimes: Vec<Box<dyn RuntimeAdapter>>,
        instance: std::fs::File,
    ) -> Self {
        Self {
            repo,
            refresher: Box::new(refresh::OpenAiRefresher(client.clone())),
            pending_rotation: Mutex::new(HashMap::new()),
            client,
            quota,
            runtimes,
            switch_lock: Mutex::new(()),
            token_locks: Mutex::new(HashMap::new()),
            login: LoginManager::default(),
            _instance_lock: instance,
        }
    }
    pub async fn token_lock(&self, id: &str) -> OwnedMutexGuard<()> {
        let lock = self
            .token_locks
            .lock()
            .await
            .entry(id.into())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        lock.lock_owned().await
    }
    pub async fn list(&self) -> Vec<AccountView> {
        self.repo.all().await.into_iter().map(|a| a.view).collect()
    }
    pub async fn status(&self) -> Vec<RuntimeStatus> {
        let all = self.repo.all().await;
        self.runtimes
            .iter()
            .map(|r| {
                let (account_id, error) = match r.read() {
                    Ok(s) => (s.account.map(|a| a.view.account_id), None),
                    Err(e) => (None, Some(e)),
                };
                let managed_id = all
                    .iter()
                    .find(|a| Some(&a.view.account_id) == account_id.as_ref())
                    .map(|a| a.view.id.clone());
                RuntimeStatus {
                    runtime: r.name().into(),
                    path: r.path().display().to_string(),
                    account_id,
                    managed_id,
                    error,
                }
            })
            .collect()
    }
    // All callers hold token_lock. Only strictly newer local tokens can supersede vault data.
    // Comparing JWT iat (when present) prevents expires_in rounding from reimporting stale tokens.
    pub async fn reconcile_locked(&self, mut a: Account) -> Result<Account> {
        for r in &self.runtimes {
            let snapshot = match r.read() {
                Ok(v) => v,
                Err(_) => continue,
            }; // unrelated damaged runtimes must not block quota queries
            if let Some(local) = snapshot.account {
                if local.view.account_id != a.view.account_id
                    || (local.credentials.access_token == a.credentials.access_token
                        && local.credentials.refresh_token == a.credentials.refresh_token)
                {
                    continue;
                }
                let issued = |c: &Credentials| {
                    protocol::claims(&c.access_token)
                        .ok()
                        .and_then(|v| v["iat"].as_i64())
                        .unwrap_or(c.expires_at / 1000)
                };
                if issued(&local.credentials) > issued(&a.credentials) {
                    let mut c = local.credentials.clone();
                    if c.id_token.is_none() {
                        c.id_token = a.credentials.id_token.clone();
                    }
                    a.credentials = c;
                    self.repo.put(a.clone()).await?;
                } else if issued(&local.credentials) == issued(&a.credentials)
                    && local.credentials.refresh_token != a.credentials.refresh_token
                {
                    return Err(AppError::new(
                        "RUNTIME_DESYNC",
                        "同账号存在无法判定新旧的凭据；请停止客户端后重新授权",
                    ));
                }
            }
        }
        Ok(a)
    }
    pub async fn fresh_locked(&self, id: &str, force: bool) -> Result<Account> {
        let pending = self.pending_rotation.lock().await.get(id).cloned();
        if let Some(a) = pending {
            self.repo.put(a).await?;
            self.pending_rotation.lock().await.remove(id);
        }
        let mut a = self.reconcile_locked(self.repo.get(id).await?).await?;
        if force || a.credentials.expires_at <= now() + 120_000 {
            let c = self.refresher.refresh(&a.credentials).await?;
            let identity = protocol::account(c.clone(), Some(&a.view.account_id))?;
            a.credentials = c;
            if identity.view.email.is_some() {
                a.view.email = identity.view.email;
            }
            if identity.view.plan_type.is_some() {
                a.view.plan_type = identity.view.plan_type;
            }
            // Save before releasing the lock. Never let another request reuse the old refresh token.
            self.pending_rotation
                .lock()
                .await
                .insert(id.into(), a.clone());
            self.repo.put(a.clone()).await.map_err(|_| {
                AppError::new(
                    "ROTATION_SAVE_FAILED",
                    "Token 已轮换但凭据库保存失败；请勿退出应用，修复磁盘权限/空间后重试",
                )
            })?;
            self.pending_rotation.lock().await.remove(id);
        }
        Ok(a)
    }
    pub async fn refresh_quota(&self, id: &str) -> Result<AccountView> {
        let _lock = self.token_lock(id).await;
        let mut a = self.fresh_locked(id, false).await?;
        let result = self.quota.get_quota(&a).await;
        let (quota, plan) = match result {
            Err(e) if e.code == "TOKEN_EXPIRED" => {
                a = self.fresh_locked(id, true).await?;
                self.quota.get_quota(&a).await?
            }
            other => other?,
        };
        a.view.quota = Some(quota);
        if plan.is_some() {
            a.view.plan_type = plan;
        }
        self.repo.put(a.clone()).await?;
        Ok(a.view)
    }
    pub async fn refresh_all(&self) -> Vec<QuotaResult> {
        let all = self.list().await;
        stream::iter(all)
            .map(|a| async move {
                QuotaResult {
                    error: self.refresh_quota(&a.id).await.err(),
                    account_id: a.id,
                }
            })
            .buffer_unordered(4)
            .collect()
            .await
    }
    pub async fn add(&self, mut a: Account, replace: Option<String>) -> Result<AccountView> {
        let _switch = self.switch_lock.lock().await;
        if let Some(id) = replace {
            let _lock = self.token_lock(&id).await;
            let old = self.repo.get(&id).await?;
            if old.view.account_id != a.view.account_id {
                return Err(AppError::new(
                    "REAUTHORIZE_MISMATCH",
                    "重新授权登录了不同账号；请使用添加账号",
                ));
            }
            a.view.id = id.clone();
            a.view.created_at = old.view.created_at;
            a.view.last_used_at = old.view.last_used_at;
            self.repo.put(a.clone()).await?;
            self.pending_rotation.lock().await.remove(&id);
        } else {
            self.repo
                .update(|all| {
                    if all.iter().any(|v| v.view.account_id == a.view.account_id) {
                        return Err(AppError::new(
                            "ACCOUNT_EXISTS",
                            "账号已经存在，请使用重新授权",
                        ));
                    }
                    all.push(a.clone());
                    Ok(())
                })
                .await?;
        }
        Ok(a.view)
    }
    pub async fn delete(&self, id: &str) -> Result<()> {
        let _switch = self.switch_lock.lock().await;
        let _token = self.token_lock(id).await;
        self.repo
            .update(|all| {
                let pos = all
                    .iter()
                    .position(|a| a.view.id == id)
                    .ok_or_else(|| AppError::new("ACCOUNT_NOT_FOUND", "账号不存在"))?;
                all.remove(pos);
                Ok(())
            })
            .await?;
        self.pending_rotation.lock().await.remove(id);
        Ok(())
    }
    pub async fn import(&self, index: usize) -> Result<AccountView> {
        // Snapshot is read under switch lock, add performs a second serialized mutation.
        let a = {
            let _s = self.switch_lock.lock().await;
            self.runtimes[index].read()?.account.ok_or_else(|| {
                AppError::new("NO_OAUTH_ACCOUNT", "运行环境没有可导入的 OAuth 账号")
            })?
        };
        self.add(a, None).await
    }
}
