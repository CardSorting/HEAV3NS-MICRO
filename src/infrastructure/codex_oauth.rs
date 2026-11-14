// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::{BoxFuture, ICodexAccountStore};
use crate::domain::types::account_types::{ChatGptAccountCredentials, ChatGptAccountStoreData};
use crate::domain::types::runtime::CancellationToken;
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::oauth_pkce::{build_openai_authorize_url, encode_base64url, encode_hex_lower, extract_jwt_expiry_ms, extract_openai_account_id, extract_openai_email, extract_openai_plan_type, generate_pkce_challenge, parse_query_params, url_encode_component};
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex as AsyncMutex;

static REFRESH_FLIGHT_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

fn refresh_flight_lock() -> &'static AsyncMutex<()> {
    REFRESH_FLIGHT_LOCK.get_or_init(|| AsyncMutex::new(()))
}

pub const OPENAI_CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const OPENAI_OAUTH_AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
pub const OPENAI_OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub const OPENAI_OAUTH_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
pub const OPENAI_OAUTH_CALLBACK_ADDR: &str = "127.0.0.1:1455";
pub const OPENAI_OAUTH_SCOPES: &str = "openid profile email offline_access";
pub const CHATGPT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
const TOKEN_REFRESH_SKEW_MS: u64 = 60_000;

