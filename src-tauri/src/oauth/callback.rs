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
        let succeeded = result.is_ok();
        let (title, description, icon, accent) = if succeeded {
            (
                "登录成功",
                "授权已完成，请返回 AIPool 继续使用。",
                "✓",
                "#10b981",
            )
        } else {
            (
                "登录未完成",
                "回调无效或授权已被拒绝，请返回 AIPool 后重试。",
                "!",
                "#ef4444",
            )
        };
        let body = format!(
            r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="light dark">
  <title>{title} | AIPool</title>
  <style>
    :root {{ color-scheme: light dark; font-family: Inter, ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif; }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px; background: #f4f7fb; color: #172033; }}
    main {{ width: min(100%, 440px); padding: 44px 36px; text-align: center; background: rgba(255,255,255,.94); border: 1px solid rgba(15,23,42,.08); border-radius: 24px; box-shadow: 0 24px 60px rgba(15,23,42,.12); }}
    .icon {{ display: grid; place-items: center; width: 64px; height: 64px; margin: 0 auto 24px; border-radius: 50%; background: {accent}; color: white; font-size: 36px; font-weight: 700; }}
    h1 {{ margin: 0 0 12px; font-size: 26px; }}
    p {{ margin: 0; color: #64748b; line-height: 1.7; }}
    .brand {{ margin-top: 28px; color: #94a3b8; font-size: 13px; letter-spacing: .08em; }}
    @media (prefers-color-scheme: dark) {{
      body {{ background: #0b1120; color: #f1f5f9; }}
      main {{ background: rgba(15,23,42,.94); border-color: rgba(148,163,184,.16); box-shadow: 0 24px 60px rgba(0,0,0,.35); }}
      p {{ color: #94a3b8; }}
    }}
  </style>
</head>
<body>
  <main>
    <div class="icon" aria-hidden="true">{icon}</div>
    <h1>{title}</h1>
    <p>{description}</p>
    <div class="brand">AIPOOL</div>
  </main>
</body>
</html>"#
        );
        let response = format!(
            "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            if succeeded { "200 OK" } else { "400 Bad Request" },
            body.len(),
            body
        );
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
