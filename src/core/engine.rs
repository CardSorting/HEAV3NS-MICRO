// [LAYER: CORE]
use crate::core::request_queue::{PromptComposer, RuntimeRequestQueue};
use crate::core::session_store::{SessionCompactor, SessionStore};
use crate::core::tool_registry::ToolRegistry;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::interfaces::{ILlmProvider, ProgressCallback};
use crate::domain::memory::{GameStateSnapshot, MemoryStore};
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::progress::{EngineProgressEvent, EngineProgressMetadata, EngineProgressPhase, EngineProgressStatus, ToolExecutionRecord};
use crate::domain::types::runtime::{AgentConfig, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel};
use crate::domain::types::tick_result::{EngineTickResult, ProviderError, ProviderRequest};
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::infrastructure::http_provider::HttpProvider;
use crate::infrastructure::vfs_store::SessionVfs;
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::secret_redactor::DeterministicSecretRedactor;
use crate::plumbing::text_tokens::{canonical_tool_name, estimate_text_tokens};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Computes the content-addressed deterministic revision of a workspace tree.
pub fn compute_workspace_revision(root: &Path) -> String {
    crate::infrastructure::vfs_store::workspace_revision(root)
}

/// Core Agent Engine orchestrating turns, tool waves, topological swarms, and deterministic state snapshots.
pub struct AgentEngine {
    workspace_root: PathBuf,
    config: Mutex<AgentConfig>,
    session: Mutex<SessionStore>,
    vfs: Mutex<SessionVfs>,
    memory: Arc<Mutex<MemoryStore>>,
    compactor: SessionCompactor,
    tools: Arc<ToolRegistry>,
    provider: Arc<dyn ILlmProvider>,
    queue: Arc<RuntimeRequestQueue>,
    snapshots: Mutex<Vec<GameStateSnapshot>>,
    frame_counter: AtomicU64,
    event_seq: AtomicU64,
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

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn tools(&self) -> &Arc<ToolRegistry> {
        &self.tools
    }

    pub fn session(&self) -> &Mutex<SessionStore> {
        &self.session
    }

    pub fn vfs(&self) -> &Mutex<SessionVfs> {
        &self.vfs
    }

    pub fn memory(&self) -> &Arc<Mutex<MemoryStore>> {
        &self.memory
    }

    pub fn queue(&self) -> &Arc<RuntimeRequestQueue> {
        &self.queue
    }

    pub(crate) fn provider(&self) -> &Arc<dyn ILlmProvider> {
        &self.provider
    }

    pub fn compactor(&self) -> &SessionCompactor {
        &self.compactor
    }

    pub fn config_snapshot(&self) -> AgentConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn inspect_or_switch_model(&self, arg: &str) -> String {
        let mut cfg = self.config.lock().unwrap();
        if arg.is_empty() {
            format!(
                "Active model: {} (provider: {}, fallback: {:?})",
                cfg.model_name, cfg.provider, cfg.fallback_model
            )
        } else {
            cfg.model_name = arg.to_string();
            format!("Switched active model to: {}", cfg.model_name)
        }
    }

    pub fn snapshots_list(&self) -> Vec<GameStateSnapshot> {
        self.snapshots.lock().unwrap().clone()
    }

    pub(crate) fn snapshot_count(&self) -> usize {
        self.snapshots.lock().unwrap().len()
    }

