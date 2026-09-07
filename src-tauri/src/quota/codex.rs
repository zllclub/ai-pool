use super::provider::QuotaProvider;
use crate::{
    account::model::{now, Account, AccountQuota},
    error::{AppError, Result},
};
use serde_json::Value;
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
pub struct CodexQuotaProvider {
    pub client: reqwest::Client,
}
fn schema() -> AppError {
    AppError::new("QUOTA_SCHEMA_CHANGED", "额度接口发生变化，无法识别窗口数据")
}
fn window(v: &Value, expected: i64) -> Result<(Option<f64>, Option<i64>)> {
    if v.is_null() {
        return Ok((None, None));
    }
    if let Some(duration) = v.get("limit_window_seconds") {
        if duration.as_i64() != Some(expected) {
            return Err(schema());
        }
    }
    let used = v["used_percent"]
        .as_f64()
        .filter(|n| n.is_finite() && (0.0..=100.0).contains(n))
        .ok_or_else(schema)?;
    let reset = if let Some(r) = v.get("reset_at") {
        Some(
            r.as_i64()
                .and_then(|n| n.checked_mul(1000))
                .ok_or_else(schema)?,
        )
    } else if let Some(r) = v.get("reset_after_seconds") {
        Some(
            now()
                + r.as_i64()
                    .filter(|n| *n >= 0 && *n < 31_536_000)
                    .ok_or_else(schema)?
                    * 1000,
        )
    } else {
        None
    };
    Ok((Some(100.0 - used), reset))
}
pub fn parse(v: &Value) -> Result<(AccountQuota, Option<String>)> {
    let rate = v
        .get("rate_limit")
        .filter(|v| v.is_object())
        .ok_or_else(schema)?;
    if rate.get("primary_window").is_none() && rate.get("secondary_window").is_none() {
        return Err(schema());
    }
    let (five_hour_remaining, five_hour_reset_at) = window(&rate["primary_window"], 18000)?;
    let (weekly_remaining, weekly_reset_at) = window(&rate["secondary_window"], 604800)?;
    Ok((
        AccountQuota {
            five_hour_remaining,
            weekly_remaining,
            five_hour_reset_at,
            weekly_reset_at,
            updated_at: now(),
        },
        v["plan_type"].as_str().map(String::from),
    ))
}
#[async_trait::async_trait]
impl QuotaProvider for CodexQuotaProvider {
    async fn get_quota(&self, a: &Account) -> Result<(AccountQuota, Option<String>)> {
        let r = self
            .client
            .get(USAGE_URL)
            .bearer_auth(&a.credentials.access_token)
            .header("ChatGPT-Account-Id", &a.view.account_id)
            .header("OpenAI-Beta", "codex-1")
            .send()
            .await
            .map_err(|_| AppError::new("QUOTA_NETWORK", "额度查询失败，请检查网络"))?;
        if r.status().as_u16() == 401 {
            return Err(AppError::new("TOKEN_EXPIRED", "额度服务拒绝 Token"));
        }
        if !r.status().is_success() {
            return Err(AppError::new(
                "QUOTA_HTTP",
                &format!("额度查询失败（HTTP {}）", r.status().as_u16()),
            ));
        }
        let v = r.json().await.map_err(|_| schema())?;
        parse(&v)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn remaining_and_reset() {
        let (q,_)=parse(&json!({"rate_limit":{"primary_window":{"used_percent":35,"reset_at":100,"limit_window_seconds":18000},"secondary_window":null}})).unwrap();
        assert_eq!(q.five_hour_remaining, Some(65.));
        assert_eq!(q.five_hour_reset_at, Some(100000));
        assert_eq!(q.weekly_remaining, None);
    }
    #[test]
    fn schema_drift() {
        assert!(parse(&json!({"rate_limit":{"primary_window":{"used_percent":101}}})).is_err());
        assert!(parse(&json!({})).is_err());
    }
}
