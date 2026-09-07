use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
// Intentionally no Debug: accidental diagnostic output cannot reveal credentials.
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: Option<String>,
    pub expires_at: i64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountQuota {
    pub five_hour_remaining: Option<f64>,
    pub weekly_remaining: Option<f64>,
    pub five_hour_reset_at: Option<i64>,
    pub weekly_reset_at: Option<i64>,
    pub updated_at: i64,
}
// The ONLY account DTO exposed over IPC. Credentials are a separate Rust-only type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub id: String,
    pub email: Option<String>,
    pub account_id: String,
    pub plan_type: Option<String>,
    pub quota: Option<AccountQuota>,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub view: AccountView,
    pub credentials: Credentials,
}
