// [LAYER: CORE]
use crate::core::request_queue::{PromptComposer, RuntimeRequestQueue};
use crate::core::session_store::{SessionCompactor, SessionStore};
use crate::core::tool_registry::ToolRegistry;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::interfaces::{ICodexAccountStore, ILlmProvider, IWorkspaceSnapshotCas, ProgressCallback};
use crate::domain::memory::{GameStateSnapshot, MemoryStore};
use crate::domain::prefix_and_manifest::{PrefixFingerprint, PrefixStabilityTracker};
use crate::domain::pricing_and_taxonomy::{ProviderTokenUsage, CREDENTIAL_REJECTED_UNSENT_CODE};
use crate::domain::turn_receipt::TurnReceipt;
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::progress::{EngineProgressEvent, EngineProgressMetadata, EngineProgressPhase, EngineProgressStatus, ToolExecutionRecord};
use crate::domain::types::runtime::{AgentConfig, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel, ExecutionPosture};
use crate::domain::types::tick_result::{EngineTickResult, ProviderError, ProviderRequest};
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::infrastructure::codex_oauth::CodexAccountStore;
use crate::infrastructure::http_provider::HttpProvider;
use crate::infrastructure::vfs_store::SessionVfs;
use crate::infrastructure::workspace_cas::WorkspaceSnapshotCas;
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::secret_redactor::DeterministicSecretRedactor;
use crate::plumbing::text_tokens::{canonical_tool_name, estimate_text_tokens};
use serde_json::{json, Value};
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
    workspace_root: PathBuf,
    config: Mutex<AgentConfig>,
    session: Mutex<SessionStore>,
    vfs: Mutex<SessionVfs>,
    memory: Arc<Mutex<MemoryStore>>,
    compactor: SessionCompactor,
    tools: Arc<ToolRegistry>,
    provider: Mutex<Arc<dyn ILlmProvider>>,
    queue: Arc<RuntimeRequestQueue>,
    snapshots: Mutex<Vec<GameStateSnapshot>>,
    snapshot_cas_map: Mutex<BTreeMap<usize, String>>,
    pub(crate) cas: Mutex<WorkspaceSnapshotCas>,
    pub(crate) prefix_tracker: Mutex<PrefixStabilityTracker>,
    pub(crate) pinned_system_prompt: Mutex<Option<String>>,
    pub(crate) last_prefix_status: Mutex<String>,
    pub(crate) active_posture: Mutex<Option<ExecutionPosture>>,
    pub(crate) turn_receipts: Mutex<Vec<TurnReceipt>>,
    frame_counter: AtomicU64,
    event_seq: AtomicU64,
    forbid_swarm_delegation: bool,
}

impl AgentEngine {
    pub extern "Rust" fn new(
        workspace_root: impl Into<PathBuf>,
        config: AgentConfig,
        provider: Arc<dyn ILlmProvider>,
    ) -> Self {
        let root = workspace_root.into();
        let memory = Arc::new(Mutex::new(MemoryStore::new()));
        let tools = Arc::new(ToolRegistry::new(Arc::clone(&memory)));
        Self::with_shared_substrate(root, config, provider, memory, tools)
    }

    pub fn with_shared_substrate(
        workspace_root: PathBuf,
        config: AgentConfig,
        provider: Arc<dyn ILlmProvider>,
        memory: Arc<Mutex<MemoryStore>>,
        tools: Arc<ToolRegistry>,
    ) -> Self {
        let mut session = SessionStore::new("default", &workspace_root, None);
        session.add_message(SessionMessage::new(Role::System, config.system_prompt.clone()));
        let cas = WorkspaceSnapshotCas::open(&workspace_root);
        let initial_receipts = cas.load_recent_receipts(32);
        Self {
            workspace_root,
            config: Mutex::new(config),
            session: Mutex::new(session),
            vfs: Mutex::new(SessionVfs::new()),
            memory,
            compactor: SessionCompactor::default(),
            tools,
            provider: Mutex::new(provider),
            queue: Arc::new(RuntimeRequestQueue::default()),
            snapshots: Mutex::new(Vec::new()),
            snapshot_cas_map: Mutex::new(BTreeMap::new()),
            cas: Mutex::new(cas),
            prefix_tracker: Mutex::new(PrefixStabilityTracker::default()),
            pinned_system_prompt: Mutex::new(None),
            last_prefix_status: Mutex::new("uninitialized".to_string()),
            active_posture: Mutex::new(None),
            turn_receipts: Mutex::new(initial_receipts),
            frame_counter: AtomicU64::new(0),
            event_seq: AtomicU64::new(0),
            forbid_swarm_delegation: false,
        }
    }

