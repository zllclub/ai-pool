use super::{callback, protocol::*};
use crate::{
    account::model::Account,
    error::{AppError, Result},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::{net::TcpListener, sync::Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct LoginManager {
    active: Mutex<Option<CancellationToken>>,
}
fn random() -> String {
    let mut b = [0; 32];
    OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}
impl LoginManager {
    pub async fn cancel(&self) {
        if let Some(c) = self.active.lock().await.as_ref() {
            c.cancel();
        }
    }
    pub async fn login(&self, client: &reqwest::Client) -> Result<Account> {
        let cancel = CancellationToken::new();
        {
            let mut active = self.active.lock().await;
            if active.is_some() {
                return Err(AppError::new("OAUTH_BUSY", "已有 OAuth 登录进行中"));
            }
            *active = Some(cancel.clone());
        }
        let result = tokio::select! {
            _=cancel.cancelled()=>Err(AppError::new("OAUTH_CANCELLED","OAuth 已取消")),
            result=tokio::time::timeout(Duration::from_secs(180),perform(client))=>result.unwrap_or_else(|_|Err(AppError::new("OAUTH_TIMEOUT","OAuth 登录超时，请重试"))),
        };
        *self.active.lock().await = None;
        result
    }
}
async fn perform(client: &reqwest::Client) -> Result<Account> {
    // Bind before launching browser. Dropping listener on cancel/timeout releases port 1455.
    let listener = TcpListener::bind(CALLBACK_ADDRESS).await.map_err(|_| {
        AppError::new(
            "OAUTH_PORT_BUSY",
            "无法绑定本地 1455 端口，请结束其他 Codex/Pi 登录后重试",
        )
    })?;
    let verifier = random();
    let state = random();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = url::Url::parse(AUTHORIZE_URL)
        .map_err(|_| AppError::new("OAUTH_CONFIG", "OAuth URL 无效"))?;
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT_URI),
        ("scope", "openid profile email offline_access"),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("state", &state),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", "codex_cli_rs"),
        ("prompt", "login"),
    ]);
    open::that(url.as_str()).map_err(|_| AppError::new("BROWSER_OPEN", "无法打开系统浏览器"))?;
    let code = callback::receive(listener, &state).await?;
    let c = exchange(
        client,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", &code),
            ("code_verifier", &verifier),
            ("redirect_uri", REDIRECT_URI),
        ],
        None,
    )
    .await?;
    account(c, None)
}
