// [LAYER: CORE]
use crate::core::engine::AgentEngine;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::swarm::{SwarmAssignment, SwarmFormationResult, SwarmItemReport};
use crate::domain::types::messages::{estimate_messages_tokens, Role, SessionMessage};
use crate::domain::types::runtime::{CancellationToken, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel};
use crate::infrastructure::vfs_store::{workspace_revision, SkillsIngestor};
use crate::plumbing::hash_and_math::now_ms;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

pub type SwarmSlashOrchestrator = AgentEngine;

impl AgentEngine {
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
        let mut pending: BTreeMap<String, SwarmAssignment> =
            assignments.into_iter().map(|a| (a.id.clone(), a)).collect();
        let mut completed: BTreeMap<String, SwarmItemReport> = BTreeMap::new();
        let mut reports = Vec::new();
        let mut waves = 0usize;

        while !pending.is_empty() {
            if cancel_token.as_ref().map(|t| t.is_cancelled()).unwrap_or(false) {
                return SwarmFormationResult {
                    objective,
                    outcome: EngineTickOutcome::Cancelled,
                    waves,
                    reports,
                    duration_ms: now_ms().saturating_sub(started_ms),
                };
            }
            if should_defer.as_ref().map(|f| f()).unwrap_or(false) {
                return SwarmFormationResult {
                    objective,
                    outcome: EngineTickOutcome::Deferred,
                    waves,
                    reports,
                    duration_ms: now_ms().saturating_sub(started_ms),
                };
            }

            let ready_ids: Vec<String> = pending
                .values()
                .filter(|a| a.depends_on.iter().all(|dep| completed.contains_key(dep)))
                .map(|a| a.id.clone())
                .collect();

            if ready_ids.is_empty() {
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
                    objective,
                    outcome: EngineTickOutcome::Failed,
                    waves,
                    reports,
                    duration_ms: now_ms().saturating_sub(started_ms),
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

                let mut child_cfg = self.config_snapshot();
                child_cfg.system_prompt = format!(
                    "{}\n[Swarm Specialist Role: {}]\nParent Objective: {}",
                    child_cfg.system_prompt, assign.role, objective
                );

                let child_engine = AgentEngine::with_shared_substrate(
                    self.workspace_root().to_path_buf(),
                    child_cfg,
                    Arc::clone(self.provider()),
                    Arc::clone(self.memory()),
                    Arc::clone(self.tools()),
                );

                let prompt = if dep_context.is_empty() {
                    assign.objective.clone()
                } else {
                    format!(
                        "{}\n\n[Upstream Specialist Results]\n{}",
                        assign.objective,
                        dep_context.join("\n")
                    )
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
                    objective,
                    outcome: final_outcome,
                    waves,
                    reports,
                    duration_ms: now_ms().saturating_sub(started_ms),
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

    pub(crate) fn handle_slash_command(&self, raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if !trimmed.starts_with('/') {
            return None;
        }
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
            ]
            .join("\n"),
            "/stats" => {
                let sess = self.session().lock().unwrap();
                let mem = self.memory().lock().unwrap();
                let vfs = self.vfs().lock().unwrap();
                let slab = sess.slab_snapshot();
                let cache = self.tools().cache.lock().unwrap();
                let journal = self.tools().journal.lock().unwrap();
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
                })
                .to_string()
            }
            "/vfs" => {
                let vfs = self.vfs().lock().unwrap();
                let staged = vfs.export_staged();
                json!({ "count": staged.len(), "staged": staged }).to_string()
            }
            "/diff" => {
                let vfs = self.vfs().lock().unwrap();
                let staged = vfs.export_staged();
                if !staged.is_empty() {
                    staged
                        .iter()
                        .filter_map(|ov| vfs.generate_diff(&ov.path))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    let j_diff = self
                        .tools()
                        .journal
                        .lock()
                        .unwrap()
                        .generate_session_diff(self.workspace_root(), None);
                    serde_json::to_string_pretty(&j_diff)
                        .unwrap_or_else(|_| "No staged VFS or session changes.".to_string())
                }
            }
            "/commit" => match self.vfs().lock().unwrap().commit_all() {
                Ok(paths) => format!("Committed {} staged file(s): {:?}", paths.len(), paths),
                Err(e) => format!("VFS commit failed: {e}"),
            },
            "/discard" => {
                self.vfs().lock().unwrap().clear();
                "Discarded all staged VFS changes.".to_string()
            }
            "/memory" => {
                let mem = self.memory().lock().unwrap();
                if arg.is_empty() {
                    mem.format_memory_envelope()
                        .unwrap_or_else(|| "Memory store is empty.".to_string())
                } else {
                    serde_json::to_string_pretty(&mem.search_memories(arg)).unwrap_or_default()
                }
            }
            "/skills" => {
                let skills = SkillsIngestor::discover(self.workspace_root());
                serde_json::to_string_pretty(&skills).unwrap_or_default()
            }
            "/compact" => {
                let mut sess = self.session().lock().unwrap();
                let policy = CompactionPolicy {
                    force: true,
                    preserve_recent_turns: 2,
                    ..Default::default()
                };
                serde_json::to_string(&sess.compact_session(self.compactor(), &policy)).unwrap_or_default()
            }
            "/model" => self.inspect_or_switch_model(arg),
            "/snapshot" => {
                let snap = self.create_snapshot();
                format!(
                    "Created snapshot #{} ({}) at frame {}",
                    self.snapshot_count() - 1,
                    snap.snapshot_id,
                    snap.frame_index
                )
            }
            "/snapshots" => self.format_snapshots_list(),
            "/rewind" => {
                let n = arg.parse::<usize>().unwrap_or(0);
                if self.snapshot_count() > 0 && n < self.snapshot_count() {
                    match self.rewind_to_snapshot(n) {
                        Ok(s) => format!("Rewound to snapshot #{n} ({}).", s.snapshot_id),
                        Err(e) => e,
                    }
                } else {
                    let count = n.max(2);
                    self.session().lock().unwrap().truncate_active_messages(count);
                    format!("Truncated last {count} message(s) from active session.")
                }
            }
            "/undo" => {
                let res = self.tools().journal.lock().unwrap().rollback_last();
                self.tools().cache.lock().unwrap().clear();
                serde_json::to_string(&res).unwrap_or_default()
            }
            "/clear" => {
                let cfg = self.config_snapshot();
                let mut sess = self.session().lock().unwrap();
                sess.clear();
                sess.add_message(SessionMessage::new(Role::System, cfg.system_prompt));
                self.tools().end_task();
                "Session cleared and arena reset to offset 0.".to_string()
            }
            "/health" => {
                let q = self.queue().snapshot();
                json!({
                    "status": "ok",
                    "provider": self.provider().provider_name(),
                    "workspaceRevision": workspace_revision(self.workspace_root()),
                    "queueActive": q.active,
                    "queuePending": q.pending_count,
                    "queueBytes": q.pending_bytes,
                })
                .to_string()
            }
            _ => return None,
        };
        Some(reply)
    }
}
