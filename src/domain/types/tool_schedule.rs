// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

pub type ScheduledWave = Vec<ScheduledToolCall>;

/// Abstract contract for scheduled tool call descriptors.
pub trait IScheduledCall {
    fn call_id(&self) -> &str;
    fn tool_name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct ScheduledToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationTransaction {
    pub id: String,
    pub turn_id: String,
    pub tool_name: String,
    pub target_path: PathBuf,
    pub previous_content: Option<Vec<u8>>,
    pub created_new_file: bool,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    #[serde(rename = "rolledBackCount")]
    pub rolled_back_count: usize,
    #[serde(rename = "restoredPaths")]
    pub restored_paths: Vec<String>,
    pub errors: Vec<String>,
}