    pub fn without_swarm_delegation(mut self) -> Self {
        self.forbid_swarm_delegation = true;
        self
    }

    pub fn forbids_swarm_delegation(&self) -> bool {
        self.forbid_swarm_delegation
    }

    pub fn with_defaults(workspace_root: impl Into<PathBuf>) -> Self {
        let config = AgentConfig::default();
        let provider = Arc::new(HttpProvider::new(
            config.provider.clone(),
            config.api_endpoint.clone(),
            config.api_key.clone(),
            config.provider_timeout_ms,
        ));
        Self::new(workspace_root, config, provider)
    }

    pub fn workspace_root(&self) -> &Path { &self.workspace_root }

    /// Returns the active ChatGPT / OpenAI Codex OAuth account store via its domain interface.
    pub fn default_account_store() -> Arc<dyn ICodexAccountStore> {
        Arc::new(CodexAccountStore::default_store())
    }
    pub fn tools(&self) -> &Arc<ToolRegistry> { &self.tools }
    pub fn session(&self) -> &Mutex<SessionStore> { &self.session }
    pub fn vfs(&self) -> &Mutex<SessionVfs> { &self.vfs }
    pub fn memory(&self) -> &Arc<Mutex<MemoryStore>> { &self.memory }
    pub fn queue(&self) -> &Arc<RuntimeRequestQueue> { &self.queue }
    pub fn compactor(&self) -> &SessionCompactor { &self.compactor }

    pub(crate) fn provider_arc(&self) -> Arc<dyn ILlmProvider> {
        Arc::clone(&self.provider.lock().unwrap())
    }

    pub fn swap_provider_and_config(&self, new_cfg: AgentConfig, new_provider: Arc<dyn ILlmProvider>) {
        *self.config.lock().unwrap() = new_cfg;
        *self.provider.lock().unwrap() = new_provider;
        *self.pinned_system_prompt.lock().unwrap() = None;
        self.prefix_tracker.lock().unwrap().declare_repin("provider_or_model_switch");
    }

    pub fn config_snapshot(&self) -> AgentConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn set_posture(&self, posture: ExecutionPosture) {
        *self.active_posture.lock().unwrap() = Some(posture);
    }

    pub fn current_posture(&self) -> ExecutionPosture {
        self.active_posture.lock().unwrap().unwrap_or(ExecutionPosture::AcceptEdits)
    }

    pub fn step_posture(&self) -> ExecutionPosture {
        let next = self.current_posture().step_next();
        self.set_posture(next);
        next
    }

    pub(crate) fn set_active_model_name(&self, name: &str) {
        self.config.lock().unwrap().model_name = name.to_string();
        *self.pinned_system_prompt.lock().unwrap() = None;
        self.prefix_tracker.lock().unwrap().declare_repin("model_switch");
    }

    pub fn set_reasoning_effort(&self, effort: Option<String>) {
        self.config.lock().unwrap().reasoning_effort = effort.filter(|s| !s.trim().is_empty());
    }

    pub fn set_fallback_providers(&self, fallbacks: Vec<String>) {
        self.config.lock().unwrap().fallback_providers = fallbacks;
    }

    pub fn snapshots_list(&self) -> Vec<GameStateSnapshot> {
        self.snapshots.lock().unwrap().clone()
    }

    pub(crate) fn snapshot_count(&self) -> usize {
        self.snapshots.lock().unwrap().len()
    }

    pub(crate) fn snapshot_cas_map_clone(&self) -> BTreeMap<usize, String> {
        self.snapshot_cas_map.lock().unwrap().clone()
    }

