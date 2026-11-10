// [LAYER: DOMAIN]
pub use crate::domain::pricing_and_taxonomy::ProviderTokenUsage;
pub use crate::domain::turn_receipt::{FileChangeReceipt, TurnReceipt};
use crate::domain::types::messages::{extract_text_tool_calls, SessionMessage, ToolCall};
use crate::domain::types::progress::ToolExecutionRecord;
use crate::domain::types::runtime::EngineTickOutcome;
use crate::plumbing::json_repair::normalize_completed_tool_call_arguments;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineTickResult {
    #[serde(rename = "frameIndex")]
    pub frame_index: u64,
    pub outcome: EngineTickOutcome,
    #[serde(rename = "activeModel")]
    pub active_model: String,
    #[serde(rename = "isFallbackModel")]
    pub is_fallback_model: bool,
    #[serde(rename = "isSlashCommand")]
    pub is_slash_command: bool,
    #[serde(rename = "composedPrompt")]
    pub composed_prompt: String,
    pub response: String,
    #[serde(rename = "toolResults")]
    pub tool_results: Vec<ToolExecutionRecord>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "safeToRetry")]
    pub safe_to_retry: Option<bool>,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<TurnReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub model: String,
    pub messages: Vec<SessionMessage>,
    pub tools: Vec<Value>,
    pub max_tokens: usize,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ProviderTokenUsage>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP {status}: {body}")]
    HttpStatus {
        status: u16,
        body: String,
        retryable: bool,
        retry_after_ms: Option<u64>,
    },
    #[error("Provider request cancelled by user")]
    Cancelled,
    #[error("Provider request timed out")]
    Timeout,
    #[error("Transport error: {0}")]
    Transport(String),
}

impl ProviderError {
    #[inline]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::HttpStatus { retryable, .. } => *retryable,
            Self::Timeout | Self::Transport(_) => true,
            Self::Cancelled => false,
        }
    }

    #[inline]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    #[inline]
    pub const fn retry_after_ms(&self) -> Option<u64> {
        match self {
            Self::HttpStatus { retry_after_ms, .. } => *retry_after_ms,
            _ => None,
        }
    }
}

#[inline]
pub const fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 425 | 429 | 500 | 502 | 503 | 504 | 529)
}

pub fn finalize_provider_response(
    mut content: Option<String>,
    mut tool_calls: Vec<ToolCall>,
    provider_label: &str,
) -> ProviderResponse {
    for tc in &mut tool_calls {
        tc.function.arguments = normalize_completed_tool_call_arguments(&tc.function.arguments, provider_label)
            .unwrap_or_else(|_| tc.function.arguments.clone());
    }
    if tool_calls.is_empty() {
        if let Some(ref text) = content {
            let extracted = extract_text_tool_calls(text);
            if !extracted.is_empty() {
                tool_calls = extracted;
            }
        }
    }
    content = content.filter(|s| !s.is_empty());
    ProviderResponse {
        content,
        tool_calls,
        usage: None,
    }
}