pub(crate) fn resolve_token_url() -> String {
    std::env::var("HEAV3NS_OPENAI_TOKEN_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| OPENAI_OAUTH_TOKEN_URL.to_string())
}

pub(crate) fn resolve_codex_base_url() -> String {
    std::env::var("HEAV3NS_CODEX_BASE_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| CHATGPT_CODEX_BASE_URL.to_string())
}

/// Persistent OpenAI / ChatGPT OAuth account store and loopback PKCE client.
#[derive(Debug, Clone)]
pub struct CodexAccountStore {
    path: PathBuf,
}

impl CodexAccountStore {
    pub fn default_store() -> Self {
        Self { path: resolve_accounts_path() }
    }

    pub fn at_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn load_accounts_store(&self) -> ChatGptAccountStoreData {
        let Ok(raw) = fs::read_to_string(&self.path) else {
            return ChatGptAccountStoreData::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save_accounts_store(&self, data: &ChatGptAccountStoreData) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create accounts dir: {e}"))?;
        }
        let serialized = serde_json::to_string_pretty(data).map_err(|e| format!("Failed to serialize accounts: {e}"))?;
        let tmp = self.path.with_extension(format!("json.tmp.{}", std::process::id()));
        fs::write(&tmp, serialized.as_bytes()).map_err(|e| format!("Failed to write accounts file: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &self.path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("Failed to persist accounts file: {e}")
        })?;
        Ok(())
    }

    pub fn rpc_snapshot(&self) -> Value {
        snapshot_value(&self.load_accounts_store())
    }

    pub fn switch_account(&self, target_id: &str) -> Result<Value, String> {
        let mut data = self.load_accounts_store();
        if !data.accounts.iter().any(|a| a.id == target_id) {
            return Err(format!("Account '{target_id}' was not found."));
        }
        data.active_id = Some(target_id.to_string());
        self.save_accounts_store(&data)?;
        Ok(snapshot_value(&data))
    }

    pub fn rename_account(&self, target_id: &str, new_label: &str) -> Result<Value, String> {
        let trimmed = new_label.trim();
        if trimmed.is_empty() {
            return Err("Account name cannot be empty.".to_string());
        }
        let mut data = self.load_accounts_store();
        let Some(acc) = data.accounts.iter_mut().find(|a| a.id == target_id) else {
            return Err(format!("Account '{target_id}' was not found."));
        };
        acc.label = trimmed.chars().take(100).collect();
        acc.updated_at_ms = now_ms();
        self.save_accounts_store(&data)?;
        Ok(snapshot_value(&data))
    }

    pub fn remove_account(&self, target_id: &str) -> Result<Value, String> {
        let mut data = self.load_accounts_store();
        let before = data.accounts.len();
        data.accounts.retain(|a| a.id != target_id);
        if data.accounts.len() == before {
            return Err(format!("Account '{target_id}' was not found."));
        }
        if data.active_id.as_deref() == Some(target_id) {
            data.active_id = data.accounts.first().map(|a| a.id.clone());
        }
        self.save_accounts_store(&data)?;
        Ok(snapshot_value(&data))
    }

    pub fn sign_out(&self, all: bool) -> Result<Value, String> {
        let mut data = self.load_accounts_store();
        if all {
            data.accounts.clear();
            data.active_id = None;
        } else if let Some(active) = data.active_id.clone() {
            data.accounts.retain(|a| a.id != active);
            data.active_id = data.accounts.first().map(|a| a.id.clone());
        }
        self.save_accounts_store(&data)?;
        Ok(snapshot_value(&data))
    }

    pub fn clear_limit(&self, target_id: &str) -> Result<Value, String> {
        let mut data = self.load_accounts_store();
        let Some(acc) = data.accounts.iter_mut().find(|a| a.id == target_id) else {
            return Err(format!("Account '{target_id}' was not found."));
        };
        acc.limited_until_ms = None;
        acc.limit_estimated = false;
        acc.updated_at_ms = now_ms();
        self.save_accounts_store(&data)?;
        Ok(snapshot_value(&data))
    }

    pub fn set_auto_switch(&self, enabled: bool) -> Result<Value, String> {
        let mut data = self.load_accounts_store();
        data.auto_switch = enabled;
        self.save_accounts_store(&data)?;
        Ok(snapshot_value(&data))
    }

    /// Runs the full OpenAI OAuth PKCE browser sign-in flow and saves the resulting account.
    pub async fn sign_in<F>(&self, cancel: CancellationToken, on_browser_url: F) -> Result<Value, String>
    where
        F: FnOnce(String),
    {
        let verifier = encode_base64url(&random_bytes_32());
        let state = encode_hex_lower(&random_bytes_32()[..16]);
        let challenge = generate_pkce_challenge(&verifier);
        let auth_url = build_openai_authorize_url(
            OPENAI_OAUTH_AUTH_URL,
            OPENAI_CODEX_CLIENT_ID,
            OPENAI_OAUTH_REDIRECT_URI,
            OPENAI_OAUTH_SCOPES,
            &challenge,
            &state,
        );

        let listener = TcpListener::bind(OPENAI_OAUTH_CALLBACK_ADDR)
            .await
            .map_err(|e| format!("Could not start local sign-in listener on {OPENAI_OAUTH_CALLBACK_ADDR}: {e}"))?;
        on_browser_url(auth_url);

        let code = wait_for_oauth_callback(listener, &state, &cancel).await?;
        if cancel.is_cancelled() {
            return Err("ChatGPT sign-in was cancelled.".to_string());
        }

        let creds = exchange_code_for_tokens(&code, &verifier).await?;
        let (access_checked, model_count) = probe_codex_models(&creds.access_token, creds.account_id.as_deref()).await;

        let mut data = self.load_accounts_store();
        let created = upsert_account(&mut data, creds.clone());
        self.save_accounts_store(&data)?;

        let mut out = snapshot_value(&data);
        if let Some(obj) = out.as_object_mut() {
            obj.insert("created".to_string(), json!(created));
            obj.insert("accessChecked".to_string(), json!(access_checked));
            obj.insert("modelCount".to_string(), json!(model_count));
            obj.insert("account".to_string(), json!({ "id": creds.id, "label": creds.label }));
        }
        Ok(out)
    }

    /// Returns `(access_token, chatgpt_account_id)` for the active account, refreshing if near expiry.
    pub async fn get_valid_active_credentials(&self) -> Result<Option<(String, Option<String>)>, String> {
        let data = self.load_accounts_store();
        let Some(active) = active_account(&data).cloned() else {
            return Ok(None);
        };
        let now = now_ms();
        let expired = active
            .expires_at_ms
            .is_some_and(|exp| exp <= now.saturating_add(TOKEN_REFRESH_SKEW_MS));
        if expired {
            return self.refresh_account(&active.id, None).await.map(Some);
        }
        Ok(Some((active.access_token, active.account_id)))
    }

    /// Forces an OAuth token refresh for the active account (e.g. after HTTP 401).
    pub async fn force_refresh_active(&self, rejected_token: &str) -> Result<(String, Option<String>), String> {
        let data = self.load_accounts_store();
        let active = active_account(&data)
            .ok_or_else(|| "No active ChatGPT account is signed in.".to_string())?;
        self.refresh_account(&active.id, Some(rejected_token)).await
    }

    async fn refresh_account(&self, id: &str, rejected_token: Option<&str>) -> Result<(String, Option<String>), String> {
        let _guard = refresh_flight_lock().lock().await;
        let mut data = self.load_accounts_store();
        let current = data
            .accounts
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .ok_or_else(|| "Active ChatGPT account no longer exists.".to_string())?;
        let now = now_ms();
        let still_valid = current
            .expires_at_ms
            .is_some_and(|exp| exp > now.saturating_add(TOKEN_REFRESH_SKEW_MS));
        let already_rotated = rejected_token.is_some_and(|rej| current.access_token != rej);
        if (rejected_token.is_none() && still_valid) || already_rotated {
            return Ok((current.access_token, current.account_id));
        }
        let refresh_tok = current
            .refresh_token
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "ChatGPT session expired and has no refresh token. Sign in again.".to_string())?;

        let refreshed = refresh_oauth_token(&refresh_tok, &current).await?;
        let result = (refreshed.access_token.clone(), refreshed.account_id.clone());
        if let Some(slot) = data.accounts.iter_mut().find(|a| a.id == id) {
            *slot = refreshed;
        }
        self.save_accounts_store(&data)?;
        Ok(result)
    }

    /// Records a 429 usage limit on the active account and auto-switches if another ready account exists.
    pub fn record_rate_limit(&self, retry_after_ms: Option<u64>) {
        let mut data = self.load_accounts_store();
        let now = now_ms();
        let until = now.saturating_add(retry_after_ms.unwrap_or(15 * 60_000).max(5_000));
        if let Some(active_id) = data.active_id.clone() {
            if let Some(acc) = data.accounts.iter_mut().find(|a| a.id == active_id) {
                acc.limited_until_ms = Some(until);
                acc.limit_estimated = retry_after_ms.is_none();
                acc.updated_at_ms = now;
            }
            if data.auto_switch {
                if let Some(next) = data.accounts.iter().find(|a| {
                    a.id != active_id && a.limited_until_ms.is_none_or(|u| u <= now)
                }) {
                    data.active_id = Some(next.id.clone());
                }
            }
            let _ = self.save_accounts_store(&data);
        }
    }
}

impl ICodexAccountStore for CodexAccountStore {
    fn has_active_account(&self) -> bool {
        let data = self.load_accounts_store();
        active_account(&data).is_some()
    }

    fn active_access_token(&self) -> Option<String> {
        let data = self.load_accounts_store();
        active_account(&data).map(|a| a.access_token.clone())
    }

    fn active_account_id(&self) -> Option<String> {
        let data = self.load_accounts_store();
        active_account(&data).and_then(|a| a.account_id.clone())
    }

    fn rpc_snapshot(&self) -> Value {
        Self::rpc_snapshot(self)
    }

    fn switch_account(&self, target_id: &str) -> Result<Value, String> {
        Self::switch_account(self, target_id)
    }

    fn rename_account(&self, target_id: &str, new_label: &str) -> Result<Value, String> {
        Self::rename_account(self, target_id, new_label)
    }

    fn remove_account(&self, target_id: &str) -> Result<Value, String> {
        Self::remove_account(self, target_id)
    }

    fn sign_out(&self, all: bool) -> Result<Value, String> {
        Self::sign_out(self, all)
    }

    fn clear_limit(&self, target_id: &str) -> Result<Value, String> {
        Self::clear_limit(self, target_id)
    }

    fn set_auto_switch(&self, enabled: bool) -> Result<Value, String> {
        Self::set_auto_switch(self, enabled)
    }

    fn sign_in_async(
        &self,
        cancel: CancellationToken,
        on_browser_url: Box<dyn FnOnce(String) + Send>,
    ) -> BoxFuture<'_, Result<Value, String>> {
        Box::pin(async move { self.sign_in(cancel, on_browser_url).await })
    }
}

