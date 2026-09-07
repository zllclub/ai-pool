use super::*;
use crate::{account::model::Credentials, oauth::protocol};
use std::path::PathBuf;
pub struct PiAdapter {
    pub path: PathBuf,
}
impl RuntimeAdapter for PiAdapter {
    fn name(&self) -> &'static str {
        "Pi Agent"
    }
    fn path(&self) -> &Path {
        &self.path
    }
    fn detect_current_account(&self, v: &Value) -> Result<Option<Account>> {
        let Some(t) = v.get("openai-codex").filter(|v| !v.is_null()) else {
            return Ok(None);
        };
        if t.as_object().is_some_and(|v| v.is_empty()) {
            return Ok(None);
        }
        if t["type"] != "oauth" {
            return Ok(None);
        }
        let access_token = required(t, "access")?;
        let expires_at = t["expires"]
            .as_i64()
            .ok_or_else(|| AppError::new("RUNTIME_SCHEMA", "Pi OAuth expires 必须为毫秒时间戳"))?;
        protocol::account(
            Credentials {
                access_token,
                refresh_token: required(t, "refresh")?,
                id_token: None,
                expires_at,
            },
            t["accountId"].as_str(),
        )
        .map(Some)
    }
    fn apply_account(&self, original: &Value, a: &Account) -> Result<Value> {
        let mut v = original.clone();
        let obj = v
            .as_object_mut()
            .ok_or_else(|| AppError::new("JSON_CORRUPT", "Pi 认证文件必须为 JSON 对象"))?;
        // Never reconstruct the root: every unrelated provider is preserved verbatim as JSON.
        obj.insert("openai-codex".into(),serde_json::json!({"type":"oauth","access":a.credentials.access_token,"refresh":a.credentials.refresh_token,"expires":a.credentials.expires_at,"accountId":a.view.account_id}));
        Ok(v)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::model::*;
    #[test]
    fn preserves_other_providers() {
        let a = Account {
            view: AccountView {
                id: "a".into(),
                account_id: "a".into(),
                email: None,
                plan_type: None,
                quota: None,
                created_at: 0,
                last_used_at: None,
            },
            credentials: Credentials {
                access_token: "access".into(),
                refresh_token: "refresh".into(),
                id_token: None,
                expires_at: 123,
            },
        };
        let p = PiAdapter {
            path: PathBuf::new(),
        };
        let old = serde_json::json!({"anthropic":{"type":"api_key","key":"test"},"other":[1,2]});
        let next = p.apply_account(&old, &a).unwrap();
        assert_eq!(next["anthropic"], old["anthropic"]);
        assert_eq!(next["other"], old["other"]);
        assert_eq!(next["openai-codex"]["expires"], 123);
    }
}
