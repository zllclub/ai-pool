use super::protocol::{exchange, CLIENT_ID};
use crate::{account::model::Credentials, error::Result};

#[async_trait::async_trait]
pub trait TokenRefresher: Send + Sync {
    async fn refresh(&self, old: &Credentials) -> Result<Credentials>;
}
pub struct OpenAiRefresher(pub reqwest::Client);
#[async_trait::async_trait]
impl TokenRefresher for OpenAiRefresher {
    async fn refresh(&self, old: &Credentials) -> Result<Credentials> {
        // Caller holds per-account mutex through durable rotation commit. Never auto-retry POST.
        exchange(
            &self.0,
            &[
                ("grant_type", "refresh_token"),
                ("client_id", CLIENT_ID),
                ("refresh_token", &old.refresh_token),
            ],
            Some(old),
        )
        .await
    }
}