    pub(crate) fn format_snapshots_list(&self) -> String {
        let snaps = self.snapshots.lock().unwrap();
        if snaps.is_empty() {
            "No snapshots recorded.".to_string()
        } else {
            snaps
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    format!(
                        "#{i}: id={} frame={} msgs={} arena={}B",
                        s.snapshot_id,
                        s.frame_index,
                        s.messages.len(),
                        s.slab_snapshot.allocated_bytes
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

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
            snaps
                .get(index)
                .cloned()
                .ok_or_else(|| format!("Snapshot #{index} not found (total: {})", snaps.len()))?
        };
        let mut session = self.session.lock().unwrap();
        let mut vfs = self.vfs.lock().unwrap();
        let mut memory = self.memory.lock().unwrap();
        session.rewind_to_snapshot(&snap, &mut vfs, &mut memory);
        Ok(snap)
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

        let _permit = match self.queue.try_acquire(input.prompt.len()) {
            Ok(p) => p,
            Err(err_msg) => {
                self.emit_progress(
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
                };
            }
        };

        if input.should_defer.as_ref().map(|f| f()).unwrap_or(false) {
            let msg = "Turn deferred due to higher-priority host activity.".to_string();
            self.emit_progress(
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
            };
        }

        if let Some(reply) = self.handle_slash_command(&input.prompt) {
            let active_model = self.config.lock().unwrap().model_name.clone();
            self.emit_progress(
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
                .unwrap()
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
                self.emit_progress(
                    &input.on_progress,
                    &turn_activity_id,
                    EngineProgressPhase::Cancelled,
                    EngineProgressStatus::Cancelled,
                    &msg,
                    None,
                    started_ms,
                    None,
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
                    duration_ms: now_ms().saturating_sub(started_ms),
                };
            }

            if round > 0 && input.should_defer.as_ref().map(|f| f()).unwrap_or(false) {
                let msg = "Turn deferred between tool rounds.".to_string();
                self.emit_progress(
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
                    active_model,
                    is_fallback_model,
                    is_slash_command: false,
                    composed_prompt: last_composed_summary,
                    response: msg,
                    tool_results: all_tool_results,
                    safe_to_retry: Some(!tools_admitted),
                    duration_ms: now_ms().saturating_sub(started_ms),
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
                    if let Some(ref fb) = cfg
                        .fallback_model
                        .as_ref()
                        .filter(|m| !m.is_empty() && **m != active_model)
                    {
                        active_model = (*fb).clone();
                        is_fallback_model = true;
                    }
                }
                self.emit_progress(
                    &input.on_progress,
                    &conn_id,
                    EngineProgressPhase::Connecting,
                    EngineProgressStatus::Started,
                    format!("Requesting completion from {active_model} (attempt {attempt})"),
                    None,
                    started_ms,
                    Some(EngineProgressMetadata {
                        source: Some("provider".to_string()),
                        scope: Some("turn".to_string()),
                        attempt: Some(attempt),
                        item_type: None,
                        completed_steps: Some(round),
                        total_steps: Some(max_rounds),
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
                        self.emit_progress(
                            &input.on_progress,
                            &conn_id,
                            EngineProgressPhase::Thinking,
                            EngineProgressStatus::InProgress,
                            "Received model response",
                            None,
                            started_ms,
                            None,
                        );
                        provider_resp = Some(resp);
                        break;
                    }
                    Err(err) => {
                        if err.is_cancelled()
                            || input.cancel_token.as_ref().map(|t| t.is_cancelled()).unwrap_or(false)
                        {
                            let msg = "Turn cancelled by user.".to_string();
                            self.emit_progress(
                                &input.on_progress,
                                &turn_activity_id,
                                EngineProgressPhase::Cancelled,
                                EngineProgressStatus::Cancelled,
                                &msg,
                                None,
                                started_ms,
                                None,
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
                                duration_ms: now_ms().saturating_sub(started_ms),
                            };
                        }
                        let can_retry = !tools_admitted && err.is_retryable() && attempt < max_attempts;
                        self.emit_progress(
                            &input.on_progress,
                            &conn_id,
                            EngineProgressPhase::Connecting,
                            EngineProgressStatus::Failed,
                            format!("Provider attempt {attempt} failed: {err}"),
                            None,
                            started_ms,
                            Some(EngineProgressMetadata {
                                source: Some("provider".to_string()),
                                scope: Some("turn".to_string()),
                                attempt: Some(attempt),
                                item_type: None,
                                completed_steps: Some(round),
                                total_steps: Some(max_rounds),
                            }),
                        );
                        last_err = Some(err);
                        if !can_retry {
                            break;
                        }
                    }
                }
            }

            let Some(resp) = provider_resp else {
                let err_str = last_err
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Unknown provider error".to_string());
                self.emit_progress(
                    &input.on_progress,
                    &turn_activity_id,
                    EngineProgressPhase::Failed,
                    EngineProgressStatus::Failed,
                    &err_str,
                    None,
                    started_ms,
                    None,
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
                    duration_ms: now_ms().saturating_sub(started_ms),
                };
            };

            if resp.tool_calls.is_empty() {
                final_response = resp.content.unwrap_or_default();
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
            self.emit_progress(
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
                .tools
                .execute_batch(
                    &scheduled_calls,
                    &self.workspace_root,
                    authority,
                    input.cancel_token.as_ref(),
                )
                .await;

            {
                let mut sess = self.session.lock().unwrap();
                let mut gov = self.tools.governor.lock().unwrap();
                let mut seen_skill_ids = BTreeSet::new();
                for (tc, mut rec) in resp.tool_calls.iter().zip(records.into_iter()) {
                    let is_skill_call = rec.name == "use_skill" || canonical_tool_name(&rec.name) == "use_skill";
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
                .map(|r| {
                    let status = if r.success { "ok" } else { "err" };
                    format!("{} ({}, {}ms)", r.name, status, r.duration_ms)
                })
                .collect();

            self.emit_progress(
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

        self.emit_progress(
            &input.on_progress,
            &turn_activity_id,
            EngineProgressPhase::Completed,
            EngineProgressStatus::Completed,
            "Turn completed",
            None,
            started_ms,
            None,
        );

        if final_response.trim().is_empty() {
            final_response = if !all_tool_results.is_empty() {
                let success_count = all_tool_results.iter().filter(|r| r.success).count();
                let tool_names: Vec<String> = all_tool_results.iter().map(|r| r.name.clone()).collect();
                format!(
                    "Completed {} tool action(s) successfully: {}.",
                    success_count,
                    tool_names.join(", ")
                )
            } else {
                "Turn completed with no text response from model.".to_string()
            };
        }

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
            duration_ms: now_ms().saturating_sub(started_ms),
        }
    }
}