    pub fn create_snapshot(&self) -> GameStateSnapshot {
        let session = self.session.lock().unwrap();
        let vfs = self.vfs.lock().unwrap();
        let memory = self.memory.lock().unwrap();
        let snap = session.create_snapshot(&vfs, &memory);
        let mut snaps = self.snapshots.lock().unwrap();
        let idx = snaps.len();
        snaps.push(snap.clone());
        if let Ok(cas_id) = self.cas.lock().unwrap().capture_snapshot(&snap.snapshot_id, snap.frame_index) {
            self.snapshot_cas_map.lock().unwrap().insert(idx, cas_id);
        }
        snap
    }

    pub fn rewind_to_snapshot(&self, index: usize) -> Result<GameStateSnapshot, String> {
        let snap = {
            let snaps = self.snapshots.lock().unwrap();
            snaps
                .get(index)
                .cloned()
                .ok_or_else(|| format!("Snapshot #{index} not found (total: {})", snaps.len()))?
        };
        if let Some(cas_id) = self.snapshot_cas_map.lock().unwrap().get(&index).cloned() {
            let _ = self.cas.lock().unwrap().restore_full_snapshot(&cas_id);
            self.tools.cache.lock().unwrap().clear();
            self.tools.read_tracker.lock().unwrap().reset_stamps();
        }
        let mut session = self.session.lock().unwrap();
        let mut vfs = self.vfs.lock().unwrap();
        let mut memory = self.memory.lock().unwrap();
        session.rewind_to_snapshot(&snap, &mut vfs, &mut memory);
        Ok(snap)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_progress_internal(
        &self,
        cb: &Option<ProgressCallback>,
        activity_id: &str,
        phase: EngineProgressPhase,
        status: EngineProgressStatus,
        message: impl Into<String>,
        detail: Option<String>,
        started_ms: u64,
        metadata: Option<EngineProgressMetadata>,
    ) {
        if let Some(callback) = cb {
            let seq = self.event_seq.fetch_add(1, Ordering::Relaxed) + 1;
            let now = now_ms();
            callback(EngineProgressEvent {
                activity_id: activity_id.to_string(),
                phase,
                status,
                message: message.into(),
                detail,
                timestamp: now,
                elapsed_ms: Some(now.saturating_sub(started_ms)),
                sequence: seq,
                metadata,
            });
        }
    }

    pub async fn tick(&self, input: EngineTickInput) -> EngineTickResult {
        let started_ms = now_ms();
        let frame_index = self.frame_counter.fetch_add(1, Ordering::Relaxed) + 1;
        let turn_activity_id = format!("turn-{frame_index}");
        let cfg = self.config.lock().unwrap().clone();
        let authority = input.execution_authority.unwrap_or(ExecutionAuthorityLevel::Autonomous);
        let effective_posture = if let Some(p) = input.posture {
            p
        } else if let Some(explicit) = *self.active_posture.lock().unwrap() {
            explicit
        } else {
            match authority {
                ExecutionAuthorityLevel::Strict | ExecutionAuthorityLevel::Interactive => ExecutionPosture::Ask,
                ExecutionAuthorityLevel::Balanced => ExecutionPosture::AcceptEdits,
                ExecutionAuthorityLevel::HighThroughput | ExecutionAuthorityLevel::Autonomous => ExecutionPosture::AutoReview,
            }
        };

        let _permit = match self.queue.try_acquire(input.prompt.len()) {
            Ok(p) => p,
            Err(err_msg) => {
                self.emit_progress_internal(&input.on_progress, &turn_activity_id, EngineProgressPhase::Failed, EngineProgressStatus::Failed, &err_msg, None, started_ms, None);
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
            self.emit_progress_internal(&input.on_progress, &turn_activity_id, EngineProgressPhase::Deferred, EngineProgressStatus::Deferred, &msg, None, started_ms, None);
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
            let active_model = self.config.lock().unwrap().model_name.clone();
            self.emit_progress_internal(&input.on_progress, &turn_activity_id, EngineProgressPhase::Completed, EngineProgressStatus::Completed, "Slash command executed", None, started_ms, None);
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
            let entry = self.memory.lock().unwrap().save_memory(format!("fact_{}", now_ms()), fact, "fact");
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
            let mut sess = self.session.lock().unwrap();
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
        self.tools.configure_turn_security(Some(effective_posture), constitution, input.approval_callback.clone());
        self.tools.begin_turn(turn_num);

        let snap_before = self
            .cas
            .lock()
            .unwrap()
            .capture_snapshot("pre-turn", turn_num)
            .ok();

        let max_rounds = cfg.max_tool_rounds.unwrap_or(32).max(1);
        let auto_decision = Self::route_turn_model(&cfg.provider, &cfg.model_name, &input.prompt);
        let mut active_model = auto_decision.selected_model;
        let mut is_fallback_model = false;
        let mut turn_route: Option<(Arc<dyn ILlmProvider>, String, String)> = None;
        let mut all_tool_results: Vec<ToolExecutionRecord> = Vec::new();
        let mut tools_admitted = false;
        let mut last_composed_summary = String::new();
        let mut final_response = String::new();
        let mut accumulated_usage = ProviderTokenUsage::default();
        let mut last_prefix_hash = String::new();
        let mut last_prefix_status = "pinned_initial".to_string();
        let mut completed_with_assistant_reply = false;

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

        for round in 0..max_rounds {
            if input.cancel_token.as_ref().map(|t| t.is_cancelled()).unwrap_or(false) {
                let msg = "Turn cancelled by user.".to_string();
                {
                    let mut sess = self.session.lock().unwrap();
                    Self::heal_dangling_tools(&mut sess, "[Turn cancelled by user]");
                }
                self.emit_progress_internal(&input.on_progress, &turn_activity_id, EngineProgressPhase::Cancelled, EngineProgressStatus::Cancelled, &msg, None, started_ms, None);
                let dur = now_ms().saturating_sub(started_ms);
                let (r_prov, r_ep) = turn_route.as_ref().map(|(_, p, e)| (p.as_str(), e.as_str())).unwrap_or((&cfg.provider, &cfg.api_endpoint));
                let receipt = self.build_and_persist_turn_receipt(
                    turn_num, frame_index, r_prov, r_ep, &active_model, effective_posture,
                    EngineTickOutcome::Cancelled, dur, accumulated_usage, last_prefix_hash, last_prefix_status, snap_before, &all_tool_results,
                );
                return EngineTickResult {
                    frame_index,
                    outcome: EngineTickOutcome::Cancelled,
                    active_model,
                    is_fallback_model,
                    is_slash_command: false,
                    composed_prompt: last_composed_summary,
                    response: msg,
                    tool_results: all_tool_results,
                    safe_to_retry: Some(!tools_admitted),
                    duration_ms: dur,
                    receipt: Some(receipt),
                };
            }

            if round > 0 && input.should_defer.as_ref().map(|f| f()).unwrap_or(false) {
                let msg = "Turn deferred between tool rounds.".to_string();
                {
                    let mut sess = self.session.lock().unwrap();
                    Self::heal_dangling_tools(&mut sess, "[Turn deferred between tool rounds]");
                }
                self.emit_progress_internal(&input.on_progress, &turn_activity_id, EngineProgressPhase::Deferred, EngineProgressStatus::Deferred, &msg, None, started_ms, None);
                let dur = now_ms().saturating_sub(started_ms);
                let (r_prov, r_ep) = turn_route.as_ref().map(|(_, p, e)| (p.as_str(), e.as_str())).unwrap_or((&cfg.provider, &cfg.api_endpoint));
                let receipt = self.build_and_persist_turn_receipt(
                    turn_num, frame_index, r_prov, r_ep, &active_model, effective_posture,
                    EngineTickOutcome::Deferred, dur, accumulated_usage, last_prefix_hash, last_prefix_status, snap_before, &all_tool_results,
                );
                return EngineTickResult {
                    frame_index,
                    outcome: EngineTickOutcome::Deferred,
                    active_model,
                    is_fallback_model,
                    is_slash_command: false,
                    composed_prompt: last_composed_summary,
                    response: msg,
                    tool_results: all_tool_results,
                    safe_to_retry: Some(!tools_admitted),
                    duration_ms: dur,
                    receipt: Some(receipt),
                };
            }

            let unlocked_before = self.tools.unlocked_tools.lock().unwrap().len();
            let routing_hint = {
                let sess = self.session.lock().unwrap();
                let tail = sess.messages().last().map(|m| m.content.as_str()).unwrap_or("");
                format!("{}\n{}", input.prompt, tail)
            };
            let openai_tools = self.tools.select_tool_schemas_for(&routing_hint, self.forbid_swarm_delegation);
            let unlocked_after = self.tools.unlocked_tools.lock().unwrap().len();
            if unlocked_after != unlocked_before {
                self.prefix_tracker.lock().unwrap().declare_repin("tool_schema_unlock");
            }

            let tool_defs_tokens = estimate_text_tokens(&Value::Array(openai_tools.clone()).to_string());
            let soul = Some(self.tools.skill_catalog.lock().unwrap().active_soul.clone());
            let spec = crate::domain::prompt_spec::SystemPromptSpec::for_turn(&cfg, authority, soul);
            let pinned_sys = self.pinned_system_prompt.lock().unwrap().clone();
            let (messages_for_provider, composed_summary) = {
                let sess = self.session.lock().unwrap();
                let mem = self.memory.lock().unwrap();
                PromptComposer::compose_with_posture(
                    &cfg,
                    &self.workspace_root,
                    &sess,
                    &mem,
                    tool_defs_tokens,
                    &spec,
                    effective_posture,
                    pinned_sys.as_deref(),
                )
            };
            if pinned_sys.is_none() {
                if let Some(sys_msg) = messages_for_provider.first().filter(|m| m.role == Role::System) {
                    *self.pinned_system_prompt.lock().unwrap() = Some(sys_msg.content.clone());
                }
            }
            let sys_str = messages_for_provider.first().map(|m| m.content.as_str()).unwrap_or("");
            let fp = PrefixFingerprint::from_wire_prefix(sys_str, &openai_tools);
            last_prefix_hash = fp.combined_hash.clone();
            last_prefix_status = self.prefix_tracker.lock().unwrap().record_prefix(fp).label();
            *self.last_prefix_status.lock().unwrap() = last_prefix_status.clone();
            last_composed_summary = composed_summary;

            let resp = match self
                .complete_round_with_backoff(
                    &cfg,
                    round,
                    max_rounds,
                    tools_admitted,
                    &mut active_model,
                    &mut is_fallback_model,
                    &mut turn_route,
                    &messages_for_provider,
                    &openai_tools,
                    &input.cancel_token,
                    &input.on_progress,
                    started_ms,
                )
                .await
            {
                Ok(r) => r,
                Err((ProviderError::Cancelled, _)) => {
                    let msg = "Turn cancelled by user.".to_string();
                    {
                        let mut sess = self.session.lock().unwrap();
                        Self::heal_dangling_tools(&mut sess, "[Turn cancelled by user]");
                    }
                    self.emit_progress_internal(&input.on_progress, &turn_activity_id, EngineProgressPhase::Cancelled, EngineProgressStatus::Cancelled, &msg, None, started_ms, None);
                    let dur = now_ms().saturating_sub(started_ms);
                    let (r_prov, r_ep) = turn_route.as_ref().map(|(_, p, e)| (p.as_str(), e.as_str())).unwrap_or((&cfg.provider, &cfg.api_endpoint));
                    let receipt = self.build_and_persist_turn_receipt(
                        turn_num, frame_index, r_prov, r_ep, &active_model, effective_posture,
                        EngineTickOutcome::Cancelled, dur, accumulated_usage, last_prefix_hash, last_prefix_status, snap_before, &all_tool_results,
                    );
                    return EngineTickResult {
                        frame_index,
                        outcome: EngineTickOutcome::Cancelled,
                        active_model,
                        is_fallback_model,
                        is_slash_command: false,
                        composed_prompt: last_composed_summary,
                        response: msg,
                        tool_results: all_tool_results,
                        safe_to_retry: Some(!tools_admitted),
                        duration_ms: dur,
                        receipt: Some(receipt),
                    };
                }
                Err((err, envelope)) => {
                    let is_unsent_auth = envelope
                        .as_ref()
                        .and_then(|e| e.code.as_deref())
                        == Some(CREDENTIAL_REJECTED_UNSENT_CODE);
                    if is_unsent_auth {
                        let mut sess = self.session.lock().unwrap();
                        sess.truncate_active_messages(1);
                    } else {
                        let mut sess = self.session.lock().unwrap();
                        Self::heal_dangling_tools(&mut sess, "[Turn halted due to provider error]");
                    }
                    let err_str = if let Some(env) = envelope {
                        let code_tag = env.code.as_deref().map(|c| format!("[{c}] ")).unwrap_or_default();
                        format!("{code_tag}{} — {}", env.message, env.remediation)
                    } else {
                        err.to_string()
                    };
                    self.emit_progress_internal(&input.on_progress, &turn_activity_id, EngineProgressPhase::Failed, EngineProgressStatus::Failed, &err_str, None, started_ms, None);
                    let dur = now_ms().saturating_sub(started_ms);
                    let (r_prov, r_ep) = turn_route.as_ref().map(|(_, p, e)| (p.as_str(), e.as_str())).unwrap_or((&cfg.provider, &cfg.api_endpoint));
                    let receipt = self.build_and_persist_turn_receipt(
                        turn_num, frame_index, r_prov, r_ep, &active_model, effective_posture,
                        EngineTickOutcome::Failed, dur, accumulated_usage, last_prefix_hash, last_prefix_status, snap_before, &all_tool_results,
                    );
                    return EngineTickResult {
                        frame_index,
                        outcome: EngineTickOutcome::Failed,
                        active_model,
                        is_fallback_model,
                        is_slash_command: false,
                        composed_prompt: last_composed_summary,
                        response: err_str,
                        tool_results: all_tool_results,
                        safe_to_retry: Some(!tools_admitted),
                        duration_ms: dur,
                        receipt: Some(receipt),
                    };
                }
            };

            let req_for_usage = ProviderRequest {
                model: active_model.clone(),
                messages: messages_for_provider,
                tools: openai_tools,
                max_tokens: cfg.max_output_tokens,
                temperature: cfg.temperature,
                reasoning_effort: cfg.reasoning_effort.clone(),
            };
            Self::accumulate_usage(&mut accumulated_usage, &req_for_usage, &resp);

            if resp.tool_calls.is_empty() {
                final_response = resp.content.unwrap_or_default();
                completed_with_assistant_reply = true;
                let mut sess = self.session.lock().unwrap();
                sess.add_message(SessionMessage::new(Role::Assistant, final_response.clone()));
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
                break;
            }

            tools_admitted = true;
            {
                let mut assistant_msg = SessionMessage::new(Role::Assistant, resp.content.clone().unwrap_or_default());
                assistant_msg.tool_calls = Some(resp.tool_calls.clone());
                self.session.lock().unwrap().add_message(assistant_msg);
            }

            let scheduled_calls: Vec<ScheduledToolCall> = resp
                .tool_calls
                .iter()
                .map(|tc| {
                    let (parsed_map, _) = self.tools.parser.parse_raw_arguments(&tc.function.arguments);
                    ScheduledToolCall {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        args: Value::Object(parsed_map),
                    }
                })
                .collect();

            let tool_desc: Vec<String> = scheduled_calls
                .iter()
                .map(|c| {
                    let target = c.args.get("path")
                        .or_else(|| c.args.get("CommandLine"))
                        .or_else(|| c.args.get("command"))
                        .or_else(|| c.args.get("query"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if target.is_empty() { c.name.clone() } else { format!("{}: {}", c.name, target) }
                })
                .collect();

            let batch_id = format!("tools-r{round}");
            self.emit_progress_internal(
                &input.on_progress,
                &batch_id,
                EngineProgressPhase::Tool,
                EngineProgressStatus::Started,
                tool_desc.join("; "),
                None,
                started_ms,
                Some(EngineProgressMetadata {
                    source: Some("scheduler".to_string()),
                    scope: Some("step".to_string()),
                    attempt: Some(1),
                    item_type: Some("tool".to_string()),
                    completed_steps: Some(0),
                    total_steps: Some(scheduled_calls.len()),
                }),
            );

            let records = self
                .execute_turn_tool_batch(&scheduled_calls, authority, input.cancel_token.as_ref())
                .await;

            {
                let mut sess = self.session.lock().unwrap();
                let mut gov = self.tools.governor.lock().unwrap();
                for (tc, mut rec) in resp.tool_calls.iter().zip(records.into_iter()) {
                    let is_skill_call = rec.name == "use_skill" || canonical_tool_name(&rec.name) == "use_skill";
                    let (out_text, spill_id) = if is_skill_call {
                        let raw = match &rec.output {
                            Value::String(s) => s.clone(),
                            other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
                        };
                        (DeterministicSecretRedactor::redact(&raw), None)
                    } else {
                        let governed = gov.govern(&rec.output);
                        if governed.was_truncated {
                            rec.output_limited = Some(true);
                        }
                        (governed.output_text, governed.spill_id)
                    };
                    if canonical_tool_name(&rec.name) == "tool_search" {
                        self.prefix_tracker.lock().unwrap().declare_repin("tool_search_unlock");
                    }

                    let wire_payload = if rec.success {
                        json!({ "success": true, "output": out_text, "spillId": spill_id })
                    } else {
                        json!({
                            "success": false,
                            "error": rec.error.clone().unwrap_or_else(|| "Tool execution failed".to_string()),
                            "output": out_text,
                        })
                    };

                    sess.add_message(SessionMessage::tool_result(
                        tc.id.clone(),
                        rec.name.clone(),
                        wire_payload.to_string(),
                    ));
                    all_tool_results.push(rec);
                }
            }

            let completed_desc: Vec<String> = all_tool_results
                .iter()
                .rev()
                .take(resp.tool_calls.len())
                .map(|r| format!("{} ({}, {}ms)", r.name, if r.success { "ok" } else { "err" }, r.duration_ms))
                .collect();

            self.emit_progress_internal(
                &input.on_progress,
                &batch_id,
                EngineProgressPhase::Tool,
                EngineProgressStatus::Completed,
                completed_desc.join("; "),
                None,
                started_ms,
                Some(EngineProgressMetadata {
                    source: Some("scheduler".to_string()),
                    scope: Some("step".to_string()),
                    attempt: Some(1),
                    item_type: Some("tool".to_string()),
                    completed_steps: Some(resp.tool_calls.len()),
                    total_steps: Some(resp.tool_calls.len()),
                }),
            );
        }

        if final_response.trim().is_empty() {
            final_response = if !all_tool_results.is_empty() {
                let success_count = all_tool_results.iter().filter(|r| r.success).count();
                let tool_names: Vec<String> = all_tool_results.iter().map(|r| r.name.clone()).collect();
                format!("Completed {} tool action(s) successfully: {}.", success_count, tool_names.join(", "))
            } else {
                "Turn completed with no text response from model.".to_string()
            };
        }

        // Heal dangling Role::Tool tail if max_rounds was reached without a final text message!
        if !completed_with_assistant_reply {
            let mut sess = self.session.lock().unwrap();
            Self::heal_dangling_tools(&mut sess, &final_response);
        }

        self.emit_progress_internal(&input.on_progress, &turn_activity_id, EngineProgressPhase::Completed, EngineProgressStatus::Completed, "Turn completed", None, started_ms, None);
        let dur = now_ms().saturating_sub(started_ms);
        let (r_prov, r_ep) = turn_route.as_ref().map(|(_, p, e)| (p.as_str(), e.as_str())).unwrap_or((&cfg.provider, &cfg.api_endpoint));
        let receipt = self.build_and_persist_turn_receipt(
            turn_num, frame_index, r_prov, r_ep, &active_model, effective_posture,
            EngineTickOutcome::Completed, dur, accumulated_usage, last_prefix_hash, last_prefix_status, snap_before, &all_tool_results,
        );

        EngineTickResult {
            frame_index,
            outcome: EngineTickOutcome::Completed,
            active_model,
            is_fallback_model,
            is_slash_command: false,
            composed_prompt: last_composed_summary,
            response: final_response,
            tool_results: all_tool_results,
            safe_to_retry: Some(!tools_admitted),
            duration_ms: dur,
            receipt: Some(receipt),
        }
    }
}
