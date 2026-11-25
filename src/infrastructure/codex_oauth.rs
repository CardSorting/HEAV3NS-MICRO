// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::{BoxFuture, ICodexAccountStore};
use crate::domain::types::account_types::{ChatGptAccountCredentials, ChatGptAccountStoreData};
use crate::domain::types::runtime::CancellationToken;
use crate::plumbing::hash_and_math::{lock_unpoison, now_ms};
use crate::plumbing::oauth_pkce::{build_openai_authorize_url, encode_base64url, encode_hex_lower, generate_pkce_challenge};
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex as AsyncMutex;

static REFRESH_FLIGHT_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
pub use crate::infrastructure::codex_oauth_http::{CHATGPT_CODEX_BASE_URL, OPENAI_CODEX_CLIENT_ID, OPENAI_OAUTH_AUTH_URL, OPENAI_OAUTH_REDIRECT_URI, OPENAI_OAUTH_SCOPES, OPENAI_OAUTH_TOKEN_URL};
pub(crate) use crate::infrastructure::codex_oauth_http::{build_codex_oauth_post, random_bytes_32, resolve_codex_base_url, TOKEN_REFRESH_SKEW_MS};
use crate::infrastructure::codex_oauth_http::{bind_oauth_callback_listener, exchange_code_for_tokens, probe_codex_models, refresh_oauth_token, wait_for_oauth_callback};
static STORE_IO_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
fn refresh_flight_lock() -> &'static AsyncMutex<()> {
    REFRESH_FLIGHT_LOCK.get_or_init(|| AsyncMutex::new(()))
}
fn store_io_lock() -> &'static std::sync::Mutex<()> {
    STORE_IO_LOCK.get_or_init(|| std::sync::Mutex::new(()))
}


#[derive(Debug, Clone)]
pub struct CodexAccountStore {
    path: PathBuf,
}

impl CodexAccountStore {
    pub fn default_store() -> Self {
        Self {
            path: resolve_accounts_path(),
        }
    }

    pub fn at_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn try_load_accounts_store(&self) -> Result<ChatGptAccountStoreData, String> {
        match fs::read_to_string(&self.path) {
            Ok(raw) => {
                serde_json::from_str(&raw).map_err(|e| format!("Corrupt accounts store: {e}"))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(ChatGptAccountStoreData::default())
            }
            Err(e) => Err(format!("Failed reading accounts store: {e}")),
        }
    }
    pub fn load_accounts_store(&self) -> ChatGptAccountStoreData {
        self.try_load_accounts_store().unwrap_or_default()
    }

