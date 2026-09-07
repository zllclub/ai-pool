use crate::{
    account::model::{Account, AccountQuota},
    error::Result,
};
#[async_trait::async_trait]
pub trait QuotaProvider: Send + Sync {
    async fn get_quota(&self, account: &Account) -> Result<(AccountQuota, Option<String>)>;
}
