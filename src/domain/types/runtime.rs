// [LAYER: DOMAIN]
use crate::domain::types::progress::EngineProgressEvent;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::watch;

pub type SharedCancelPredicate = Arc<dyn Fn() -> bool + Send + Sync>;
pub type SharedProgressListener = Arc<dyn Fn(EngineProgressEvent) + Send + Sync>;

/// Abstract contract for agent runtime configuration envelopes.
pub trait IRuntimeConfig {
    fn active_model_name(&self) -> &str;
    fn context_window(&self) -> usize;
}

#[derive(Clone, Debug)]
pub struct CancellationToken {
    tx: Arc<watch::Sender<bool>>,
    rx: watch::Receiver<bool>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self { tx: Arc::new(tx), rx }
    }

    pub fn cancel(&self) {
        let _ = self.tx.send(true);
    }

    #[inline]
    pub fn is_cancelled(&self) -> bool {
        *self.rx.borrow()
    }

    pub async fn cancelled(&self) {
        let mut rx = self.rx.clone();
        if *rx.borrow_and_update() {
            return;
        }
        while rx.changed().await.is_ok() {
            if *rx.borrow() {
                break;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionAuthorityLevel {
    Autonomous,
    HighThroughput,
    Balanced,
    Interactive,
    Strict,
}

impl ExecutionAuthorityLevel {
    #[inline]
    pub const fn is_autonomous(&self) -> bool {
        matches!(self, Self::Autonomous | Self::HighThroughput)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineTickOutcome {
    Completed,
    Failed,
    Cancelled,
    Deferred,
}

#[derive(Clone, Default)]
pub struct EngineTickInput {
    pub prompt: String,
    pub execution_authority: Option<ExecutionAuthorityLevel>,
    pub cancel_token: Option<CancellationToken>,
    pub should_defer: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    pub on_progress: Option<Arc<dyn Fn(EngineProgressEvent) + Send + Sync>>,
}

impl EngineTickInput {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub provider: String,
    pub model_name: String,
    pub fallback_model: Option<String>,
    pub system_prompt: String,
    pub max_turns: usize,
    pub max_tool_rounds: Option<usize>,
    pub context_window_tokens: usize,
    pub max_output_tokens: usize,
    pub temperature: f32,
    pub provider_timeout_ms: u64,
    pub api_endpoint: String,
    pub api_key: Option<String>,
}

pub const DEFAULT_SYSTEM_PROMPT: &str = "You are HEAV3NS-MICRO, a sovereign local-first software engineering agent. Own the requested outcome from inspection through verified completion.";

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model_name: "gpt-5.6-terra".to_string(),
            fallback_model: Some("gpt-4o-mini".to_string()),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            max_turns: 40,
            max_tool_rounds: Some(32),
            context_window_tokens: 128_000,
            max_output_tokens: 8_192,
            temperature: 0.0,
            provider_timeout_ms: 120_000,
            api_endpoint: "https://api.openai.com/v1".to_string(),
            api_key: std::env::var("OPENAI_API_KEY").ok().filter(|s| !s.is_empty()),
        }
    }
}