    pub fn save_accounts_store(&self, data: &ChatGptAccountStoreData) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create accounts dir: {e}"))?;
        }
        let serialized = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Failed to serialize accounts: {e}"))?;
        static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = self
            .path
            .with_extension(format!("json.tmp.{}.{}", std::process::id(), seq));
        let _ = fs::remove_file(&tmp);
        #[cfg(unix)]
        {
            use std::{io::Write, os::unix::fs::OpenOptionsExt};
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)
                .and_then(|mut f| f.write_all(serialized.as_bytes()))
                .map_err(|e| format!("Failed to write accounts file: {e}"))?;
        }
        #[cfg(not(unix))]
        fs::write(&tmp, serialized.as_bytes())
            .map_err(|e| format!("Failed to write accounts file: {e}"))?;
        fs::rename(&tmp, &self.path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("Failed to persist accounts file: {e}")
        })
    }

    pub fn rpc_snapshot(&self) -> Value {
        snapshot_value(&self.load_accounts_store())
    }

    pub fn switch_account(&self, target_id: &str) -> Result<Value, String> {
        let _io = lock_unpoison(store_io_lock());
        let mut data = self.try_load_accounts_store()?;
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
        let _io = lock_unpoison(store_io_lock());
        let mut data = self.try_load_accounts_store()?;
        let Some(acc) = data.accounts.iter_mut().find(|a| a.id == target_id) else {
            return Err(format!("Account '{target_id}' was not found."));
        };
        acc.label = trimmed.chars().take(100).collect();
        acc.updated_at_ms = now_ms();
        self.save_accounts_store(&data)?;
        Ok(snapshot_value(&data))
    }

    pub fn remove_account(&self, target_id: &str) -> Result<Value, String> {
        let _io = lock_unpoison(store_io_lock());
        let mut data = self.try_load_accounts_store()?;
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
        let _io = lock_unpoison(store_io_lock());
        let mut data = self.try_load_accounts_store()?;
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
        let _io = lock_unpoison(store_io_lock());
        let mut data = self.try_load_accounts_store()?;
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
        let _io = lock_unpoison(store_io_lock());
        let mut data = self.try_load_accounts_store()?;
        data.auto_switch = enabled;
        self.save_accounts_store(&data)?;
        Ok(snapshot_value(&data))
    }

    /// Runs the full OpenAI OAuth PKCE browser sign-in flow and saves the resulting account.
    pub async fn sign_in<F>(
        &self,
        cancel: CancellationToken,
        on_browser_url: F,
    ) -> Result<Value, String>
    where
        F: FnOnce(String),
    {
        let verifier = encode_base64url(&random_bytes_32()?);
        let state = encode_hex_lower(&random_bytes_32()?[..16]);
        let challenge = generate_pkce_challenge(&verifier);
        let auth_url = build_openai_authorize_url(
            OPENAI_OAUTH_AUTH_URL,
            OPENAI_CODEX_CLIENT_ID,
            OPENAI_OAUTH_REDIRECT_URI,
            OPENAI_OAUTH_SCOPES,
            &challenge,
            &state,
        );

        let listener = bind_oauth_callback_listener(&cancel).await?;
        on_browser_url(auth_url);

        let code = wait_for_oauth_callback(listener, &state, &cancel).await?;
        if cancel.is_cancelled() {
            return Err("ChatGPT sign-in was cancelled.".to_string());
        }

        let creds = exchange_code_for_tokens(&code, &verifier).await?;
        let (access_checked, model_count) =
            probe_codex_models(&creds.access_token, creds.account_id.as_deref()).await;

        let (data, created) = {
            let _io = lock_unpoison(store_io_lock());
            let mut data = self.load_accounts_store();
            let created = upsert_account(&mut data, creds.clone());
            self.save_accounts_store(&data)?;
            (data, created)
        };

        let mut out = snapshot_value(&data);
        if let Some(obj) = out.as_object_mut() {
            obj.insert("created".to_string(), json!(created));
            obj.insert("accessChecked".to_string(), json!(access_checked));
            obj.insert("modelCount".to_string(), json!(model_count));
            obj.insert(
                "account".to_string(),
                json!({ "id": creds.id, "label": creds.label }),
            );
        }
        Ok(out)
    }

    /// Returns `(access_token, chatgpt_account_id)` for the active account, refreshing if near expiry.
    pub async fn get_valid_active_credentials(
        &self,
    ) -> Result<Option<(String, Option<String>)>, String> {
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
    pub async fn force_refresh_active(
        &self,
        rejected_token: &str,
    ) -> Result<(String, Option<String>), String> {
        let data = self.load_accounts_store();
        let active = active_account(&data)
            .ok_or_else(|| "No active ChatGPT account is signed in.".to_string())?;
        self.refresh_account(&active.id, Some(rejected_token)).await
    }

    async fn refresh_account(
        &self,
        id: &str,
        rejected_token: Option<&str>,
    ) -> Result<(String, Option<String>), String> {
        let _guard = refresh_flight_lock().lock().await;
        let data = self.load_accounts_store();
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
            .ok_or_else(|| {
                "ChatGPT session expired and has no refresh token. Sign in again.".to_string()
            })?;

        let refreshed = refresh_oauth_token(&refresh_tok, &current).await?;
        let result = (refreshed.access_token.clone(), refreshed.account_id.clone());
        let _io = lock_unpoison(store_io_lock());
        let mut data = self.try_load_accounts_store()?;
        if let Some(slot) = data.accounts.iter_mut().find(|a| a.id == id) {
            slot.access_token = refreshed.access_token;
            slot.refresh_token = refreshed.refresh_token;
            slot.id_token = refreshed.id_token;
            slot.expires_at_ms = refreshed.expires_at_ms;
            slot.account_id = refreshed.account_id;
            slot.email = refreshed.email;
            slot.plan_type = refreshed.plan_type;
            slot.updated_at_ms = refreshed.updated_at_ms;
        }
        self.save_accounts_store(&data)?;
        Ok(result)
    }

    /// Records a 429 usage limit on the active account and auto-switches if another ready account exists.
    pub fn record_rate_limit(&self, retry_after_ms: Option<u64>) {
        let _io = lock_unpoison(store_io_lock());
        let Ok(mut data) = self.try_load_accounts_store() else {
            return;
        };
        let now = now_ms();
        let until = now.saturating_add(retry_after_ms.unwrap_or(15 * 60_000).max(5_000));
        if let Some(active_id) = data.active_id.clone() {
            if let Some(acc) = data.accounts.iter_mut().find(|a| a.id == active_id) {
                acc.limited_until_ms = Some(until);
                acc.limit_estimated = retry_after_ms.is_none();
                acc.updated_at_ms = now;
            }
            if data.auto_switch {
                if let Some(next) = data
                    .accounts
                    .iter()
                    .find(|a| a.id != active_id && a.limited_until_ms.is_none_or(|u| u <= now))
                {
                    data.active_id = Some(next.id.clone());
                }
            }
            if let Err(e) = self.save_accounts_store(&data) {
                eprintln!("[heav3ns] Failed to persist rate limit: {e}");
            }
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
    let active_summary = summaries
        .iter()
        .find(|s| s.active)
        .or_else(|| summaries.first());
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

pub(crate) fn upsert_account(
    data: &mut ChatGptAccountStoreData,
    mut incoming: ChatGptAccountCredentials,
) -> bool {
    let existing_idx = data.accounts.iter().position(|a| {
        (incoming.account_id.is_some() && a.account_id == incoming.account_id)
            || (incoming.email.is_some() && a.email == incoming.email)
            || a.id == incoming.id
    });
    if let Some(idx) = existing_idx {
        incoming.id = data.accounts[idx].id.clone();
        if !data.accounts[idx].label.is_empty() && !data.accounts[idx].label.starts_with("ChatGPT")
        {
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


