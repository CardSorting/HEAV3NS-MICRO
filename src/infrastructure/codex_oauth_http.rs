// [LAYER: INFRASTRUCTURE]
use crate::domain::types::account_types::ChatGptAccountCredentials;
use crate::domain::types::runtime::CancellationToken;
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::oauth_pkce::{encode_hex_lower, extract_jwt_expiry_ms, extract_openai_account_id, extract_openai_email, extract_openai_plan_type, parse_query_params, render_oauth_callback_html, url_encode_component};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub(crate) const OPENAI_OAUTH_CALLBACK_ADDR: &str = "127.0.0.1:1455";

pub const OPENAI_CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const OPENAI_OAUTH_AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
pub const OPENAI_OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub const OPENAI_OAUTH_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
pub const OPENAI_OAUTH_SCOPES: &str = "openid profile email offline_access";
pub const CHATGPT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
pub(crate) const TOKEN_REFRESH_SKEW_MS: u64 = 60_000;

fn is_safe_oauth_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("https://") {
        return true;
    }
    let Some(rest) = lower.strip_prefix("http://") else {
        return false;
    };
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host_port.contains('@')
        && (matches!(
            host_port.split(':').next().unwrap_or(""),
            "127.0.0.1" | "localhost"
        ) || host_port == "[::1]"
            || host_port.starts_with("[::1]:"))
}

