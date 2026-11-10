// [LAYER: CORE]
use crate::core::engine::AgentEngine;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::prompt_spec::SystemPromptSpec;
use crate::domain::swarm::{classify_task_for_legion, SwarmAssignment, SwarmFormationResult, SwarmItemReport};
use crate::domain::types::messages::{estimate_messages_tokens, Role, SessionMessage};
use crate::domain::types::progress::ToolExecutionRecord;
use crate::domain::types::runtime::{CancellationToken, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel};
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::infrastructure::vfs_store::{workspace_revision, SkillsIngestor, WorkspacePromptProbe};
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::text_tokens::canonical_tool_name;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type SwarmSlashOrchestrator = AgentEngine;

impl AgentEngine {
    pub(crate) async fn execute_turn_tool_batch(
        &self,
        scheduled_calls: &[ScheduledToolCall],
        authority: ExecutionAuthorityLevel,
        cancel_token: Option<&CancellationToken>,
    ) -> Vec<ToolExecutionRecord> {
        if self.forbids_swarm_delegation()
            && scheduled_calls.iter().any(|c| canonical_tool_name(&c.name) == "engage_swarm")
        {
            let mut records = Vec::with_capacity(scheduled_calls.len());
            for call in scheduled_calls {
                if canonical_tool_name(&call.name) == "engage_swarm" {
                    records.push(ToolExecutionRecord {
                        name: call.name.clone(),
                        call_id: Some(call.id.clone()),
                        args: Some(call.args.clone()),
                        output: Value::Null,
                        success: false,
                        denied: Some(true),
                        output_limited: None,
                        error: Some(
                            "Recursive swarm delegation ('engage_swarm') is forbidden inside a Legion specialist."
                                .to_string(),
                        ),
                        duration_ms: 0,
                    });
                } else {
                    records.push(
                        self.tools()
                            .execute_single(call, self.workspace_root(), authority, cancel_token)
                            .await,
                    );
                }
            }
            records
        } else {
            self.tools()
                .execute_batch(scheduled_calls, self.workspace_root(), authority, cancel_token)
                .await
        }
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
            let swarm_facts = WorkspacePromptProbe::probe_cached(self.workspace_root());
            let swarm_skills = SkillsIngestor::discover(self.workspace_root());
            let mut wave_futs = Vec::with_capacity(ready_ids.len());
            for id in ready_ids {
                let assign = pending.remove(&id).unwrap();
                let dep_context: Vec<String> = assign
                    .depends_on
                    .iter()
                    .filter_map(|d| completed.get(d).map(|r| format!("- [{} ({})]: {}", r.id, r.role, r.response)))
                    .collect();

                let mut child_cfg = self.config_snapshot();
                let spec = SystemPromptSpec::for_specialist(&assign.role, &objective, assign.assigned_files.clone());
                child_cfg.system_prompt = spec.render(&child_cfg, self.workspace_root(), &swarm_facts, &swarm_skills);

                let child_engine = AgentEngine::with_shared_substrate(
                    self.workspace_root().to_path_buf(),
                    child_cfg,
                    Arc::clone(self.provider()),
                    Arc::clone(self.memory()),
                    Arc::clone(self.tools()),
                )
                .without_swarm_delegation();

                let prompt = SystemPromptSpec::format_swarm_task_prompt(&assign.objective, &dep_context);

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
                "  /context [json]     - Visual token budget HUD, KV-cache prefix, and tool routing",
                "  /prompt [mode]      - Inspect active system prompt (prefix|compact|specialist|json)",
                "  /stats              - Session, arena, cache, and token statistics",
                "  /vfs                - Show staged Virtual File System changes",
                "  /diff               - Render unified diff of staged VFS or workspace changes",
                "  /commit             - Commit staged VFS mutations to disk",
                "  /discard            - Discard staged VFS mutations",
                "  /tools              - List core, unlocked, and on-demand built-in tools (29 total)",
                "  /guide              - Display quickstart setup and onboarding guide",
                "  /demo               - Information on running the offline interactive tour",
                "  /memory [query]     - List or search persistent LUMI-MEMORY/1 facts",
                "  /skills             - List discovered workspace SKILL.md & DAG capabilities",
                "  /soul               - Inspect active Soul persona, axioms, and competencies",
                "  /forge <prompt>     - Synthesize a new custom skill into the Skill Tree DAG",
                "  /doctor             - Audit Computer Use SLA health and Skill Tree DAG integrity",
                "  /computer [action]  - Capture or inspect Set-of-Marks Virtual Display state",
                "  /spatial [target]   - Audit spatial grounding health or ground a UI selector",
                "  /browser [url]      - Navigate or snapshot interactive Set-of-Marks DOM tree",
                "  /vision [path]      - Decode binary image header metadata or generate test SVG",
                "  /legion <objective> - Classify task into The Legion 7-specialist swarm DAG",
                "  /compact            - Force rolling LUMI-CONTEXT/1 compaction",
                "  /model [name]       - Inspect or switch the active model and prompt profile",
                "  /snapshot           - Capture a holistic session + VFS + arena snapshot",
                "  /snapshots          - List captured snapshots",
                "  /rewind [index|n]   - Rewind to snapshot or drop last N messages",
                "  /undo               - Roll back the most recent file mutation",
                "  /clear              - Clear conversation messages and reset arena",
                "  /health             - Runtime health, workspace revision, and queue diagnostics",
            ]
            .join("\n"),
            "/context" => self.render_context_hud(arg),
            "/prompt" => self.render_prompt_inspector(arg),
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
            "/tools" => self.render_tools_status(),
            "/guide" | "/quickstart" => [
                "HEAV3NS-MICRO Quickstart Guide:",
                "  1. Configure AI Provider:",
                "     - Set OpenAI key:    export OPENAI_API_KEY=\"sk-...\"",
                "     - Or use Gemini:      export GEMINI_API_KEY=\"...\"",
                "     - Or use free Ollama: heav3ns-micro --provider ollama -m llama3.2",
                "  2. Core Safety & VFS Commands:",
                "     /stats    - View session memory, 16-MiB arena slab, token usage",
                "     /vfs      - Inspect staged sandboxed file modifications",
                "     /diff     - View unified diff before committing changes to disk",
                "     /commit   - Safely write staged VFS files to disk",
                "     /discard  - Discard staged changes safely",
                "     /memory   - Search long-term persistent knowledge store",
                "     /doctor   - Inspect SLA health, Skill Tree DAG, and display status",
                "     /tools    - List all registered built-in tools",
                "     /demo     - Walkthrough the 5 sovereign pillars",
            ]
            .join("\n"),
            "/demo" => [
                "HEAV3NS-MICRO Zero-Key Guided Tour:",
                "  To run the full interactive guided tour in your terminal, run:",
                "    heav3ns-micro demo",
                "  This executes the 5 sovereign pillars (Arena Slab, VFS Sandbox, Diff Review,",
                "  LUMI-MEMORY persistence, and Flight Suite validation) completely offline.",
            ]
            .join("\n"),
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
                let dag = self.tools().skill_catalog.lock().unwrap().evaluate_dag();
                serde_json::to_string_pretty(&json!({ "workspaceSkills": skills, "skillTreeDag": dag })).unwrap_or_default()
            }
            "/soul" => {
                let cat = self.tools().skill_catalog.lock().unwrap();
                serde_json::to_string_pretty(&json!({
                    "activeSoul": cat.active_soul,
                    "competencies": cat.competency_scores
                }))
                .unwrap_or_default()
            }
            "/forge" => {
                let prompt = if arg.is_empty() { "sovereign-custom-operator" } else { arg };
                let mut cat = self.tools().skill_catalog.lock().unwrap();
                match cat.forge_skill(&json!({ "prompt": prompt })) {
                    Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                    Err(e) => format!("Skill forge error: {e}"),
                }
            }
            "/doctor" => {
                let disp_health = self.tools().display_state.lock().unwrap().audit_health();
                let dag = self.tools().skill_catalog.lock().unwrap().evaluate_dag();
                serde_json::to_string_pretty(&json!({
                    "displayHealth": disp_health,
                    "skillTreeAcyclic": dag.get("acyclic"),
                    "skillNodeCount": dag.get("nodeCount")
                }))
                .unwrap_or_default()
            }
            "/computer" => {
                let mut sub_parts = arg.splitn(2, char::is_whitespace);
                let sub_cmd = sub_parts.next().unwrap_or("").trim().to_ascii_lowercase();
                let sub_arg = sub_parts.next().unwrap_or("").trim();
                match sub_cmd.as_str() {
                    "observe" => {
                        let obs = self.tools().computer_orchestrator.observe();
                        serde_json::to_string_pretty(&obs).unwrap_or_default()
                    }
                    "interpret" => {
                        let obs = self.tools().computer_orchestrator.observe();
                        let interp = self.tools().computer_orchestrator.interpret(&obs);
                        serde_json::to_string_pretty(&interp).unwrap_or_default()
                    }
                    "patterns" => {
                        let kstore = self.tools().computer_orchestrator.knowledge_store.lock().unwrap();
                        serde_json::to_string_pretty(&json!({
                            "patterns": kstore.list_patterns(),
                            "procedures": kstore.list_procedures()
                        }))
                        .unwrap_or_default()
                    }
                    "consult" => {
                        use crate::domain::interfaces::ISwarmIntelligenceConsultant;
                        let obs = self.tools().computer_orchestrator.observe();
                        let goal = if sub_arg.is_empty() { "Analyze active window" } else { sub_arg };
                        let mut req = crate::domain::types::swarm_intelligence_types::SwarmConsultationRequest::new(
                            "consult-slash",
                            goal,
                            &obs.app_name,
                            &obs.window_title,
                        );
                        req.modal_blocking = obs.modal_detected;
                        req.candidate_target_ids = obs.elements.iter().map(|e| e.id.clone()).collect();
                        let res = self.tools().computer_orchestrator.consult(&req);
                        serde_json::to_string_pretty(&res).unwrap_or_default()
                    }
                    "orchestrate" => {
                        let target = if sub_arg.is_empty() { None } else { Some(sub_arg.to_string()) };
                        let contract = crate::domain::types::spatial_types::SpatialActionContract {
                            action_name: "click".into(),
                            target_selector: target,
                            ..Default::default()
                        };
                        match self.tools().computer_orchestrator.execute_closed_loop_cycle("Slash command orchestration", &contract) {
                            Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                            Err(e) => format!("Computer orchestration error: {e}"),
                        }
                    }
                    _ => {
                        let mut disp = self.tools().display_state.lock().unwrap();
                        let action = if arg.is_empty() { "capture" } else { arg };
                        match disp.execute_action(&json!({ "action": action })) {
                            Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                            Err(e) => format!("Computer action error: {e}"),
                        }
                    }
                }
            }
            "/browser" => {
                let mut disp = self.tools().display_state.lock().unwrap();
                let req = if arg.is_empty() {
                    json!({ "action": "snapshot" })
                } else {
                    json!({ "action": "navigate", "url": arg })
                };
                match disp.execute_browser_action(&req) {
                    Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                    Err(e) => format!("Browser action error: {e}"),
                }
            }
            "/vision" => {
                if arg.is_empty() {
                    let svg = crate::plumbing::vision_and_dom::generate_synthetic_svg(800, 600, "HEAV3NS-MICRO Vision");
                    let meta = crate::plumbing::vision_and_dom::decode_image_metadata(svg.as_bytes()).unwrap_or_default();
                    serde_json::to_string_pretty(&meta).unwrap_or_default()
                } else {
                    match SkillsIngestor::inspect_workspace_image(self.workspace_root(), arg) {
                        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
                        Err(e) => format!("Vision inspect error: {e}"),
                    }
                }
            }
            "/spatial" => {
                if arg.is_empty() {
                    serde_json::to_string_pretty(&self.tools().spatial_engine.audit_health()).unwrap_or_default()
                } else {
                    let contract = crate::domain::types::spatial_types::SpatialActionContract {
                        action_name: "click".into(),
                        target_selector: Some(arg.to_string()),
                        ..Default::default()
                    };
                    let disp = self.tools().display_state.lock().unwrap();
                    let fresh = crate::core::spatial_engine::SpatialInteractionEngine::extract_scene_from_display(&disp);
                    let mut med = self.tools().spatial_engine.mediator.lock().unwrap();
                    med.update_scene(fresh);
                    let res = med.ground_action(&contract);
                    serde_json::to_string_pretty(&res).unwrap_or_default()
                }
            }
            "/legion" => {
                let obj = if arg.is_empty() { "Audit, harden, and verify workspace architecture" } else { arg };
                serde_json::to_string_pretty(&classify_task_for_legion(obj, &[], None, 5)).unwrap_or_default()
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
