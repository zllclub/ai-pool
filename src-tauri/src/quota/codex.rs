use super::provider::QuotaProvider;
use crate::{
    account::model::{now, Account, AccountQuota, QuotaWindow},
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
fn window(v: &Value, source: &str) -> Result<Option<QuotaWindow>> {
    if v.is_null() {
        return Ok(None);
    }
    let object = v.as_object().ok_or_else(schema)?;
    if object.is_empty() {
        return Ok(None);
    }
    let duration = v
        .get("limit_window_seconds")
        .filter(|n| !n.is_null())
        .map(|n| n.as_i64().filter(|n| *n > 0).ok_or_else(schema))
        .transpose()?;
    // primary/secondary indicate priority, NOT duration. Weekly-only plans may put
    // their weekly limit in primary_window. Never infer a 5H limit from its position.
    let label = match duration {
        Some(18000) => "5H".into(),
        Some(604800) => "Weekly".into(),
        Some(n) if n % 86400 == 0 => format!("{} 天", n / 86400),
        Some(n) if n % 3600 == 0 => format!("{}H", n / 3600),
        Some(n) if n % 60 == 0 => format!("{} 分钟", n / 60),
        Some(n) => format!("{} 秒", n),
        None => if source == "primary_window" {
            "主窗口"
        } else {
            "次窗口"
        }
        .into(),
    };
    let remaining = v
        .get("used_percent")
        .filter(|n| !n.is_null())
        .map(|n| {
            n.as_f64()
                .filter(|n| n.is_finite() && (0.0..=100.0).contains(n))
                .map(|used| 100.0 - used)
                .ok_or_else(schema)
        })
        .transpose()?;
    let reset_at = if let Some(r) = v.get("reset_at").filter(|n| !n.is_null()) {
        Some(
            r.as_i64()
                .filter(|n| *n >= 0)
                .and_then(|n| n.checked_mul(1000))
                .ok_or_else(schema)?,
        )
    } else if let Some(r) = v.get("reset_after_seconds").filter(|n| !n.is_null()) {
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
    // An unrecognized non-empty object should still signal schema drift, not fake success.
    if duration.is_none() && remaining.is_none() && reset_at.is_none() {
        return Err(schema());
    }
    Ok(Some(QuotaWindow {
        id: source.into(),
        label,
        limit_window_seconds: duration,
        remaining,
        reset_at,
    }))
}
pub fn parse(v: &Value) -> Result<(AccountQuota, Option<String>)> {
    let rate = v.get("rate_limit").ok_or_else(schema)?;
    if !rate.is_null() && !rate.is_object() {
        return Err(schema());
    }
    let mut windows = Vec::new();
    for source in ["primary_window", "secondary_window"] {
        if let Some(w) = window(&rate[source], source)? {
            windows.push(w);
        }
    }
    // Preserve legacy fields for existing consumers, but derive them from duration only.
    let mut quota = AccountQuota {
        updated_at: now(),
        ..Default::default()
    };
    for w in &windows {
        match w.limit_window_seconds {
            Some(18000) => {
                quota.five_hour_remaining = w.remaining;
                quota.five_hour_reset_at = w.reset_at;
            }
            Some(604800) => {
                quota.weekly_remaining = w.remaining;
                quota.weekly_reset_at = w.reset_at;
            }
            _ => {}
        }
    }
    windows.sort_by_key(|w| w.limit_window_seconds.unwrap_or(i64::MAX));
    quota.windows = Some(windows);
    Ok((quota, v["plan_type"].as_str().map(String::from)))
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
    fn weekly_only_primary_is_not_five_hour() {
        let (q, _) = parse(&json!({"rate_limit": {"primary_window": {
            "limit_window_seconds": 604800, "used_percent": 13, "reset_at": 100
        }, "secondary_window": null}}))
        .unwrap();
        assert_eq!(q.five_hour_remaining, None);
        assert_eq!(q.weekly_remaining, Some(87.));
        let windows = q.windows.unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].label, "Weekly");
    }
    #[test]
    fn missing_primary_and_secondary_only() {
        let (q, _) = parse(&json!({"rate_limit": {"secondary_window": {
            "limit_window_seconds": 604800, "used_percent": 0, "reset_at": null
        }}}))
        .unwrap();
        assert_eq!(q.five_hour_remaining, None);
        assert_eq!(q.weekly_remaining, Some(100.));
        assert_eq!(q.weekly_reset_at, None);
    }
    #[test]
    fn reversed_windows_are_classified_by_duration() {
        let (q, _) = parse(&json!({"rate_limit": {
            "primary_window": {"limit_window_seconds": 604800, "used_percent": 35},
            "secondary_window": {"limit_window_seconds": 18000, "used_percent": 100}
        }}))
        .unwrap();
        assert_eq!(q.five_hour_remaining, Some(0.));
        assert_eq!(q.weekly_remaining, Some(65.));
        assert_eq!(q.windows.unwrap()[0].label, "5H");
    }
    #[test]
    fn null_empty_or_absent_windows_are_not_schema_errors() {
        for rate in [
            Value::Null,
            json!({}),
            json!({"primary_window":null}),
            json!({"primary_window":{},"secondary_window":null}),
        ] {
            let (q, _) = parse(&json!({"rate_limit":rate})).unwrap();
            assert!(q.windows.unwrap().is_empty());
            assert_eq!(q.five_hour_remaining, None);
            assert_eq!(q.weekly_remaining, None);
        }
    }
    #[test]
    fn non_standard_duration_and_missing_usage_remain_visible() {
        let (q, _) = parse(&json!({"rate_limit": {"primary_window": {
            "limit_window_seconds": 86400, "used_percent": null
        }}}))
        .unwrap();
        assert_eq!(q.five_hour_remaining, None);
        let w = q.windows.unwrap();
        assert_eq!(w[0].label, "1 天");
        assert_eq!(w[0].remaining, None);
    }
    #[test]
    fn absent_duration_does_not_invent_a_five_hour_limit() {
        let (q, _) =
            parse(&json!({"rate_limit": {"primary_window": {"used_percent": 35}}})).unwrap();
        assert_eq!(q.five_hour_remaining, None);
        assert_eq!(q.windows.unwrap()[0].label, "主窗口");
    }
    #[test]
    fn old_cache_deserializes_without_windows() {
        let q: AccountQuota = serde_json::from_value(json!({"fiveHourRemaining": 65,"weeklyRemaining":null,"fiveHourResetAt":null,"weeklyResetAt":null,"updatedAt":1})).unwrap();
        assert!(q.windows.is_none());
        assert_eq!(q.five_hour_remaining, Some(65.));
    }
    #[test]
    fn schema_drift() {
        assert!(parse(&json!({"rate_limit":{"primary_window":{"used_percent":101}}})).is_err());
        assert!(parse(&json!({})).is_err());
        assert!(parse(&json!({"rate_limit": "wrong"})).is_err());
        assert!(parse(&json!({"rate_limit": {"primary_window": {"used_percent":"35"}}})).is_err());
        assert!(parse(&json!({"rate_limit": {"primary_window": {"unknown_field":true}}})).is_err());
    }
}
