use super::*;
use crate::{account::model::Credentials, oauth::protocol};
use std::path::PathBuf;
pub struct CodexAdapter {
    pub path: PathBuf,
}
impl RuntimeAdapter for CodexAdapter {
    fn name(&self) -> &'static str {
        "Codex"
    }
    fn path(&self) -> &Path {
        &self.path
    }
    fn detect_current_account(&self, v: &Value) -> Result<Option<Account>> {
        if v["auth_mode"]
            .as_str()
            .is_some_and(|mode| mode != "chatgpt")
            || v["OPENAI_API_KEY"]
                .as_str()
                .is_some_and(|key| !key.is_empty())
        {
            return Ok(None);
        }
        let Some(t) = v.get("tokens").filter(|t| !t.is_null()) else {
            return Ok(None);
        };
        let access_token = required(t, "access_token")?;
        let c = Credentials {
            expires_at: protocol::expiry(&access_token),
            access_token,
            refresh_token: required(t, "refresh_token")?,
            id_token: t["id_token"].as_str().map(String::from),
        };
        protocol::account(c, t["account_id"].as_str()).map(Some)
    }
    fn apply_account(&self, _: &Value, a: &Account) -> Result<Value> {
        if a.credentials.id_token.as_deref().is_none_or(str::is_empty) {
            return Err(AppError::new(
                "CODEX_ID_TOKEN_MISSING",
                "Codex 需要 ID Token；请在管理器中重新授权此账号",
            ));
        }
        // Codex file schema is isolated here. API-key mode must not survive an OAuth switch.
        Ok(
            serde_json::json!({"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{"id_token":a.credentials.id_token,"access_token":a.credentials.access_token,"refresh_token":a.credentials.refresh_token,"account_id":a.view.account_id},"last_refresh":chrono::Utc::now().to_rfc3339()}),
        )
    }
}
