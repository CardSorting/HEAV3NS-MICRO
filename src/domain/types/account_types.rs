// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};

/// Persisted ChatGPT / OpenAI Codex OAuth credentials for one account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptAccountCredentials {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limited_until_ms: Option<u64>,
    #[serde(default)]
    pub limit_estimated: bool,
    #[serde(default)]
    pub updated_at_ms: u64,
}

/// RPC-safe projection of a saved ChatGPT account (never carries tokens or email).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptAccountSummary {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    pub active: bool,
    pub limited: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limited_until: Option<u64>,
    pub limit_estimated: bool,
    pub can_refresh: bool,
}

/// On-disk multi-account ChatGPT OAuth store snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptAccountStoreData {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_id: Option<String>,
    #[serde(default = "default_auto_switch")]
    pub auto_switch: bool,
    #[serde(default)]
    pub accounts: Vec<ChatGptAccountCredentials>,
}

fn default_auto_switch() -> bool {
    true
}

impl Default for ChatGptAccountStoreData {
    fn default() -> Self {
        Self {
            version: 1,
            active_id: None,
            auto_switch: true,
            accounts: Vec::new(),
        }
    }
}

impl ChatGptAccountStoreData {
    /// Projects safe account summaries without leaking tokens or PII.
    pub fn summaries(&self, now_ms: u64) -> Vec<ChatGptAccountSummary> {
        self.accounts
            .iter()
            .take(16)
            .map(|acc| {
                let active = self.active_id.as_deref() == Some(acc.id.as_str());
                let limited = acc.limited_until_ms.is_some_and(|until| until > now_ms);
                ChatGptAccountSummary {
                    id: acc.id.clone(),
                    label: acc.label.chars().take(100).collect(),
                    plan_type: acc.plan_type.as_ref().map(|p| p.chars().take(32).collect()),
                    active,
                    limited,
                    limited_until: if limited { acc.limited_until_ms } else { None },
                    limit_estimated: acc.limit_estimated,
                    can_refresh: acc.refresh_token.as_deref().is_some_and(|r| !r.is_empty()),
                }
            })
            .collect()
    }
}
