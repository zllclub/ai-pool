use crate::error::{AppError, Result};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use url::Url;

pub fn parse(target: &str, state: &str) -> Result<String> {
    let url = Url::parse(&format!("http://localhost{target}"))
        .map_err(|_| AppError::new("OAUTH_CALLBACK", "无效回调"))?;
    if url.path() != "/auth/callback" {
        return Err(AppError::new("OAUTH_CALLBACK", "回调路径不匹配"));
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    let values = |key: &str| {
        pairs
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_ref())
            .collect::<Vec<_>>()
    };
    if values("state") != vec![state] {
        return Err(AppError::new("OAUTH_STATE", "OAuth state 不匹配"));
    }
    if !values("error").is_empty() {
        return Err(AppError::new("OAUTH_DENIED", "OAuth 登录被拒绝或取消"));
    }
    let code = values("code");
    if code.len() != 1 || code[0].is_empty() {
        return Err(AppError::new(
            "OAUTH_CALLBACK",
            "回调缺少唯一 authorization code",
        ));
    }
    Ok(code[0].into())
}
pub async fn receive(listener: TcpListener, state: &str) -> Result<String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|_| AppError::new("OAUTH_CALLBACK", "无法接受 OAuth 回调"))?;
        // Bound each request in size and time. Bad requests cannot consume the login attempt.
        let request = tokio::time::timeout(Duration::from_secs(3), async {
            let mut bytes = Vec::new();
            let mut buf = [0; 1024];
            loop {
                let n = stream.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&buf[..n]);
                if bytes.windows(4).any(|v| v == b"\r\n\r\n") || bytes.len() > 8192 {
                    break;
                }
            }
            Ok::<_, std::io::Error>(bytes)
        })
        .await;
        let result = match request {
            Ok(Ok(b)) if b.len() <= 8192 => {
                let text = String::from_utf8_lossy(&b);
                let mut words = text.lines().next().unwrap_or("").split_whitespace();
                if words.next() == Some("GET") {
                    parse(words.next().unwrap_or(""), state)
                } else {
                    Err(AppError::new("OAUTH_CALLBACK", "无效 HTTP 请求"))
                }
            }
            _ => Err(AppError::new("OAUTH_CALLBACK", "回调请求超时或过大")),
        };
        let message = if result.is_ok() {
            "Callback received. Return to Codex Accounts to finish login."
        } else {
            "Invalid or denied callback. Return to the application."
        };
        let response=format!("HTTP/1.1 {}\r\nContent-Type: text/plain\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",if result.is_ok(){"200 OK"}else{"400 Bad Request"},message.len(),message);
        let _ = tokio::time::timeout(
            Duration::from_secs(1),
            stream.write_all(response.as_bytes()),
        )
        .await;
        match result {
            Ok(code) => return Ok(code),
            Err(e) if e.code == "OAUTH_DENIED" => return Err(e),
            _ => continue,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_and_duplicates() {
        assert!(parse("/auth/callback?code=x&state=good", "good").is_ok());
        assert!(parse("/auth/callback?code=x&state=bad", "good").is_err());
        assert!(parse("/auth/callback?code=x&state=good&state=good", "good").is_err());
    }
}
