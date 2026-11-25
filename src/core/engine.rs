// [LAYER: CORE]
use crate::core::request_queue::RuntimeRequestQueue;
use crate::core::session_store::{SessionCompactor, SessionStore};
use crate::core::tool_registry::ToolRegistry;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::interfaces::{
    ILlmProvider, IWorkspaceSnapshotCas,
};
use crate::domain::memory::{GameStateSnapshot, MemoryStore};
use crate::domain::prefix_and_manifest::PrefixStabilityTracker;
use crate::domain::pricing_and_taxonomy::ProviderTokenUsage;
use crate::domain::turn_receipt::TurnReceipt;
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::progress::{
    EngineProgressMetadata, EngineProgressPhase, EngineProgressStatus,
    ToolExecutionRecord,
};
use crate::domain::types::runtime::{
    AgentConfig, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel, ExecutionPosture,
};
use crate::domain::types::tick_result::EngineTickResult;
use crate::infrastructure::vfs_store::SessionVfs;
use crate::infrastructure::workspace_cas::WorkspaceSnapshotCas;
use crate::plumbing::hash_and_math::now_ms;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Computes the content-addressed deterministic revision of a workspace tree.
pub fn compute_workspace_revision(root: &Path) -> String {
    crate::infrastructure::vfs_store::workspace_revision(root)
}

/// Core Agent Engine orchestrating turns, KV-pinned prefix stability, posture gates, CAS receipts, and swarms.
pub struct AgentEngine {
    pub(crate) workspace_root: PathBuf,
    pub(crate) config: Mutex<AgentConfig>,
    pub(crate) session: Mutex<SessionStore>,
    pub(crate) vfs: Mutex<SessionVfs>,
    pub(crate) memory: Arc<Mutex<MemoryStore>>,
    pub(crate) compactor: SessionCompactor,
    pub(crate) tools: Arc<ToolRegistry>,
    pub(crate) provider: Mutex<Arc<dyn ILlmProvider>>,
    pub(crate) queue: Arc<RuntimeRequestQueue>,
    pub(crate) snapshots: Mutex<Vec<GameStateSnapshot>>,
    pub(crate) snapshot_cas_map: Mutex<BTreeMap<usize, String>>,
    pub(crate) cas: Mutex<WorkspaceSnapshotCas>,
    pub(crate) prefix_tracker: Mutex<PrefixStabilityTracker>,
    pub(crate) pinned_system_prompt: Mutex<Option<String>>,
    pub(crate) last_prefix_status: Mutex<String>,
    pub(crate) active_posture: Mutex<Option<ExecutionPosture>>,
    pub(crate) turn_receipts: Mutex<Vec<TurnReceipt>>,
    pub(crate) frame_counter: AtomicU64,
    pub(crate) event_seq: AtomicU64,
    pub(crate) forbid_swarm_delegation: bool,
}


