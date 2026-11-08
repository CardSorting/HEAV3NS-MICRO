pub mod engine;
pub mod flight;
pub mod parser;
pub mod provider;
pub mod session;
pub mod tools;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::watch;

#[inline]
pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[derive(Clone, Debug)]
pub struct CancellationToken {
    tx: Arc<watch::Sender<bool>>,
    rx: watch::Receiver<bool>,
}

impl Default for CancellationToken {
    fn default() -> Self { Self::new() }
}

impl CancellationToken {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self { tx: Arc::new(tx), rx }
    }
    pub fn cancel(&self) { let _ = self.tx.send(true); }
    #[inline]
    pub fn is_cancelled(&self) -> bool { *self.rx.borrow() }
    pub async fn cancelled(&self) {
        let mut rx = self.rx.clone();
        if *rx.borrow_and_update() { return; }
        while rx.changed().await.is_ok() {
            if *rx.borrow() { break; }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role { System, User, Assistant, Tool }

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(default = "default_tool_type")]
    pub r#type: String,
    pub function: ToolCallFunction,
}

fn default_tool_type() -> String { "function".to_string() }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "toolCallId")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    pub timestamp: u64,
}

impl SessionMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self { role, content: content.into(), tool_call_id: None, name: None, tool_calls: None, timestamp: now_ms() }
    }
    pub fn tool_result(call_id: impl Into<String>, name: impl Into<String>, content: impl Into<String>) -> Self {
        Self { role: Role::Tool, content: content.into(), tool_call_id: Some(call_id.into()), name: Some(name.into()), tool_calls: None, timestamp: now_ms() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionAuthorityLevel { Autonomous, HighThroughput, Balanced, Interactive, Strict }

impl ExecutionAuthorityLevel {
    #[inline]
    pub fn is_autonomous(&self) -> bool { matches!(self, Self::Autonomous | Self::HighThroughput) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineProgressPhase {
    Connecting, Thinking, Planning, Tool, Writing, Verifying, Responding, Completed, Deferred, Failed, Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineProgressStatus { Denied, Started, InProgress, Completed, Deferred, Failed, Cancelled }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineProgressMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "itemType")]
    pub item_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "completedSteps")]
    pub completed_steps: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "totalSteps")]
    pub total_steps: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineProgressEvent {
    #[serde(rename = "activityId")]
    pub activity_id: String,
    pub phase: EngineProgressPhase,
    pub status: EngineProgressStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none", rename = "elapsedMs")]
    pub elapsed_ms: Option<u64>,
    pub sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<EngineProgressMetadata>,
}

pub type ProgressCallback = Arc<dyn Fn(EngineProgressEvent) + Send + Sync>;

#[derive(Clone, Default)]
pub struct EngineTickInput {
    pub prompt: String,
    pub execution_authority: Option<ExecutionAuthorityLevel>,
    pub cancel_token: Option<CancellationToken>,
    pub should_defer: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    pub on_progress: Option<ProgressCallback>,
}

impl EngineTickInput {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self { prompt: prompt.into(), ..Default::default() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineTickOutcome { Completed, Failed, Cancelled, Deferred }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "callId")]
    pub call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Value>,
    pub output: Value,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denied: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "outputLimited")]
    pub output_limited: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
}

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

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model_name: "gpt-5.6-terra".to_string(),
            fallback_model: Some("gpt-4o-mini".to_string()),
            system_prompt: "You are HEAV3NS-MICRO, a hyper-efficient local-first coding agent runtime. Inspect context, make scoped changes, verify evidence, and hand control back cleanly.".to_string(),
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
