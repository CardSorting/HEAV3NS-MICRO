// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};

pub const CONTEXT_CHECKPOINT_PREFIX: &str = "LUMI-CONTEXT/1";
pub type CheckpointId = String;

/// Abstract contract for context window budget and compaction strategies.
pub trait ICompactionPolicy {
    fn is_forced(&self) -> bool;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ContextBudgetInfo {
    pub context_window_tokens: usize,
    pub reserved_output_tokens: usize,
    pub safety_margin_tokens: usize,
    pub available_input_tokens: usize,
    pub compaction_trigger_tokens: usize,
    pub target_input_tokens: usize,
}

impl ContextBudgetInfo {
    pub fn calculate(context_window_tokens: usize, requested_output_tokens: usize) -> Self {
        let window = context_window_tokens.max(4_096);
        let reserved_out = requested_output_tokens.clamp(256, window / 2);
        let safety = (window / 20).clamp(256, 4_096);
        let available_in = window.saturating_sub(reserved_out + safety).max(1_024);
        Self {
            context_window_tokens: window,
            reserved_output_tokens: reserved_out,
            safety_margin_tokens: safety,
            available_input_tokens: available_in,
            compaction_trigger_tokens: (available_in * 80) / 100,
            target_input_tokens: (available_in * 60) / 100,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompactionPolicy {
    pub max_messages: Option<usize>,
    pub max_input_tokens: Option<usize>,
    pub trigger_input_tokens: Option<usize>,
    pub target_input_tokens: Option<usize>,
    pub reserved_tokens: usize,
    pub preserve_recent_turns: usize,
    pub summary_max_tokens: usize,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionReport {
    pub compacted: bool,
    pub input_message_count: usize,
    pub output_message_count: usize,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub summarized_message_count: usize,
    pub checkpoint_id: Option<String>,
}
