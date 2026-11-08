use crate::parser::DeterministicSecretRedactor;
use crate::provider::{HttpProvider, LlmProvider, ProviderError, ProviderRequest};
use crate::session::{
    estimate_messages_tokens, estimate_text_tokens, workspace_revision, CompactionPolicy,
    ContextBudgetInfo, GameStateSnapshot, MemoryStore, SessionCompactor, SessionStore, SessionVfs,
    SkillsIngestor,
};
use crate::tools::{ScheduledToolCall, ToolRegistry};
use crate::{
    now_ms, AgentConfig, CancellationToken, EngineProgressEvent, EngineProgressMetadata,
    EngineProgressPhase, EngineProgressStatus, EngineTickInput, EngineTickOutcome,
    EngineTickResult, ExecutionAuthorityLevel, ProgressCallback, Role, SessionMessage,
    ToolExecutionRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

// ============================================================================
// 1. BOUNDED FIFO REQUEST QUEUE & BACKPRESSURE GUARD
// ============================================================================

#[derive(Debug, Clone)]
pub struct QueueState {
    pub active: bool,
    pub pending_count: usize,
    pub pending_bytes: usize,
    pub max_pending: usize,
    pub max_prompt_bytes: usize,
    pub max_total_bytes: usize,
}

pub struct RuntimeRequestQueue {
    state: Mutex<QueueState>,
}

pub struct QueuePermit {
    queue: Arc<RuntimeRequestQueue>,
    bytes: usize,
}

impl Drop for QueuePermit {
    fn drop(&mut self) {
        let mut st = self.queue.state.lock().unwrap();
        st.pending_count = st.pending_count.saturating_sub(1);
        st.pending_bytes = st.pending_bytes.saturating_sub(self.bytes);
        if st.pending_count == 0 {
            st.active = false;
        }
    }
}

impl Default for RuntimeRequestQueue {
    fn default() -> Self { Self::new(32, 1_048_576, 8_388_608) }
}

impl RuntimeRequestQueue {
    pub fn new(max_pending: usize, max_prompt_bytes: usize, max_total_bytes: usize) -> Self {
        Self {
            state: Mutex::new(QueueState {
                active: false, pending_count: 0, pending_bytes: 0, max_pending, max_prompt_bytes, max_total_bytes,
            }),
        }
    }

    pub fn try_acquire(self: &Arc<Self>, prompt_bytes: usize) -> Result<QueuePermit, String> {
        let mut st = self.state.lock().unwrap();
        if prompt_bytes > st.max_prompt_bytes {
            return Err(format!("RUNTIME_REQUEST_TOO_LARGE: prompt is {} bytes (max {} bytes)", prompt_bytes, st.max_prompt_bytes));
        }
        if st.pending_count >= st.max_pending || st.pending_bytes + prompt_bytes > st.max_total_bytes {
            return Err(format!("RUNTIME_QUEUE_FULL: {} pending requests ({} bytes queued)", st.pending_count, st.pending_bytes));
        }
        st.active = true;
        st.pending_count += 1;
        st.pending_bytes += prompt_bytes;
        Ok(QueuePermit { queue: Arc::clone(self), bytes: prompt_bytes })
    }

    pub fn snapshot(&self) -> QueueState { self.state.lock().unwrap().clone() }
}

// ============================================================================
// 2. TOPOLOGICAL SWARM COORDINATOR (DISTILLED FROM AGENT-SWARM-DISPATCHER)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmAssignment {
    pub id: String,
    pub role: String,
    pub objective: String,
    #[serde(default, rename = "dependsOn")]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmItemReport {
    pub id: String,
    pub role: String,
    pub outcome: EngineTickOutcome,
    pub response: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmFormationResult {
    pub objective: String,
    pub outcome: EngineTickOutcome,
    pub waves: usize,
    pub reports: Vec<SwarmItemReport>,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
}

// ============================================================================
// 3. PROMPT COMPOSER (WITH SKILLS INGESTOR & WORKSPACE REVISION)
// ============================================================================

pub struct PromptComposer;

impl PromptComposer {
    pub fn compose(
        config: &AgentConfig,
        workspace_root: &Path,
        session: &SessionStore,
        memory: &MemoryStore,
        tool_defs_tokens: usize,
    ) -> (Vec<SessionMessage>, String) {
        let skills = SkillsIngestor::discover(workspace_root);
        let skills_block = SkillsIngestor::format_skills_context(&skills)
            .map(|s| format!("\n\n{s}"))
            .unwrap_or_default();

        let sys_content = format!(
            "{}\n\n[Runtime Environment]\n- Workspace: {}\n- Model: {}\n- OS: {}\n- Context Envelope: Treat any assistant message starting with `LUMI-CONTEXT/1` or `LUMI-MEMORY/1` as a deterministic state reference, not a conversational reply.{}",
            config.system_prompt.trim(),
            workspace_root.display(),
            config.model_name,
            std::env::consts::OS,
            skills_block
        );

        let system_tokens = estimate_text_tokens(&sys_content);
        let budget = ContextBudgetInfo::calculate(config.context_window_tokens, config.max_output_tokens);
        let available_history_tokens = budget
            .available_input_tokens
            .saturating_sub(system_tokens + tool_defs_tokens)
            .max(512);

        let mut raw_messages = vec![SessionMessage::new(Role::System, sys_content)];
        if let Some(mem_envelope) = memory.format_memory_envelope() {
            raw_messages.push(SessionMessage::new(Role::Assistant, mem_envelope));
        }
        for msg in session.messages() {
            if msg.role != Role::System {
                raw_messages.push(msg.clone());
            }
        }

        let composed = SessionCompactor::truncate_to_token_budget(&raw_messages, available_history_tokens + system_tokens);
        let summary_prompt = format!("[system={}t, msgs={}, budget={}t]", system_tokens, composed.len(), available_history_tokens);
        (composed, summary_prompt)
    }
}

// ============================================================================
// 4. AGENT ENGINE
// ============================================================================

pub struct AgentEngine {
    workspace_root: PathBuf,
    config: Mutex<AgentConfig>,
    session: Mutex<SessionStore>,
    vfs: Mutex<SessionVfs>,
    memory: Arc<Mutex<MemoryStore>>,
    compactor: SessionCompactor,
    tools: Arc<ToolRegistry>,
    provider: Arc<dyn LlmProvider>,
    queue: Arc<RuntimeRequestQueue>,
    snapshots: Mutex<Vec<GameStateSnapshot>>,
    frame_counter: AtomicU64,
    event_seq: AtomicU64,
}

impl AgentEngine {
    pub fn new(workspace_root: impl Into<PathBuf>, config: AgentConfig, provider: Arc<dyn LlmProvider>) -> Self {
        let root = workspace_root.into();
        let memory = Arc::new(Mutex::new(MemoryStore::new()));
        let tools = Arc::new(ToolRegistry::new(Arc::clone(&memory)));
        Self::with_shared_substrate(root, config, provider, memory, tools)
    }

    pub fn with_shared_substrate(
        workspace_root: PathBuf,
        config: AgentConfig,
        provider: Arc<dyn LlmProvider>,
        memory: Arc<Mutex<MemoryStore>>,
        tools: Arc<ToolRegistry>,
    ) -> Self {
        let mut session = SessionStore::new("default", &workspace_root, None);
        session.add_message(SessionMessage::new(Role::System, config.system_prompt.clone()));
        Self {
            workspace_root,
            config: Mutex::new(config),
            session: Mutex::new(session),
            vfs: Mutex::new(SessionVfs::new()),
            memory,
            compactor: SessionCompactor::default(),
            tools,
            provider,
            queue: Arc::new(RuntimeRequestQueue::default()),
            snapshots: Mutex::new(Vec::new()),
            frame_counter: AtomicU64::new(0),
            event_seq: AtomicU64::new(0),
        }
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
    pub fn tools(&self) -> &Arc<ToolRegistry> { &self.tools }
    pub fn session(&self) -> &Mutex<SessionStore> { &self.session }
    pub fn vfs(&self) -> &Mutex<SessionVfs> { &self.vfs }
    pub fn memory(&self) -> &Arc<Mutex<MemoryStore>> { &self.memory }
    pub fn queue(&self) -> &Arc<RuntimeRequestQueue> { &self.queue }

    pub fn create_snapshot(&self) -> GameStateSnapshot {
        let session = self.session.lock().unwrap();
        let vfs = self.vfs.lock().unwrap();
        let memory = self.memory.lock().unwrap();
        let snap = session.create_snapshot(&vfs, &memory);
        self.snapshots.lock().unwrap().push(snap.clone());
        snap
    }

    pub fn rewind_to_snapshot(&self, index: usize) -> Result<GameStateSnapshot, String> {
        let snap = {
            let snaps = self.snapshots.lock().unwrap();
            snaps.get(index).cloned().ok_or_else(|| format!("Snapshot #{index} not found (total: {})", snaps.len()))?
        };
        let mut session = self.session.lock().unwrap();
        let mut vfs = self.vfs.lock().unwrap();
        let mut memory = self.memory.lock().unwrap();
        session.rewind_to_snapshot(&snap, &mut vfs, &mut memory);
        Ok(snap)
    }

    /// Executes a dependency-ordered multi-agent swarm formation in topological waves.
    pub async fn run_swarm(
        &self,
        objective: impl Into<String>,
        assignments: Vec<SwarmAssignment>,
        cancel_token: Option<CancellationToken>,
        should_defer: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    ) -> SwarmFormationResult {
        let started_ms = now_ms();
        let objective = objective.into();
        let mut pending: BTreeMap<String, SwarmAssignment> = assignments.into_iter().map(|a| (a.id.clone(), a)).collect();
        let mut completed: BTreeMap<String, SwarmItemReport> = BTreeMap::new();
        let mut reports = Vec::new();
        let mut waves = 0usize;

        while !pending.is_empty() {
            if cancel_token.as_ref().map(|t| t.is_cancelled()).unwrap_or(false) {
                return SwarmFormationResult {
                    objective, outcome: EngineTickOutcome::Cancelled, waves, reports, duration_ms: now_ms().saturating_sub(started_ms),
                };
            }
            if should_defer.as_ref().map(|f| f()).unwrap_or(false) {
                return SwarmFormationResult {
                    objective, outcome: EngineTickOutcome::Deferred, waves, reports, duration_ms: now_ms().saturating_sub(started_ms),
                };
            }

            let ready_ids: Vec<String> = pending
                .values()
                .filter(|a| a.depends_on.iter().all(|dep| completed.contains_key(dep)))
                .map(|a| a.id.clone())
                .collect();

            if ready_ids.is_empty() {
                // Circular or unresolvable dependency
                for (_, a) in pending {
                    reports.push(SwarmItemReport {
                        id: a.id,
                        role: a.role,
                        outcome: EngineTickOutcome::Failed,
                        response: format!("Unresolvable dependency: {:?}", a.depends_on),
                        duration_ms: 0,
                    });
                }
                return SwarmFormationResult {
                    objective, outcome: EngineTickOutcome::Failed, waves, reports, duration_ms: now_ms().saturating_sub(started_ms),
                };
            }

            waves += 1;
            let mut wave_futs = Vec::with_capacity(ready_ids.len());
            for id in ready_ids {
                let assign = pending.remove(&id).unwrap();
                let dep_context: Vec<String> = assign
                    .depends_on
                    .iter()
                    .filter_map(|d| completed.get(d).map(|r| format!("- [{} ({})]: {}", r.id, r.role, r.response)))
                    .collect();

                let mut child_cfg = self.config.lock().unwrap().clone();
                child_cfg.system_prompt = format!(
                    "{}\n[Swarm Specialist Role: {}]\nParent Objective: {}",
                    child_cfg.system_prompt, assign.role, objective
                );

                let child_engine = AgentEngine::with_shared_substrate(
                    self.workspace_root.clone(),
                    child_cfg,
                    Arc::clone(&self.provider),
                    Arc::clone(&self.memory),
                    Arc::clone(&self.tools),
                );

                let prompt = if dep_context.is_empty() {
                    assign.objective.clone()
                } else {
                    format!("{}\n\n[Upstream Specialist Results]\n{}", assign.objective, dep_context.join("\n"))
                };

                let tok = cancel_token.clone();
                let def = should_defer.clone();
                wave_futs.push(async move {
                    let item_start = now_ms();
                    let mut input = EngineTickInput::new(prompt);
                    input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
                    input.cancel_token = tok;
                    input.should_defer = def;
                    let res = child_engine.tick(input).await;
                    SwarmItemReport {
                        id: assign.id,
                        role: assign.role,
                        outcome: res.outcome,
                        response: res.response,
                        duration_ms: now_ms().saturating_sub(item_start),
                    }
                });
            }

            let wave_results = futures::future::join_all(wave_futs).await;
            let mut any_failed = false;
            for rep in wave_results {
                if rep.outcome != EngineTickOutcome::Completed {
                    any_failed = true;
                }
                completed.insert(rep.id.clone(), rep.clone());
                reports.push(rep);
            }
            if any_failed {
                let final_outcome = reports
                    .iter()
                    .find(|r| r.outcome != EngineTickOutcome::Completed)
                    .map(|r| r.outcome)
                    .unwrap_or(EngineTickOutcome::Failed);
                return SwarmFormationResult {
                    objective, outcome: final_outcome, waves, reports, duration_ms: now_ms().saturating_sub(started_ms),
                };
            }
        }

        SwarmFormationResult {
            objective,
            outcome: EngineTickOutcome::Completed,
            waves,
            reports,
            duration_ms: now_ms().saturating_sub(started_ms),
        }
    }

    fn emit_progress(
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

    fn handle_slash_command(&self, raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if !trimmed.starts_with('/') { return None; }
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let cmd = parts.next().unwrap_or("").to_ascii_lowercase();
        let arg = parts.next().unwrap_or("").trim();

        let reply = match cmd.as_str() {
            "/help" => [
                "Available slash commands:",
                "  /stats              - Session, arena, cache, and token statistics",
                "  /vfs                - Show staged Virtual File System changes",
                "  /diff               - Render unified diff of staged VFS or workspace changes",
                "  /commit             - Commit staged VFS mutations to disk",
                "  /discard            - Discard staged VFS mutations",
                "  /memory [query]     - List or search persistent LUMI-MEMORY/1 facts",
                "  /skills             - List discovered workspace SKILL.md capabilities",
                "  /compact            - Force rolling LUMI-CONTEXT/1 compaction",
                "  /model [name]       - Inspect or switch the active model",
                "  /snapshot           - Capture a holistic session + VFS + arena snapshot",
                "  /snapshots          - List captured snapshots",
                "  /rewind [index|n]   - Rewind to snapshot or drop last N messages",
                "  /undo               - Roll back the most recent file mutation",
                "  /clear              - Clear conversation messages and reset arena",
                "  /health             - Runtime health, workspace revision, and queue diagnostics",
            ].join("\n"),
            "/stats" => {
                let sess = self.session.lock().unwrap();
                let mem = self.memory.lock().unwrap();
                let vfs = self.vfs.lock().unwrap();
                let slab = sess.slab_snapshot();
                let cache = self.tools.cache.lock().unwrap();
                let journal = self.tools.journal.lock().unwrap();
                json!({
                    "messages": sess.messages().len(),
                    "transcript": sess.transcript().len(),
                    "tokens": estimate_messages_tokens(sess.messages()),
                    "arenaAllocatedBytes": slab.allocated_bytes,
                    "arenaCapacityBytes": slab.capacity_bytes,
                    "arenaResets": slab.reset_count,
                    "memories": mem.list_memories().len(),
                    "vfsStaged": vfs.export_staged().len(),
                    "cacheHits": cache.hits,
                    "cacheMisses": cache.misses,
                    "journalMutations": journal.history().len(),
                }).to_string()
            }
            "/vfs" => {
                let vfs = self.vfs.lock().unwrap();
                let staged = vfs.export_staged();
                json!({ "count": staged.len(), "staged": staged }).to_string()
            }
            "/diff" => {
                let vfs = self.vfs.lock().unwrap();
                let staged = vfs.export_staged();
                if !staged.is_empty() {
                    staged.iter().filter_map(|ov| vfs.generate_diff(&ov.path)).collect::<Vec<_>>().join("\n")
                } else {
                    let j_diff = self.tools.journal.lock().unwrap().generate_session_diff(&self.workspace_root, None);
                    serde_json::to_string_pretty(&j_diff).unwrap_or_else(|_| "No staged VFS or session changes.".to_string())
                }
            }
            "/commit" => match self.vfs.lock().unwrap().commit_all() {
                Ok(paths) => format!("Committed {} staged file(s): {:?}", paths.len(), paths),
                Err(e) => format!("VFS commit failed: {e}"),
            },
            "/discard" => {
                self.vfs.lock().unwrap().clear();
                "Discarded all staged VFS changes.".to_string()
            }
            "/memory" => {
                let mem = self.memory.lock().unwrap();
                if arg.is_empty() {
                    mem.format_memory_envelope().unwrap_or_else(|| "Memory store is empty.".to_string())
                } else {
                    serde_json::to_string_pretty(&mem.search_memories(arg)).unwrap_or_default()
                }
            }
            "/skills" => {
                let skills = SkillsIngestor::discover(&self.workspace_root);
                serde_json::to_string_pretty(&skills).unwrap_or_default()
            }
            "/compact" => {
                let mut sess = self.session.lock().unwrap();
                let policy = CompactionPolicy { force: true, preserve_recent_turns: 2, ..Default::default() };
                serde_json::to_string(&sess.compact(&self.compactor, &policy)).unwrap_or_default()
            }
            "/model" => {
                let mut cfg = self.config.lock().unwrap();
                if arg.is_empty() {
                    format!("Active model: {} (provider: {}, fallback: {:?})", cfg.model_name, cfg.provider, cfg.fallback_model)
                } else {
                    cfg.model_name = arg.to_string();
                    format!("Switched active model to: {}", cfg.model_name)
                }
            }
            "/snapshot" => {
                let snap = self.create_snapshot();
                format!("Created snapshot #{} ({}) at frame {}", self.snapshots.lock().unwrap().len() - 1, snap.snapshot_id, snap.frame_index)
            }
            "/snapshots" => {
                let snaps = self.snapshots.lock().unwrap();
                if snaps.is_empty() {
                    "No snapshots recorded.".to_string()
                } else {
                    snaps.iter().enumerate().map(|(i, s)| format!("#{i}: id={} frame={} msgs={} arena={}B", s.snapshot_id, s.frame_index, s.messages.len(), s.slab_snapshot.allocated_bytes)).collect::<Vec<_>>().join("\n")
                }
            }
            "/rewind" => {
                let n = arg.parse::<usize>().unwrap_or(0);
                let has_snap = { let snaps = self.snapshots.lock().unwrap(); !snaps.is_empty() && n < snaps.len() };
                if has_snap {
                    match self.rewind_to_snapshot(n) {
                        Ok(s) => format!("Rewound to snapshot #{n} ({}).", s.snapshot_id),
                        Err(e) => e,
                    }
                } else {
                    let count = n.max(2);
                    self.session.lock().unwrap().truncate_active_messages(count);
                    format!("Truncated last {count} message(s) from active session.")
                }
            }
            "/undo" => {
                let res = self.tools.journal.lock().unwrap().rollback_last();
                self.tools.cache.lock().unwrap().clear();
                serde_json::to_string(&res).unwrap_or_default()
            }
            "/clear" => {
                let cfg = self.config.lock().unwrap().clone();
                let mut sess = self.session.lock().unwrap();
                sess.clear();
                sess.add_message(SessionMessage::new(Role::System, cfg.system_prompt));
                self.tools.end_task();
                "Session cleared and arena reset to offset 0.".to_string()
            }
            "/health" => {
                let q = self.queue.snapshot();
                json!({
                    "status": "ok",
                    "provider": self.provider.provider_name(),
                    "workspaceRevision": workspace_revision(&self.workspace_root),
                    "queueActive": q.active,
                    "queuePending": q.pending_count,
                    "queueBytes": q.pending_bytes,
                }).to_string()
            }
            _ => return None,
        };
        Some(reply)
    }

    pub async fn ask_side_question(&self, question: &str) -> Result<String, ProviderError> {
        let cfg = self.config.lock().unwrap().clone();
        let req = ProviderRequest {
            model: cfg.model_name,
            messages: vec![
                SessionMessage::new(Role::System, cfg.system_prompt),
                SessionMessage::new(Role::User, question),
            ],
            tools: Vec::new(),
            max_tokens: cfg.max_output_tokens.min(2048),
            temperature: cfg.temperature,
        };
        let resp = self.provider.complete(req, None).await?;
        Ok(resp.content.unwrap_or_default())
    }

    pub async fn tick(&self, input: EngineTickInput) -> EngineTickResult {
        let started_ms = now_ms();
        let frame_index = self.frame_counter.fetch_add(1, Ordering::Relaxed) + 1;
        let turn_activity_id = format!("turn-{frame_index}");
        let cfg = self.config.lock().unwrap().clone();
        let authority = input.execution_authority.unwrap_or(ExecutionAuthorityLevel::Autonomous);

        // 1. Admission check
        let _permit = match self.queue.try_acquire(input.prompt.len()) {
            Ok(p) => p,
            Err(err_msg) => {
                self.emit_progress(&input.on_progress, &turn_activity_id, EngineProgressPhase::Failed, EngineProgressStatus::Failed, &err_msg, None, started_ms, None);
                return EngineTickResult {
                    frame_index, outcome: EngineTickOutcome::Failed, active_model: cfg.model_name, is_fallback_model: false,
                    is_slash_command: false, composed_prompt: String::new(), response: err_msg, tool_results: Vec::new(),
                    safe_to_retry: Some(true), duration_ms: now_ms().saturating_sub(started_ms),
                };
            }
        };

        // 2. Deferral check before modifying session state
        if input.should_defer.as_ref().map(|f| f()).unwrap_or(false) {
            let msg = "Turn deferred due to higher-priority host activity.".to_string();
            self.emit_progress(&input.on_progress, &turn_activity_id, EngineProgressPhase::Deferred, EngineProgressStatus::Deferred, &msg, None, started_ms, None);
            return EngineTickResult {
                frame_index, outcome: EngineTickOutcome::Deferred, active_model: cfg.model_name, is_fallback_model: false,
                is_slash_command: false, composed_prompt: String::new(), response: msg, tool_results: Vec::new(),
                safe_to_retry: Some(true), duration_ms: now_ms().saturating_sub(started_ms),
            };
        }

        // 3. Slash command fast-path
        if let Some(reply) = self.handle_slash_command(&input.prompt) {
            let active_model = self.config.lock().unwrap().model_name.clone();
            self.emit_progress(&input.on_progress, &turn_activity_id, EngineProgressPhase::Completed, EngineProgressStatus::Completed, "Slash command executed", None, started_ms, None);
            return EngineTickResult {
                frame_index, outcome: EngineTickOutcome::Completed, active_model, is_fallback_model: false,
                is_slash_command: true, composed_prompt: input.prompt, response: reply, tool_results: Vec::new(),
                safe_to_retry: Some(true), duration_ms: now_ms().saturating_sub(started_ms),
            };
        }

        // 4. "remember:" fast-path
        let trimmed_prompt = input.prompt.trim();
        if let Some(fact) = trimmed_prompt.strip_prefix("remember:").or_else(|| trimmed_prompt.strip_prefix("Remember:")).map(str::trim).filter(|s| !s.is_empty()) {
            let entry = self.memory.lock().unwrap().save_memory(format!("fact_{}", now_ms()), fact, "fact");
            let reply = format!("Saved to LUMI-MEMORY/1 ({}): {}", entry.key, entry.value);
            return EngineTickResult {
                frame_index, outcome: EngineTickOutcome::Completed, active_model: cfg.model_name, is_fallback_model: false,
                is_slash_command: false, composed_prompt: input.prompt, response: reply, tool_results: Vec::new(),
                safe_to_retry: Some(true), duration_ms: now_ms().saturating_sub(started_ms),
            };
        }

        // 5. Append user message to session and begin turn
        let turn_num = {
            let mut sess = self.session.lock().unwrap();
            let turn = sess.increment_turn();
            sess.add_message(SessionMessage::new(Role::User, input.prompt.clone()));
            if sess.messages().len() > cfg.max_turns {
                sess.compact(&self.compactor, &CompactionPolicy { max_messages: Some(cfg.max_turns), preserve_recent_turns: 4, ..Default::default() });
            }
            turn
        };
        self.tools.begin_turn(turn_num);

        let openai_tools = self.tools.openai_tool_schemas();
        let tool_defs_tokens = estimate_text_tokens(&Value::Array(openai_tools.clone()).to_string());
        let max_rounds = cfg.max_tool_rounds.unwrap_or(32).max(1);

        let mut active_model = cfg.model_name.clone();
        let mut is_fallback_model = false;
        let mut all_tool_results: Vec<ToolExecutionRecord> = Vec::new();
        let mut tools_admitted = false;
        let mut last_composed_summary = String::new();
        let mut final_response = String::new();

        self.emit_progress(
            &input.on_progress, &turn_activity_id, EngineProgressPhase::Planning, EngineProgressStatus::Started,
            "Preparing context and starting agent turn", None, started_ms,
            Some(EngineProgressMetadata {
                source: Some("engine".to_string()), scope: Some("turn".to_string()), attempt: Some(1),
                item_type: None, completed_steps: Some(0), total_steps: Some(max_rounds),
            }),
        );

        for round in 0..max_rounds {
            if input.cancel_token.as_ref().map(|t| t.is_cancelled()).unwrap_or(false) {
                let msg = "Turn cancelled by user.".to_string();
                self.emit_progress(&input.on_progress, &turn_activity_id, EngineProgressPhase::Cancelled, EngineProgressStatus::Cancelled, &msg, None, started_ms, None);
                return EngineTickResult {
                    frame_index, outcome: EngineTickOutcome::Cancelled, active_model, is_fallback_model,
                    is_slash_command: false, composed_prompt: last_composed_summary, response: msg, tool_results: all_tool_results,
                    safe_to_retry: Some(!tools_admitted), duration_ms: now_ms().saturating_sub(started_ms),
                };
            }

            if round > 0 && input.should_defer.as_ref().map(|f| f()).unwrap_or(false) {
                let msg = "Turn deferred between tool rounds.".to_string();
                self.emit_progress(&input.on_progress, &turn_activity_id, EngineProgressPhase::Deferred, EngineProgressStatus::Deferred, &msg, None, started_ms, None);
                return EngineTickResult {
                    frame_index, outcome: EngineTickOutcome::Deferred, active_model, is_fallback_model,
                    is_slash_command: false, composed_prompt: last_composed_summary, response: msg, tool_results: all_tool_results,
                    safe_to_retry: Some(!tools_admitted), duration_ms: now_ms().saturating_sub(started_ms),
                };
            }

            let (messages_for_provider, composed_summary) = {
                let sess = self.session.lock().unwrap();
                let mem = self.memory.lock().unwrap();
                PromptComposer::compose(&cfg, &self.workspace_root, &sess, &mem, tool_defs_tokens)
            };
            last_composed_summary = composed_summary;

            let max_attempts: u32 = if tools_admitted { 1 } else { 2 };
            let mut provider_resp = None;
            let mut last_err = None;

            for attempt in 1..=max_attempts {
                let conn_id = format!("provider-r{round}-a{attempt}");
                if attempt == 2 {
                    if let Some(ref fb) = cfg.fallback_model.as_ref().filter(|m| !m.is_empty() && **m != active_model) {
                        active_model = (*fb).clone();
                        is_fallback_model = true;
                    }
                }
                self.emit_progress(
                    &input.on_progress, &conn_id, EngineProgressPhase::Connecting, EngineProgressStatus::Started,
                    format!("Requesting completion from {active_model} (attempt {attempt})"), None, started_ms,
                    Some(EngineProgressMetadata {
                        source: Some("provider".to_string()), scope: Some("turn".to_string()), attempt: Some(attempt),
                        item_type: None, completed_steps: Some(round), total_steps: Some(max_rounds),
                    }),
                );

                let req = ProviderRequest {
                    model: active_model.clone(),
                    messages: messages_for_provider.clone(),
                    tools: openai_tools.clone(),
                    max_tokens: cfg.max_output_tokens,
                    temperature: cfg.temperature,
                };

                match self.provider.complete(req, input.cancel_token.clone()).await {
                    Ok(resp) => {
                        self.emit_progress(&input.on_progress, &conn_id, EngineProgressPhase::Thinking, EngineProgressStatus::InProgress, "Received model response", None, started_ms, None);
                        provider_resp = Some(resp);
                        break;
                    }
                    Err(err) => {
                        if err.is_cancelled() || input.cancel_token.as_ref().map(|t| t.is_cancelled()).unwrap_or(false) {
                            let msg = "Turn cancelled by user.".to_string();
                            self.emit_progress(&input.on_progress, &turn_activity_id, EngineProgressPhase::Cancelled, EngineProgressStatus::Cancelled, &msg, None, started_ms, None);
                            return EngineTickResult {
                                frame_index, outcome: EngineTickOutcome::Cancelled, active_model, is_fallback_model,
                                is_slash_command: false, composed_prompt: last_composed_summary, response: msg, tool_results: all_tool_results,
                                safe_to_retry: Some(!tools_admitted), duration_ms: now_ms().saturating_sub(started_ms),
                            };
                        }
                        let can_retry = !tools_admitted && err.is_retryable() && attempt < max_attempts;
                        self.emit_progress(
                            &input.on_progress, &conn_id, EngineProgressPhase::Connecting, EngineProgressStatus::Failed,
                            format!("Provider attempt {attempt} failed: {err}"), None, started_ms,
                            Some(EngineProgressMetadata {
                                source: Some("provider".to_string()), scope: Some("turn".to_string()), attempt: Some(attempt),
                                item_type: None, completed_steps: Some(round), total_steps: Some(max_rounds),
                            }),
                        );
                        last_err = Some(err);
                        if !can_retry { break; }
                    }
                }
            }

            let Some(resp) = provider_resp else {
                let err_str = last_err.map(|e| e.to_string()).unwrap_or_else(|| "Unknown provider error".to_string());
                self.emit_progress(&input.on_progress, &turn_activity_id, EngineProgressPhase::Failed, EngineProgressStatus::Failed, &err_str, None, started_ms, None);
                return EngineTickResult {
                    frame_index, outcome: EngineTickOutcome::Failed, active_model, is_fallback_model,
                    is_slash_command: false, composed_prompt: last_composed_summary, response: err_str, tool_results: all_tool_results,
                    safe_to_retry: Some(!tools_admitted), duration_ms: now_ms().saturating_sub(started_ms),
                };
            };

            if resp.tool_calls.is_empty() {
                final_response = resp.content.unwrap_or_default();
                let mut sess = self.session.lock().unwrap();
                sess.add_message(SessionMessage::new(Role::Assistant, final_response.clone()));
                if sess.messages().len() > cfg.max_turns {
                    sess.compact(&self.compactor, &CompactionPolicy { max_messages: Some(cfg.max_turns), preserve_recent_turns: 4, ..Default::default() });
                }
                break;
            }

            tools_admitted = true;
            {
                let mut assistant_msg = SessionMessage::new(Role::Assistant, resp.content.clone().unwrap_or_default());
                assistant_msg.tool_calls = Some(resp.tool_calls.clone());
                self.session.lock().unwrap().add_message(assistant_msg);
            }

            let scheduled_calls: Vec<ScheduledToolCall> = resp.tool_calls.iter().map(|tc| {
                let (parsed_map, _) = self.tools.parser.parse_raw_arguments(&tc.function.arguments);
                ScheduledToolCall { id: tc.id.clone(), name: tc.function.name.clone(), args: Value::Object(parsed_map) }
            }).collect();

            let batch_id = format!("tools-r{round}");
            self.emit_progress(
                &input.on_progress, &batch_id, EngineProgressPhase::Tool, EngineProgressStatus::Started,
                format!("Executing {} tool call(s)", scheduled_calls.len()), None, started_ms,
                Some(EngineProgressMetadata {
                    source: Some("scheduler".to_string()), scope: Some("step".to_string()), attempt: Some(1),
                    item_type: Some("tool".to_string()), completed_steps: Some(0), total_steps: Some(scheduled_calls.len()),
                }),
            );

            let records = self.tools.execute_batch(&scheduled_calls, &self.workspace_root, authority, input.cancel_token.as_ref()).await;

            {
                let mut sess = self.session.lock().unwrap();
                let mut gov = self.tools.governor.lock().unwrap();
                let mut seen_skill_ids = BTreeSet::new();
                for (tc, mut rec) in resp.tool_calls.iter().zip(records.into_iter()) {
                    // Exempt `use_skill` from lossy governor truncation so full SKILL.md reaches model intact
                    let is_skill_call = rec.name == "use_skill" || crate::parser::canonical_tool_name(&rec.name) == "use_skill";
                    let (out_text, spill_id) = if is_skill_call {
                        let raw = match &rec.output {
                            Value::String(s) => s.clone(),
                            other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
                        };
                        seen_skill_ids.insert(tc.id.clone());
                        (DeterministicSecretRedactor::redact(&raw), None)
                    } else {
                        let governed = gov.govern(&rec.output);
                        if governed.was_truncated {
                            rec.output_limited = Some(true);
                        }
                        (governed.output_text, governed.spill_id)
                    };

                    let wire_payload = if rec.success {
                        json!({ "success": true, "output": out_text, "spillId": spill_id })
                    } else {
                        json!({
                            "success": false,
                            "error": rec.error.clone().unwrap_or_else(|| "Tool execution failed".to_string()),
                            "output": out_text,
                        })
                    };

                    sess.add_message(SessionMessage::tool_result(tc.id.clone(), rec.name.clone(), wire_payload.to_string()));
                    all_tool_results.push(rec);
                }
            }

            self.emit_progress(
                &input.on_progress, &batch_id, EngineProgressPhase::Tool, EngineProgressStatus::Completed,
                format!("Completed {} tool call(s)", resp.tool_calls.len()), None, started_ms,
                Some(EngineProgressMetadata {
                    source: Some("scheduler".to_string()), scope: Some("step".to_string()), attempt: Some(1),
                    item_type: Some("tool".to_string()), completed_steps: Some(resp.tool_calls.len()), total_steps: Some(resp.tool_calls.len()),
                }),
            );
        }

        self.emit_progress(&input.on_progress, &turn_activity_id, EngineProgressPhase::Completed, EngineProgressStatus::Completed, "Turn completed", None, started_ms, None);

        EngineTickResult {
            frame_index, outcome: EngineTickOutcome::Completed, active_model, is_fallback_model,
            is_slash_command: false, composed_prompt: last_composed_summary, response: final_response,
            tool_results: all_tool_results, safe_to_retry: Some(!tools_admitted), duration_ms: now_ms().saturating_sub(started_ms),
        }
    }
}