pub(crate) fn resolve_token_url() -> String {
    std::env::var("HEAV3NS_OPENAI_TOKEN_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && is_safe_oauth_url(s))
        .unwrap_or_else(|| OPENAI_OAUTH_TOKEN_URL.to_string())
}

pub(crate) fn resolve_codex_base_url() -> String {
    std::env::var("HEAV3NS_CODEX_BASE_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty() && is_safe_oauth_url(s))
        .unwrap_or_else(|| CHATGPT_CODEX_BASE_URL.to_string())
}

pub(crate) fn random_bytes_32() -> Result<[u8; 32], String> {
    let mut buf = [0u8; 32];
    let mut f = fs::File::open("/dev/urandom")
        .map_err(|e| format!("CSPRNG /dev/urandom unavailable: {e}"))?;
    f.read_exact(&mut buf)
        .map_err(|e| format!("Failed reading CSPRNG bytes: {e}"))?;
    Ok(buf)
}


async fn read_oauth_body_bounded(
    mut resp: reqwest::Response,
    max_bytes: usize,
) -> Result<String, String> {
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if buf.len().saturating_add(chunk.len()) > max_bytes {
            return Err(format!("OAuth response exceeded {max_bytes} byte cap"));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Binds the loopback OAuth callback listener with brief `AddrInUse` retries so rapid cancel-and-retry sign-in succeeds.
pub(crate) async fn bind_oauth_callback_listener(cancel: &CancellationToken) -> Result<TcpListener, String> {
    let mut last_err = None;
    for attempt in 0..6 {
        if cancel.is_cancelled() { return Err("ChatGPT sign-in was cancelled.".to_string()); }
        match TcpListener::bind(OPENAI_OAUTH_CALLBACK_ADDR).await {
            Ok(listener) => return Ok(listener),
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse && attempt < 5 => {
                last_err = Some(err); tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(err) => return Err(format!("Could not start local sign-in listener on {OPENAI_OAUTH_CALLBACK_ADDR}: {err}")),
        }
    }
    Err(format!("Could not start local sign-in listener on {OPENAI_OAUTH_CALLBACK_ADDR}: {}", last_err.map(|e| e.to_string()).unwrap_or_else(|| "address already in use".to_string())))
}

pub(crate) async fn wait_for_oauth_callback(
    listener: TcpListener,
    expected_state: &str,
    cancel: &CancellationToken,
) -> Result<String, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err("ChatGPT sign-in timed out after 3 minutes.".to_string());
        }
        let accepted = tokio::select! {
            _ = cancel.cancelled() => return Err("ChatGPT sign-in was cancelled.".to_string()),
            _ = tokio::time::sleep(remaining) => return Err("ChatGPT sign-in timed out after 3 minutes.".to_string()),
            res = listener.accept() => res.map_err(|e| format!("Sign-in listener error: {e}"))?,
        };
        let (mut stream, _) = accepted;
        let mut req_buf = [0u8; 8192];
        let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_secs(5), stream.read(&mut req_buf)).await
        else {
            continue;
        };
        let req_text = String::from_utf8_lossy(&req_buf[..n]);
        let first_line = req_text.lines().next().unwrap_or("");
        let target = first_line.split_whitespace().nth(1).unwrap_or("/");
        if !target.starts_with("/auth/callback") {
            let _ = write_http_html(
                &mut stream,
                404,
                "Not found",
                "Waiting for ChatGPT sign-in.",
            )
            .await;
            continue;
        }
        let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
        let params = parse_query_params(query);
        if params.get("state").map(String::as_str) != Some(expected_state) {
            let _ = write_http_html(
                &mut stream,
                400,
                "Invalid state",
                "We couldn't verify this sign-in link.",
            )
            .await;
            continue;
        }
        if let Some(err) = params.get("error") {
            let desc = params
                .get("error_description")
                .cloned()
                .unwrap_or_else(|| err.clone());
            let _ = write_http_html(&mut stream, 400, "Sign-in cancelled", &desc).await;
            return Err(format!("ChatGPT sign-in failed: {desc}"));
        }
        let Some(code) = params.get("code").filter(|s| !s.is_empty()).cloned() else {
            let _ = write_http_html(
                &mut stream,
                400,
                "Missing code",
                "Authorization code missing.",
            )
            .await;
            continue;
        };
        let _ = write_http_html(
            &mut stream,
            200,
            "You're connected",
            "Your ChatGPT sign-in is connected to HEAV3NS. You can close this tab and return to the app.",
        )
        .await;
        return Ok(code);
    }
}

async fn write_http_html(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    title: &str,
    message: &str,
) -> std::io::Result<()> {
    let reason = match status { 200 => "OK", 404 => "Not Found", _ => "Bad Request" };
    let body = render_oauth_callback_html(status, title, message);
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.flush().await
}

fn oauth_http_client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: std::sync::OnceLock<Result<reqwest::Client, String>> =
        std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(crate) async fn exchange_code_for_tokens(
    code: &str,
    verifier: &str,
) -> Result<ChatGptAccountCredentials, String> {
    let body = format!(
        "grant_type=authorization_code&client_id={}&code={}&redirect_uri={}&code_verifier={}",
        url_encode_component(OPENAI_CODEX_CLIENT_ID),
        url_encode_component(code),
        url_encode_component(OPENAI_OAUTH_REDIRECT_URI),
        url_encode_component(verifier)
    );
    let resp = oauth_http_client()?
        .post(&resolve_token_url())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("OpenAI token exchange request failed: {e}"))?;
    let status = resp.status();
    let text = read_oauth_body_bounded(resp, 1024 * 1024).await?;
    if !status.is_success() {
        return Err(format!("OpenAI token exchange failed ({status}): {text}"));
    }
    let parsed: Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid token JSON: {e}"))?;
    parse_token_response(&parsed, None)
}

pub(crate) async fn refresh_oauth_token(
    refresh_token: &str,
    prev: &ChatGptAccountCredentials,
) -> Result<ChatGptAccountCredentials, String> {
    let body = format!(
        "grant_type=refresh_token&client_id={}&refresh_token={}",
        url_encode_component(OPENAI_CODEX_CLIENT_ID),
        url_encode_component(refresh_token)
    );
    let resp = oauth_http_client()?
        .post(&resolve_token_url())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("OpenAI token refresh failed: {e}"))?;
    let status = resp.status();
    let text = read_oauth_body_bounded(resp, 1024 * 1024).await?;
    if !status.is_success() {
        return Err(format!("OpenAI token refresh rejected ({status}): {text}"));
    }
    let parsed: Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid refresh JSON: {e}"))?;
    parse_token_response(&parsed, Some(prev))
}

