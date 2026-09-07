use super::model::Account;
use crate::{
    error::{AppError, Result},
    storage::secure_store::SecureStore,
};
use tokio::sync::Mutex;
use zeroize::Zeroizing;

pub struct Repository {
    accounts: Mutex<Vec<Account>>,
    store: Box<dyn SecureStore>,
}
impl Repository {
    pub fn new(store: Box<dyn SecureStore>) -> Result<Self> {
        let accounts = match store.load()? {
            Some(b) => serde_json::from_slice(&b)
                .map_err(|_| AppError::new("VAULT_CORRUPT", "凭据库结构损坏"))?,
            None => vec![],
        };
        Ok(Self {
            accounts: Mutex::new(accounts),
            store,
        })
    }
    pub async fn all(&self) -> Vec<Account> {
        self.accounts.lock().await.clone()
    }
    pub async fn get(&self, id: &str) -> Result<Account> {
        self.accounts
            .lock()
            .await
            .iter()
            .find(|a| a.view.id == id)
            .cloned()
            .ok_or_else(|| AppError::new("ACCOUNT_NOT_FOUND", "账号不存在"))
    }
    // Persist copy first. A failed durable write never mutates the in-memory repository.
    pub async fn update<T>(&self, f: impl FnOnce(&mut Vec<Account>) -> Result<T>) -> Result<T> {
        let mut guard = self.accounts.lock().await;
        let mut next = guard.clone();
        let result = f(&mut next)?;
        let bytes = Zeroizing::new(
            serde_json::to_vec(&next).map_err(|_| AppError::new("SERIALIZE", "凭据序列化失败"))?,
        );
        self.store.save(&bytes)?;
        *guard = next;
        Ok(result)
    }
    pub async fn put(&self, a: Account) -> Result<()> {
        self.update(|all| {
            let slot = all
                .iter_mut()
                .find(|v| v.view.id == a.view.id)
                .ok_or_else(|| AppError::new("ACCOUNT_NOT_FOUND", "账号不存在"))?;
            *slot = a;
            Ok(())
        })
        .await
    }
}
