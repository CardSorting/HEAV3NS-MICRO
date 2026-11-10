// [LAYER: DOMAIN]
use crate::domain::pricing_and_taxonomy::ProviderTokenUsage;
use serde::{Deserialize, Serialize};

/// Abstract contract for prose-free turn verification receipts (`heav3ns.receipt/v1`).
pub trait ITurnReceipt {
    fn receipt_schema(&self) -> &str;
    fn claim_ceiling_summary(&self) -> &str;
}

/// Deterministic file mutation receipt entry (`heav3ns.receipt/v1` — zero trust in model prose).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeReceipt {
    pub path: String,
    pub kind: String,
    #[serde(rename = "linesAdded")]
    pub lines_added: usize,
    #[serde(rename = "linesRemoved")]
    pub lines_removed: usize,
    #[serde(rename = "afterHash")]
    pub after_hash: String,
}

/// Deterministic shell command execution receipt entry (`heav3ns.receipt/v1`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRunReceipt {
    pub command: String,
    pub outcome: String,
    pub sandboxed: bool,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
}

/// Prose-free audit receipt for a turn (`heav3ns.receipt/v1`, distilled from Codewhale `RECEIPTS.md`).
/// Records only runtime-verified facts (CAS deltas, command outcomes, approvals, KV-cache status, honest billing)
/// and explicitly declares `not_recorded` + `claim_ceiling`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnReceipt {
    pub schema: String,
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "frameIndex")]
    pub frame_index: u64,
    #[serde(rename = "timestampMs")]
    pub timestamp_ms: u64,
    pub provider: String,
    pub model: String,
    pub posture: String,
    pub outcome: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    pub usage: ProviderTokenUsage,
    #[serde(rename = "billingDisplay")]
    pub billing_display: String,
    #[serde(rename = "billingKind")]
    pub billing_kind: String,
    #[serde(rename = "prefixHash")]
    pub prefix_hash: String,
    #[serde(rename = "prefixStatus")]
    pub prefix_status: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "snapshotBefore")]
    pub snapshot_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "snapshotAfter")]
    pub snapshot_after: Option<String>,
    #[serde(rename = "fileChanges")]
    pub file_changes: Vec<FileChangeReceipt>,
    pub commands: Vec<CommandRunReceipt>,
    pub approvals: Vec<String>,
    #[serde(rename = "toolsRun")]
    pub tools_run: usize,
    #[serde(rename = "notRecorded")]
    pub not_recorded: Vec<String>,
    #[serde(rename = "claimCeiling")]
    pub claim_ceiling: Vec<String>,
}

impl TurnReceipt {
    pub fn render_human_readable(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "┌─ Turn Receipt [{}] · {} · {} ({}) ─────────",
            self.turn_id, self.model, self.posture, self.outcome
        ));
        lines.push(format!(
            "│ Timing & Cache : {} ms · KV-Prefix {} ({})",
            self.duration_ms, self.prefix_hash, self.prefix_status
        ));
        lines.push(format!(
            "│ Usage & Cost   : in={}t (cached={}t) · out={}t ({}) · cost={}",
            self.usage.prompt_tokens,
            self.usage.cached_prompt_tokens,
            self.usage.completion_tokens,
            if self.usage.is_exact { "exact" } else { "estimated" },
            self.billing_display
        ));
        if let (Some(b), Some(a)) = (&self.snapshot_before, &self.snapshot_after) {
            lines.push(format!("│ CAS Snapshots  : {b} -> {a} (undoable via `/undo`)"));
        }
        if self.file_changes.is_empty() {
            lines.push("│ Files Changed  : (none)".to_string());
        } else {
            lines.push(format!("│ Files Changed  : {} file(s)", self.file_changes.len()));
            for fc in &self.file_changes {
                lines.push(format!(
                    "│   • [{}] {} (+{} / -{}) [{}]",
                    fc.kind, fc.path, fc.lines_added, fc.lines_removed, fc.after_hash
                ));
            }
        }
        if !self.commands.is_empty() {
            lines.push(format!("│ Commands Run   : {} command(s)", self.commands.len()));
            for c in &self.commands {
                lines.push(format!(
                    "│   • {} [{} · sandbox={} · {}ms]",
                    c.command, c.outcome, c.sandboxed, c.duration_ms
                ));
            }
        }
        if !self.approvals.is_empty() {
            lines.push(format!("│ Approvals      : {}", self.approvals.join("; ")));
        }
        lines.push(format!("│ Claim Ceiling  : {}", self.claim_ceiling.join(" | ")));
        lines.push("└──────────────────────────────────────────────────────────────────────────".to_string());
        lines.join("\n")
    }
}
