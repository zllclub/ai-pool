use crate::{
    account::model::{now, Account, AccountView, Credentials},
    error::{AppError, Result},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use serde_json::Value;
// Version-sensitive OpenAI protocol constants live only in this module.
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
pub const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
pub const CALLBACK_ADDRESS: &str = "127.0.0.1:1455";
const AUTH_CLAIM: &str = "https://api.openai.com/auth";

#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_in: i64,
}
// Claims are metadata, NOT proof of authentication. Only HTTPS token exchange establishes trust.
// Local imported JWTs are untrusted; their metadata is used for display/routing only.
pub fn claims(token: &str) -> Result<Value> {
    let p = token
        .split('.')
        .nth(1)
        .ok_or_else(|| AppError::new("TOKEN_FORMAT", "无法解析 Token 元数据"))?;
    let b = URL_SAFE_NO_PAD
        .decode(p.trim_end_matches('='))
        .map_err(|_| AppError::new("TOKEN_FORMAT", "无法解析 Token 元数据"))?;
    serde_json::from_slice(&b).map_err(|_| AppError::new("TOKEN_FORMAT", "无法解析 Token 元数据"))
}
pub fn expiry(token: &str) -> i64 {
    claims(token)
        .ok()
        .and_then(|v| v["exp"].as_i64())
        .and_then(|v| v.checked_mul(1000))
        .unwrap_or(0)
}
pub fn account(credentials: Credentials, explicit_id: Option<&str>) -> Result<Account> {
    let access = claims(&credentials.access_token)?;
    let id = credentials
        .id_token
        .as_deref()
        .and_then(|s| claims(s).ok())
        .unwrap_or(Value::Null);
    let claimed = access[AUTH_CLAIM]["chatgpt_account_id"]
        .as_str()
        .or_else(|| id[AUTH_CLAIM]["chatgpt_account_id"].as_str());
    if let (Some(a), Some(b)) = (explicit_id, claimed) {
        if a != b {
            return Err(AppError::new("RUNTIME_DESYNC", "账号 ID 与 Token 不一致"));
        }
    }
    let account_id = claimed
        .or(explicit_id)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::new("ACCOUNT_ID_MISSING", "Token 缺少 ChatGPT accountId"))?;
    let email = id["email"]
        .as_str()
        .or_else(|| access["https://api.openai.com/profile"]["email"].as_str())
        .or_else(|| access["email"].as_str())
        .map(String::from);
    let plan_type = access[AUTH_CLAIM]["chatgpt_plan_type"]
        .as_str()
        .map(String::from);
    Ok(Account {
        view: AccountView {
            id: uuid::Uuid::new_v4().to_string(),
            email,
            account_id: account_id.into(),
            plan_type,
            quota: None,
            created_at: now(),
            last_used_at: None,
        },
        credentials,
    })
}
pub async fn exchange(
    client: &reqwest::Client,
    form: &[(&str, &str)],
    old: Option<&Credentials>,
) -> Result<Credentials> {
    let response = client
        .post(TOKEN_URL)
        .form(form)
        .send()
        .await
        .map_err(|_| AppError::new("OAUTH_NETWORK", "OAuth 网络请求失败，请检查网络"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(
            if old.is_some() && (status.as_u16() == 400 || status.as_u16() == 401) {
                AppError::new(
                    "REFRESH_TOKEN_INVALID",
                    "Refresh Token 无效、已撤销或被其他客户端轮换；请重新授权",
                )
            } else {
                AppError::new(
                    "OAUTH_FAILED",
                    &format!("OAuth 请求失败（HTTP {}）", status.as_u16()),
                )
            },
        );
    }
    let t: TokenResponse = response
        .json()
        .await
        .map_err(|_| AppError::new("OAUTH_SCHEMA", "OAuth 返回结构发生变化"))?;
    if t.access_token.is_empty() || t.expires_in <= 0 || t.expires_in > 31_536_000 {
        return Err(AppError::new(
            "OAUTH_SCHEMA",
            "OAuth 返回无效 Token 或过期时间",
        ));
    }
    let refresh = t
        .refresh_token
        .filter(|s| !s.is_empty())
        .or_else(|| old.map(|c| c.refresh_token.clone()))
        .ok_or_else(|| AppError::new("OAUTH_SCHEMA", "OAuth 未返回 Refresh Token"))?;
    Ok(Credentials {
        access_token: t.access_token,
        refresh_token: refresh,
        id_token: t.id_token.or_else(|| old.and_then(|c| c.id_token.clone())),
        expires_at: now() + t.expires_in * 1000,
    })
}
