// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type ActivitySequence = u64;
pub type ToolRecordBatch = Vec<ToolExecutionRecord>;

/// Abstract contract for structured runtime progress events.
pub trait IProgressEvent {
    fn phase(&self) -> EngineProgressPhase;
    fn status(&self) -> EngineProgressStatus;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineProgressPhase {
    Connecting,
    Thinking,
    Planning,
    Tool,
    Writing,
    Verifying,
    Responding,
    Completed,
    Deferred,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineProgressStatus {
    Denied,
    Started,
    InProgress,
    Completed,
    Deferred,
    Failed,
    Cancelled,
}

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