fn resolve_accounts_path() -> PathBuf {
    if let Ok(explicit) = std::env::var("HEAV3NS_ACCOUNTS_PATH") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".heav3ns").join("accounts.json")
}

fn active_account(data: &ChatGptAccountStoreData) -> Option<&ChatGptAccountCredentials> {
    data.active_id
        .as_deref()
        .and_then(|id| data.accounts.iter().find(|a| a.id == id))
        .or_else(|| data.accounts.first())
}

fn snapshot_value(data: &ChatGptAccountStoreData) -> Value {
    let summaries = data.summaries(now_ms());
    let saved = !summaries.is_empty();
    let active_summary = summaries.iter().find(|s| s.active).or_else(|| summaries.first());
    let mut obj = json!({
        "saved": saved,
        "accounts": summaries,
        "autoSwitch": data.auto_switch,
    });
    if let Some(active) = active_summary {
        obj["account"] = json!({ "id": active.id, "label": active.label });
    }
    obj
}

fn upsert_account(data: &mut ChatGptAccountStoreData, mut incoming: ChatGptAccountCredentials) -> bool {
    let existing_idx = data.accounts.iter().position(|a| {
        (incoming.account_id.is_some() && a.account_id == incoming.account_id)
            || (incoming.email.is_some() && a.email == incoming.email)
            || a.id == incoming.id
    });
    if let Some(idx) = existing_idx {
        incoming.id = data.accounts[idx].id.clone();
        if !data.accounts[idx].label.is_empty() && !data.accounts[idx].label.starts_with("ChatGPT") {
            incoming.label = data.accounts[idx].label.clone();
        }
        data.accounts[idx] = incoming.clone();
        data.active_id = Some(incoming.id);
        false
    } else {
        if data.accounts.len() >= 16 {
            data.accounts.pop();
        }
        let id = incoming.id.clone();
        data.accounts.insert(0, incoming);
        data.active_id = Some(id);
        true
    }
}

