// [LAYER: DOMAIN]
use crate::domain::types::progress::EngineProgressEvent;
use crate::plumbing::seatbelt_profile::SeatbeltSandboxMode;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::watch;

pub type SharedCancelPredicate = Arc<dyn Fn() -> bool + Send + Sync>;
pub type SharedProgressListener = Arc<dyn Fn(EngineProgressEvent) + Send + Sync>;
pub type ApprovalGateCallback = Arc<dyn Fn(&str, &str, &str) -> UserApprovalChoice + Send + Sync>;

/// Abstract contract for agent runtime configuration envelopes.
pub trait IRuntimeConfig {
    fn active_model_name(&self) -> &str;
    fn context_window(&self) -> usize;
}

/// Interactive user response to a tool/command approval prompt (`y` once / `a` session / `n` deny).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserApprovalChoice {
    AllowOnce,
    AllowSession,
    Deny,
}

/// Unified 5-stop Permission & Mode Dial (`Shift+Tab`) distilled from Codewhale (`MODES.md` / `AUTHORIZATION_ORDER.md`).
/// Cycle order: `Ask` -> `AcceptEdits` (default) -> `Plan` -> `AutoReview` -> `FullAccess`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPosture {
    Ask,
    AcceptEdits,
    Plan,
    AutoReview,
    FullAccess,
}

impl Default for ExecutionPosture {
    fn default() -> Self {
        Self::AcceptEdits
    }
}

impl ExecutionPosture {
    #[inline]
    pub const fn step_next(self) -> Self {
        match self {
            Self::Ask => Self::AcceptEdits,
            Self::AcceptEdits => Self::Plan,
            Self::Plan => Self::AutoReview,
            Self::AutoReview => Self::FullAccess,
            Self::FullAccess => Self::Ask,
        }
    }

    pub fn parse_label(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "ask" | "suggest" | "interactive" => Some(Self::Ask),
            "accept-edits" | "acceptedits" | "balanced" | "work" => Some(Self::AcceptEdits),
            "plan" | "read-only" | "readonly" | "strict" => Some(Self::Plan),
            "auto-review" | "autoreview" | "review" | "guardian" => Some(Self::AutoReview),
            "full-access" | "fullaccess" | "auto" | "autonomous" | "bypass" => Some(Self::FullAccess),
            _ => None,
        }
    }

    #[inline]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::AcceptEdits => "accept_edits",
            Self::Plan => "plan",
            Self::AutoReview => "auto_review",
            Self::FullAccess => "full_access",
        }
    }

    #[inline]
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Ask => "Ask",
            Self::AcceptEdits => "Accept Edits",
            Self::Plan => "Plan",
            Self::AutoReview => "Auto-Review",
            Self::FullAccess => "Full Access",
        }
    }

    #[inline]
    pub const fn to_authority_level(self) -> ExecutionAuthorityLevel {
        match self {
            Self::Plan => ExecutionAuthorityLevel::Strict,
            Self::Ask => ExecutionAuthorityLevel::Interactive,
            Self::AcceptEdits => ExecutionAuthorityLevel::Balanced,
            Self::AutoReview => ExecutionAuthorityLevel::HighThroughput,
            Self::FullAccess => ExecutionAuthorityLevel::Autonomous,
        }
    }

    #[inline]
    pub const fn from_authority_level(level: ExecutionAuthorityLevel) -> Self {
        match level {
            ExecutionAuthorityLevel::Strict => Self::Plan,
            ExecutionAuthorityLevel::Interactive => Self::Ask,
            ExecutionAuthorityLevel::Balanced => Self::AcceptEdits,
            ExecutionAuthorityLevel::HighThroughput => Self::AutoReview,
            ExecutionAuthorityLevel::Autonomous => Self::FullAccess,
        }
    }

    #[inline]
    pub const fn sandbox_mode(self) -> SeatbeltSandboxMode {
        match self {
            Self::Plan => SeatbeltSandboxMode::ReadOnly,
            Self::Ask | Self::AcceptEdits | Self::AutoReview => SeatbeltSandboxMode::WorkspaceWrite { allow_network: true },
            Self::FullAccess => SeatbeltSandboxMode::Unrestricted,
        }
    }
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
    pub posture: Option<ExecutionPosture>,
    pub cancel_token: Option<CancellationToken>,
    pub should_defer: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    pub on_progress: Option<Arc<dyn Fn(EngineProgressEvent) + Send + Sync>>,
    pub approval_callback: Option<ApprovalGateCallback>,
}

impl EngineTickInput {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            ..Default::default()
        }
    }

    pub fn with_posture(mut self, posture: ExecutionPosture) -> Self {
        self.posture = Some(posture);
        self.execution_authority = Some(posture.to_authority_level());
        self
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