impl AgentEngine {
    pub async fn tick(&self, input: EngineTickInput) -> EngineTickResult {
        let started_ms = now_ms();
        let frame_index = self.frame_counter.fetch_add(1, Ordering::Relaxed) + 1;
        let turn_activity_id = format!("turn-{frame_index}");
        let cfg = self
            .config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let authority = input
            .execution_authority
            .unwrap_or(ExecutionAuthorityLevel::Autonomous);
        let effective_posture = if let Some(p) = input.posture {
            p
        } else if let Some(explicit) = *self
            .active_posture
            .lock()
            .unwrap_or_else(|e| e.into_inner())
        {
            explicit
        } else {
            match authority {
                ExecutionAuthorityLevel::Strict | ExecutionAuthorityLevel::Interactive => {
                    ExecutionPosture::Ask
                }
                ExecutionAuthorityLevel::Balanced => ExecutionPosture::AcceptEdits,
                ExecutionAuthorityLevel::HighThroughput | ExecutionAuthorityLevel::Autonomous => {
                    ExecutionPosture::AutoReview
                }
            }
        };

        let _permit = match self.queue.try_acquire(input.prompt.len()) {
            Ok(p) => p,
            Err(err_msg) => {
                self.emit_progress_internal(
                    &input.on_progress,
                    &turn_activity_id,
                    EngineProgressPhase::Failed,
                    EngineProgressStatus::Failed,
                    &err_msg,
                    None,
                    started_ms,
                    None,
                );
                return EngineTickResult {
                    frame_index,
                    outcome: EngineTickOutcome::Failed,
                    active_model: cfg.model_name,
                    is_fallback_model: false,
                    is_slash_command: false,
                    composed_prompt: String::new(),
                    response: err_msg,
                    tool_results: Vec::new(),
                    safe_to_retry: Some(true),
                    duration_ms: now_ms().saturating_sub(started_ms),
                    receipt: None,
                };
            }
        };

        if input.should_defer.as_ref().map(|f| f()).unwrap_or(false) {
            let msg = "Turn deferred due to higher-priority host activity.".to_string();
            self.emit_progress_internal(
                &input.on_progress,
                &turn_activity_id,
                EngineProgressPhase::Deferred,
                EngineProgressStatus::Deferred,
                &msg,
                None,
                started_ms,
                None,
            );
            return EngineTickResult {
                frame_index,
                outcome: EngineTickOutcome::Deferred,
                active_model: cfg.model_name,
                is_fallback_model: false,
                is_slash_command: false,
                composed_prompt: String::new(),
                response: msg,
                tool_results: Vec::new(),
                safe_to_retry: Some(true),
                duration_ms: now_ms().saturating_sub(started_ms),
                receipt: None,
            };
        }

        if let Some(reply) = self.handle_slash_command(&input.prompt) {
            let active_model = self
                .config
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .model_name
                .clone();
            self.emit_progress_internal(
                &input.on_progress,
                &turn_activity_id,
                EngineProgressPhase::Completed,
                EngineProgressStatus::Completed,
                "Slash command executed",
                None,
                started_ms,
                None,
            );
            return EngineTickResult {
                frame_index,
                outcome: EngineTickOutcome::Completed,
                active_model,
                is_fallback_model: false,
                is_slash_command: true,
                composed_prompt: input.prompt,
                response: reply,
                tool_results: Vec::new(),
                safe_to_retry: Some(true),
                duration_ms: now_ms().saturating_sub(started_ms),
                receipt: None,
            };
        }

        let trimmed_prompt = input.prompt.trim();
        if let Some(fact) = trimmed_prompt
            .strip_prefix("remember:")
            .or_else(|| trimmed_prompt.strip_prefix("Remember:"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let entry = self
                .memory
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .save_memory(format!("fact_{}", now_ms()), fact, "fact");
            let reply = format!("Saved to LUMI-MEMORY/1 ({}): {}", entry.key, entry.value);
            return EngineTickResult {
                frame_index,
                outcome: EngineTickOutcome::Completed,
                active_model: cfg.model_name,
                is_fallback_model: false,
                is_slash_command: false,
                composed_prompt: input.prompt,
                response: reply,
                tool_results: Vec::new(),
                safe_to_retry: Some(true),
                duration_ms: now_ms().saturating_sub(started_ms),
                receipt: None,
            };
        }

        let turn_num = {
            let mut sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
            let turn = sess.increment_turn();
            sess.add_message(SessionMessage::new(Role::User, input.prompt.clone()));
            if sess.messages().len() > cfg.max_turns {
                sess.compact_session(
                    &self.compactor,
                    &CompactionPolicy {
                        max_messages: Some(cfg.max_turns),
                        preserve_recent_turns: 4,
                        ..Default::default()
                    },
                );
            }
            turn
        };

        let constitution = Self::load_constitution(&self.workspace_root);
        self.tools.configure_turn_security(
            Some(effective_posture),
            constitution,
            input.approval_callback.clone(),
        );
        struct TurnApprovalGuard<'a>(&'a crate::core::tool_registry::ToolRegistry);
        impl Drop for TurnApprovalGuard<'_> {
            fn drop(&mut self) {
                self.0.clear_turn_approval_gate();
            }
        }
        let _approval_guard = TurnApprovalGuard(&self.tools);
        self.tools.begin_turn(turn_num);

        let snap_before = self
            .cas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .capture_snapshot("pre-turn", turn_num)
            .ok();

        let max_rounds = cfg.max_tool_rounds.unwrap_or(32).max(1);
        let auto_decision = Self::route_turn_model(&cfg.provider, &cfg.model_name, &input.prompt);
        let active_model = auto_decision.selected_model;
        let is_fallback_model = false;
        let turn_route: Option<(Arc<dyn ILlmProvider>, String, String)> = None;
        let all_tool_results: Vec<ToolExecutionRecord> = Vec::new();
        let tools_admitted = false;
        let last_composed_summary = String::new();
        let final_response = String::new();
        let accumulated_usage = ProviderTokenUsage::default();
        let last_prefix_hash = String::new();
        let last_prefix_status = "pinned_initial".to_string();
        let completed_with_assistant_reply = false;

        self.emit_progress_internal(
            &input.on_progress,
            &turn_activity_id,
            EngineProgressPhase::Planning,
            EngineProgressStatus::Started,
            "Preparing context and starting agent turn",
            None,
            started_ms,
            Some(EngineProgressMetadata {
                source: Some("engine".to_string()),
                scope: Some("turn".to_string()),
                attempt: Some(1),
                item_type: None,
                completed_steps: Some(0),
                total_steps: Some(max_rounds),
            }),
        );

        self.run_tick_rounds(
            input,
            &cfg,
            turn_num,
            frame_index,
            effective_posture,
            authority,
            snap_before,
            started_ms,
            &turn_activity_id,
            max_rounds,
            active_model,
            is_fallback_model,
            turn_route,
            all_tool_results,
            tools_admitted,
            last_composed_summary,
            final_response,
            accumulated_usage,
            last_prefix_hash,
            last_prefix_status,
            completed_with_assistant_reply,
        ).await
    }
}