fn random_bytes_32() -> [u8; 32] {
    let mut buf = [0u8; 32];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        if f.read_exact(&mut buf).is_ok() {
            return buf;
        }
    }
    let seed = now_ms();
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((seed >> ((i % 8) * 8)) ^ (i as u64).wrapping_mul(0x9E)) as u8;
    }
    buf
}

async fn wait_for_oauth_callback(
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
        let Ok(Ok(n)) = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut req_buf)).await else {
            continue;
        };
        let req_text = String::from_utf8_lossy(&req_buf[..n]);
        let first_line = req_text.lines().next().unwrap_or("");
        let target = first_line.split_whitespace().nth(1).unwrap_or("/");
        if !target.starts_with("/auth/callback") {
            let _ = write_http_html(&mut stream, 404, "Not found", "Waiting for ChatGPT sign-in.").await;
            continue;
        }
        let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
        let params = parse_query_params(query);
        if let Some(err) = params.get("error") {
            let desc = params.get("error_description").cloned().unwrap_or_else(|| err.clone());
            let _ = write_http_html(&mut stream, 400, "Sign-in cancelled", &desc).await;
            return Err(format!("ChatGPT sign-in failed: {desc}"));
        }
        if params.get("state").map(String::as_str) != Some(expected_state) {
            let _ = write_http_html(&mut stream, 400, "Invalid state", "We couldn't verify this sign-in link.").await;
            continue;
        }
        let Some(code) = params.get("code").filter(|s| !s.is_empty()).cloned() else {
            let _ = write_http_html(&mut stream, 400, "Missing code", "Authorization code missing.").await;
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

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

async fn write_http_html(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    title: &str,
    message: &str,
) -> std::io::Result<()> {
    let reason = if status == 200 { "OK" } else { "Bad Request" };
    let safe_title = escape_html(title);
    let safe_message = escape_html(message);
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{safe_title}</title></head>\
         <body style=\"font-family:-apple-system,system-ui,sans-serif;padding:48px;max-width:520px;margin:auto\">\
         <h2>{safe_title}</h2><p>{safe_message}</p></body></html>"
    );
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.flush().await
}

async fn exchange_code_for_tokens(code: &str, verifier: &str) -> Result<ChatGptAccountCredentials, String> {
    let body = format!(
        "grant_type=authorization_code&client_id={}&code={}&redirect_uri={}&code_verifier={}",
        url_encode_component(OPENAI_CODEX_CLIENT_ID),
        url_encode_component(code),
        url_encode_component(OPENAI_OAUTH_REDIRECT_URI),
        url_encode_component(verifier)
    );
    let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().map_err(|e| e.to_string())?;
    let resp = client
        .post(&resolve_token_url())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("OpenAI token exchange request failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("OpenAI token exchange failed ({status}): {text}"));
    }
    let parsed: Value = serde_json::from_str(&text).map_err(|e| format!("Invalid token JSON: {e}"))?;
    parse_token_response(&parsed, None)
}