pub(crate) fn parse_token_response(
    parsed: &Value,
    prev: Option<&ChatGptAccountCredentials>,
) -> Result<ChatGptAccountCredentials, String> {
    let access_token = parsed.get("access_token").and_then(Value::as_str).filter(|s| !s.is_empty())
        .ok_or_else(|| "OpenAI token response did not contain access_token.".to_string())?.to_string();
    let refresh_token = parsed.get("refresh_token").and_then(Value::as_str).filter(|s| !s.is_empty())
        .map(String::from).or_else(|| prev.and_then(|p| p.refresh_token.clone()));
    let id_token = parsed.get("id_token").and_then(Value::as_str).filter(|s| !s.is_empty())
        .map(String::from).or_else(|| prev.and_then(|p| p.id_token.clone()));

    let claim_source = id_token.as_deref().unwrap_or(&access_token);
    let account_id = extract_openai_account_id(claim_source)
        .or_else(|| extract_openai_account_id(&access_token))
        .or_else(|| prev.and_then(|p| p.account_id.clone()));
    let email = parsed
        .get("email")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| extract_openai_email(claim_source))
        .or_else(|| extract_openai_email(&access_token))
        .or_else(|| prev.and_then(|p| p.email.clone()));
    let plan_type = extract_openai_plan_type(claim_source)
        .or_else(|| extract_openai_plan_type(&access_token))
        .or_else(|| prev.and_then(|p| p.plan_type.clone()));

    let expires_in = parsed.get("expires_in").and_then(Value::as_u64);
    let expires_at_ms = extract_jwt_expiry_ms(&access_token)
        .or_else(|| expires_in.map(|s| now_ms().saturating_add(s.saturating_mul(1_000))))
        .or_else(|| prev.and_then(|p| p.expires_at_ms));

    let label = prev
        .map(|p| p.label.clone())
        .or_else(|| email.clone())
        .unwrap_or_else(|| "ChatGPT Account".to_string());
    let id = match prev.map(|p| p.id.clone()).or_else(|| account_id.clone()) {
        Some(existing) => existing,
        None => format!("acc-{}", &encode_hex_lower(&random_bytes_32()?[..6])),
    };

    Ok(ChatGptAccountCredentials {
        id: id.chars().take(64).collect(),
        label: label.chars().take(100).collect(),
        email,
        plan_type,
        account_id,
        access_token,
        refresh_token,
        id_token,
        expires_at_ms,
        limited_until_ms: prev.and_then(|p| p.limited_until_ms),
        limit_estimated: prev.is_some_and(|p| p.limit_estimated),
        updated_at_ms: now_ms(),
    })
}

/// Builds a ChatGPT Codex backend POST request with OAuth Bearer + account headers.
pub(crate) fn build_codex_oauth_post(
    client: &reqwest::Client,
    url: &str,
    oauth_token: &str,
    account_id: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut b = client
        .post(url)
        .header("content-type", "application/json")
        .header("originator", "codex_cli")
        .bearer_auth(oauth_token);
    if let Some(id) = account_id.filter(|s| !s.is_empty()) {
        b = b.header("ChatGPT-Account-ID", id);
    }
    b
}

pub(crate) async fn probe_codex_models(access_token: &str, account_id: Option<&str>) -> (bool, usize) {
    let Ok(client) = oauth_http_client() else {
        return (false, 0);
    };
    let url = format!(
        "{}/models?client_version={}",
        resolve_codex_base_url(),
        env!("CARGO_PKG_VERSION")
    );
    let mut req = client
        .get(&url)
        .timeout(Duration::from_secs(5))
        .header("Accept", "application/json")
        .header("originator", "codex_cli")
        .bearer_auth(access_token);
    if let Some(id) = account_id.filter(|s| !s.is_empty()) {
        req = req.header("ChatGPT-Account-ID", id);
    }
    let Ok(resp) = req.send().await else {
        return (false, 0);
    };
    if !resp.status().is_success() {
        return (false, 0);
    }
    let Ok(text) = read_oauth_body_bounded(resp, 2 * 1024 * 1024).await else {
        return (false, 0);
    };
    let Ok(body) = serde_json::from_str::<Value>(&text) else {
        return (false, 0);
    };
    let count = body
        .get("models")
        .and_then(Value::as_array)
        .or_else(|| body.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    (true, count)
}