async fn refresh_oauth_token(
    refresh_token: &str,
    prev: &ChatGptAccountCredentials,
) -> Result<ChatGptAccountCredentials, String> {
    let body = format!(
        "grant_type=refresh_token&client_id={}&refresh_token={}",
        url_encode_component(OPENAI_CODEX_CLIENT_ID),
        url_encode_component(refresh_token)
    );
    let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().map_err(|e| e.to_string())?;
    let resp = client
        .post(&resolve_token_url())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("OpenAI token refresh failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("OpenAI token refresh rejected ({status}): {text}"));
    }
    let parsed: Value = serde_json::from_str(&text).map_err(|e| format!("Invalid refresh JSON: {e}"))?;
    parse_token_response(&parsed, Some(prev))
}

fn parse_token_response(
    parsed: &Value,
    prev: Option<&ChatGptAccountCredentials>,
) -> Result<ChatGptAccountCredentials, String> {
    let access_token = parsed
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "OpenAI token response did not contain access_token.".to_string())?
        .to_string();
    let refresh_token = parsed
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| prev.and_then(|p| p.refresh_token.clone()));
    let id_token = parsed
        .get("id_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| prev.and_then(|p| p.id_token.clone()));

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
    let id = prev
        .map(|p| p.id.clone())
        .or_else(|| account_id.clone())
        .unwrap_or_else(|| format!("acc-{}", &encode_hex_lower(&random_bytes_32()[..6])));

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

async fn probe_codex_models(access_token: &str, account_id: Option<&str>) -> (bool, usize) {
    let Ok(client) = reqwest::Client::builder().timeout(Duration::from_secs(5)).build() else { return (false, 0); };
    let url = format!("{}/models?client_version={}", resolve_codex_base_url(), env!("CARGO_PKG_VERSION"));
    let mut req = client.get(&url).header("Accept", "application/json").header("originator", "codex_cli").bearer_auth(access_token);
    if let Some(id) = account_id.filter(|s| !s.is_empty()) {
        req = req.header("ChatGPT-Account-ID", id);
    }
    let Ok(resp) = req.send().await else { return (false, 0); };
    if !resp.status().is_success() {
        return (false, 0);
    }
    let Ok(body) = resp.json::<Value>().await else { return (false, 0); };
    let count = body.get("models").and_then(Value::as_array).or_else(|| body.as_array()).map(|a| a.len()).unwrap_or(0);
    (true, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plumbing::oauth_pkce::encode_base64url;

    fn sample_jwt(email: &str, account_id: &str, plan: &str) -> String {
        let header = encode_base64url(br#"{"alg":"none"}"#);
        let payload = json!({
            "email": email,
            "exp": 2_000_000_000u64,
            "https://api.openai.com/auth": { "chatgpt_account_id": account_id, "chatgpt_plan_type": plan }
        });
        format!("{header}.{}.sig", encode_base64url(payload.to_string().as_bytes()))
    }

    #[test]
    fn test_account_store_lifecycle_and_rpc_projection() {
        let dir = std::env::temp_dir().join(format!("heav3ns-acct-test-{}", now_ms()));
        let store = CodexAccountStore::at_path(dir.join("accounts.json"));
        let empty = store.rpc_snapshot();
        assert_eq!(empty["saved"], json!(false));
        assert_eq!(empty["accounts"], json!([]));

        let jwt1 = sample_jwt("one@example.com", "acct_1", "pro");
        let jwt2 = sample_jwt("two@example.com", "acct_2", "plus");
        let c1 = parse_token_response(&json!({ "access_token": jwt1, "refresh_token": "rt_1" }), None).unwrap();
        let c2 = parse_token_response(&json!({ "access_token": jwt2, "refresh_token": "rt_2" }), None).unwrap();

        let mut data = store.load_accounts_store();
        assert!(upsert_account(&mut data, c1));
        assert!(upsert_account(&mut data, c2));
        store.save_accounts_store(&data).unwrap();

        let snap = store.rpc_snapshot();
        assert_eq!(snap["saved"], json!(true));
        assert_eq!(snap["accounts"].as_array().unwrap().len(), 2);
        assert_eq!(snap["account"]["id"], json!("acct_2"));
        // Tokens must never be leaked in RPC snapshots.
        assert!(!snap.to_string().contains("rt_1"));
        assert!(!snap.to_string().contains("rt_2"));

        let switched = store.switch_account("acct_1").unwrap();
        assert_eq!(switched["account"]["id"], json!("acct_1"));

        let renamed = store.rename_account("acct_1", "Work Pro").unwrap();
        assert_eq!(renamed["account"]["label"], json!("Work Pro"));

        let auto_off = store.set_auto_switch(false).unwrap();
        assert_eq!(auto_off["autoSwitch"], json!(false));

        let removed = store.remove_account("acct_1").unwrap();
        assert_eq!(removed["accounts"].as_array().unwrap().len(), 1);
        assert_eq!(removed["account"]["id"], json!("acct_2"));

        let signed_out = store.sign_out(true).unwrap();
        assert_eq!(signed_out["saved"], json!(false));
        assert_eq!(signed_out["accounts"], json!([]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_loopback_oauth_callback_receives_code_and_validates_state() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let cancel = CancellationToken::new();
        let task = tokio::spawn(async move {
            wait_for_oauth_callback(listener, "expected_state_123", &cancel).await
        });

        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")).await.unwrap();
        stream.write_all(b"GET /auth/callback?code=auth_code_xyz&state=expected_state_123 HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();
        let mut resp = vec![0u8; 1024];
        let n = stream.read(&mut resp).await.unwrap();
        let text = String::from_utf8_lossy(&resp[..n]);
        assert!(text.contains("200 OK") && text.contains("You&#39;re connected"));
        assert_eq!(task.await.unwrap().unwrap(), "auth_code_xyz");
    }

    static ENV_TEST_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

    #[tokio::test]
    async fn test_single_flight_refresh_deduplication_and_429_auto_switch() {
        let _env_guard = ENV_TEST_LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock().await;
        let dir = std::env::temp_dir().join(format!("heav3ns-rtr-test-{}", now_ms()));
        let acct_path = dir.join("accounts.json");
        let store = CodexAccountStore::at_path(&acct_path);

        let stale_jwt = sample_jwt("one@example.com", "acct_1", "pro");
        let fresh_jwt = sample_jwt("one@example.com", "acct_1", "pro");
        let backup_jwt = sample_jwt("two@example.com", "acct_2", "plus");

        let mut c1 = parse_token_response(&json!({ "access_token": stale_jwt, "refresh_token": "rt_1" }), None).unwrap();
        c1.expires_at_ms = Some(1_000); // expired
        c1.access_token = "stale_access_token".to_string();
        let c2 = parse_token_response(&json!({ "access_token": backup_jwt, "refresh_token": "rt_2" }), None).unwrap();

        let mut data = store.load_accounts_store();
        upsert_account(&mut data, c2);
        upsert_account(&mut data, c1); // acct_1 is active
        store.save_accounts_store(&data).unwrap();

        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hits_srv = std::sync::Arc::clone(&hits);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let fresh_for_srv = fresh_jwt.clone();

        let srv = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf).await;
                hits_srv.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(40)).await;
                let body = json!({
                    "access_token": fresh_for_srv,
                    "refresh_token": "rt_1_rotated",
                    "expires_in": 3600
                })
                .to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            }
        });

        std::env::set_var("HEAV3NS_OPENAI_TOKEN_URL", format!("http://127.0.0.1:{port}/oauth/token"));
        let mut tasks = Vec::new();
        for _ in 0..5 {
            let s = store.clone();
            tasks.push(tokio::spawn(async move { s.get_valid_active_credentials().await }));
        }
        for t in tasks {
            let (tok, acct) = t.await.unwrap().unwrap().unwrap();
            assert_eq!(tok, fresh_jwt);
            assert_eq!(acct.as_deref(), Some("acct_1"));
        }
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1, "Single-flight refresh must coalesce concurrent callers");

        // Verify 429 rate-limit auto-switches to acct_2
        store.record_rate_limit(Some(60_000));
        let snap = store.rpc_snapshot();
        assert_eq!(snap["account"]["id"], json!("acct_2"));

        std::env::remove_var("HEAV3NS_OPENAI_TOKEN_URL");
        srv.abort();
        let _ = fs::remove_dir_all(&dir);
    }
}
