// [LAYER: CORE]
use crate::core::computer_use_orchestrator::ComputerUseOrchestrator;
use crate::core::spatial_engine::SpatialInteractionEngine;
use crate::core::tool_governor::{ToolExecutionCache, ToolLoopBreaker, ToolOutputGovernor};
use crate::core::tool_specs::builtin_tool_specs;
use crate::domain::computer_use::VirtualDisplayState;
use crate::domain::interfaces::{BoxFuture, ISwarmIntelligenceConsultant, IToolExecutor};
use crate::domain::memory::MemoryStore;
use crate::domain::skill_tree::SkillTreeCatalog;
use crate::domain::swarm::classify_task_for_legion;
use crate::domain::tool_parser::ToolCallArgParser;
use crate::domain::types::progress::ToolExecutionRecord;
use crate::domain::types::runtime::{CancellationToken, ExecutionAuthorityLevel};
use crate::domain::types::spatial_types::SpatialActionContract;
use crate::domain::types::swarm_intelligence_types::SwarmConsultationRequest;
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::domain::types::tool_types::ToolSpec;
use crate::infrastructure::behavioral_store::BehavioralKnowledgeStore;
use crate::infrastructure::fs_tools::dispatch_fs_tool;
use crate::infrastructure::search_tools::resolve_workspace_path;
use crate::infrastructure::shell_runner::execute_shell_command;
use crate::infrastructure::transaction_journal::TransactionJournal;
use crate::infrastructure::vfs_store::SkillsIngestor;
use crate::plumbing::secret_redactor::SafetyPolicy;
use crate::plumbing::text_tokens::canonical_tool_name;
use crate::plumbing::vision_and_dom::{decode_image_metadata, generate_synthetic_bmp, generate_synthetic_svg};
use futures::future::join_all;
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Hyper-compressed Tool Registry, Wave Scheduler, Cache, Governor, and Native I/O Orchestrator.
pub struct ToolRegistry {
    specs: HashMap<&'static str, ToolSpec>,
    ordered_names: Vec<&'static str>,
    pub parser: ToolCallArgParser,
    pub journal: Mutex<TransactionJournal>,
    pub cache: Mutex<ToolExecutionCache>,
    pub governor: Mutex<ToolOutputGovernor>,
    pub loop_breaker: Mutex<ToolLoopBreaker>,
    pub memory_store: Arc<Mutex<MemoryStore>>,
    pub display_state: Arc<Mutex<VirtualDisplayState>>,
    pub spatial_engine: Arc<SpatialInteractionEngine>,
    pub behavioral_store: Arc<Mutex<BehavioralKnowledgeStore>>,
    pub computer_orchestrator: Arc<ComputerUseOrchestrator>,
    pub skill_catalog: Mutex<SkillTreeCatalog>,
    pub unlocked_tools: Mutex<BTreeSet<&'static str>>,
    pub max_concurrency: usize,
}

impl ToolRegistry {
    pub extern "Rust" fn new(memory_store: Arc<Mutex<MemoryStore>>) -> Self {
        let display_state = Arc::new(Mutex::new(VirtualDisplayState::new()));
        let spatial_engine = Arc::new(SpatialInteractionEngine::new(Arc::clone(&display_state)));
        let mem_sink = {
            let m = Arc::clone(&memory_store);
            Arc::new(move |k: &str, v: &str, cat: &str| {
                if let Ok(mut lock) = m.lock() {
                    lock.save_memory(k, v, cat);
                }
            })
        };
        let behavioral_store = Arc::new(Mutex::new(BehavioralKnowledgeStore::new(Some(mem_sink))));
        let computer_orchestrator = Arc::new(ComputerUseOrchestrator::new(
            Arc::clone(&display_state),
            Arc::clone(&spatial_engine),
            Arc::clone(&behavioral_store),
        ));
        let mut registry = Self {
            specs: HashMap::new(),
            ordered_names: Vec::new(),
            parser: ToolCallArgParser::new(),
            journal: Mutex::new(TransactionJournal::new()),
            cache: Mutex::new(ToolExecutionCache::default()),
            governor: Mutex::new(ToolOutputGovernor::new()),
            loop_breaker: Mutex::new(ToolLoopBreaker::default()),
            memory_store,
            display_state,
            spatial_engine,
            behavioral_store,
            computer_orchestrator,
            skill_catalog: Mutex::new(SkillTreeCatalog::new()),
            unlocked_tools: Mutex::new(BTreeSet::new()),
            max_concurrency: 8,
        };
        for spec in builtin_tool_specs() {
            registry.register(spec);
        }
        registry
    }

    fn register(&mut self, spec: ToolSpec) {
        self.ordered_names.push(spec.name);
        self.specs.insert(spec.name, spec);
    }

    pub fn get_spec(&self, name: &str) -> Option<&ToolSpec> {
        self.specs.get(canonical_tool_name(name))
    }

    pub fn list_specs(&self) -> Vec<&ToolSpec> {
        self.ordered_names.iter().filter_map(|name| self.specs.get(name)).collect()
    }

    pub fn openai_tool_schemas(&self) -> Vec<Value> {
        self.list_specs().iter().map(|s| s.to_openai_tool()).collect()
    }

    /// Context-aware sticky dynamic tool router: always includes core tools, unlocks domain tools matching prompt intent, and retains session-unlocked tools across turns.
    pub fn select_relevant_tool_schemas(&self, prompt: &str) -> Vec<Value> {
        self.select_tool_schemas_for(prompt, false)
    }

    /// Selects relevant OpenAI tool schemas with optional structural exclusion of `engage_swarm` for child Legion specialists.
    pub fn select_tool_schemas_for(&self, prompt: &str, forbid_swarm: bool) -> Vec<Value> {
        let lower = prompt.trim().to_lowercase();
        if lower.is_empty() {
            return self
                .list_specs()
                .into_iter()
                .filter(|spec| !forbid_swarm || spec.name != "engage_swarm")
                .map(|s| s.to_openai_tool())
                .collect();
        }
        let mut unlocked = self.unlocked_tools.lock().unwrap();
        for spec in self.list_specs() {
            if spec.is_core || (forbid_swarm && spec.name == "engage_swarm") {
                continue;
            }
            let matched = lower.contains(spec.name)
                || (matches!(spec.name, "list_skills" | "use_skill" | "skill_forge")
                    && (lower.contains("skill") || lower.contains("workflow") || lower.contains("soul") || lower.contains("forge") || lower.contains("dag")))
                || (spec.name == "computer_use"
                    && (lower.contains("computer") || lower.contains("click") || lower.contains("window") || lower.contains("display") || lower.contains("gui") || lower.contains("screen") || lower.contains("clipboard")))
                || (spec.name == "browser_use"
                    && (lower.contains("browser") || lower.contains("dom") || lower.contains("url") || lower.contains("http") || lower.contains("web") || lower.contains("navigate") || lower.contains(".html")))
                || (spec.name == "vision_inspect"
                    && (lower.contains("vision") || lower.contains("image") || lower.contains("png") || lower.contains("svg") || lower.contains("jpeg") || lower.contains("jpg") || lower.contains("webp") || lower.contains("bmp")))
                || (spec.name == "engage_swarm"
                    && (lower.contains("swarm") || lower.contains("legion") || lower.contains("specialist") || lower.contains("parallel")))
                || (spec.name == "spatial_interact"
                    && (lower.contains("spatial") || lower.contains("bounding") || lower.contains("ground") || lower.contains("occlu") || lower.contains("geometry") || lower.contains("assurance")))
                || (spec.name == "get_workspace_diff"
                    && (lower.contains("diff") || lower.contains("patch") || lower.contains("git")))
                || (spec.name == "batch_replace_regex"
                    && (lower.contains("symbol") || lower.contains("rename") || lower.contains("refactor") || lower.contains("regex")))
                || (spec.name == "move_file" && (lower.contains("move") || lower.contains("rename")))
                || (spec.name == "directory_tree" && lower.contains("tree"))
                || (spec.name == "file_info" && (lower.contains("metadata") || lower.contains("sha256") || lower.contains("digest")))
                || (spec.name == "edit_file_anchored" && (lower.contains("anchor") || lower.contains("fnv")));
            if matched {
                unlocked.insert(spec.name);
            }
        }
        self.list_specs()
            .into_iter()
            .filter(|spec| (!forbid_swarm || spec.name != "engage_swarm") && (spec.is_core || unlocked.contains(spec.name)))
            .map(|s| s.to_openai_tool())
            .collect()
    }

    pub fn unlock_tool_by_name(&self, name: &str) {
        if let Some(spec) = self.get_spec(name) {
            if !spec.is_core {
                if let Ok(mut unlocked) = self.unlocked_tools.lock() {
                    unlocked.insert(spec.name);
                }
            }
        }
    }

    pub fn begin_turn(&self, turn_count: u64) {
        if let Ok(mut j) = self.journal.lock() {
            j.set_turn_id(format!("turn_{turn_count}"));
        }
        if let Ok(mut lb) = self.loop_breaker.lock() {
            lb.reset();
        }
    }

    pub fn end_task(&self) {
        if let Ok(mut c) = self.cache.lock() {
            c.clear();
        }
        if let Ok(mut g) = self.governor.lock() {
            g.clear();
        }
        if let Ok(mut u) = self.unlocked_tools.lock() {
            u.clear();
        }
    }

    pub fn extract_target_resources(&self, call: &ScheduledToolCall, cwd: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let Some(obj) = call.args.as_object() else { return paths };
        for key in ["path", "filePath", "file_path", "source", "target", "cwd"] {
            if let Some(Value::String(s)) = obj.get(key) {
                if !s.trim().is_empty() && !s.contains('\n') {
                    paths.push(resolve_workspace_path(cwd, s));
                }
            }
        }
        paths
    }

    /// Partitions a batch of tool calls into dependency-safe concurrent execution waves.
    pub fn partition_waves(&self, calls: &[ScheduledToolCall], cwd: &Path) -> Vec<Vec<ScheduledToolCall>> {
        if calls.len() <= 1 {
            return if calls.is_empty() {
                Vec::new()
            } else {
                vec![calls.to_vec()]
            };
        }
        let (mut waves, mut current_wave, mut current_resources, mut current_has_mutations) =
            (Vec::new(), Vec::new(), HashSet::new(), false);

        for call in calls {
            let canonical = canonical_tool_name(&call.name);
            let is_mutating = self.get_spec(canonical).map(|s| s.is_mutating).unwrap_or(true);
            let is_global = matches!(
                canonical,
                "run_command" | "rollback_last_mutation" | "apply_unified_diff" | "batch_replace_regex"
            );
            let resources = self.extract_target_resources(call, cwd);

            if !is_mutating {
                let conflict = current_has_mutations && resources.iter().any(|r| current_resources.contains(r));
                if conflict || current_wave.len() >= self.max_concurrency {
                    if !current_wave.is_empty() {
                        waves.push(std::mem::take(&mut current_wave));
                        current_resources.clear();
                        current_has_mutations = false;
                    }
                }
                current_resources.extend(resources);
                current_wave.push(call.clone());
            } else {
                if !current_wave.is_empty() && !current_has_mutations {
                    waves.push(std::mem::take(&mut current_wave));
                    current_resources.clear();
                }
                let conflict = resources.is_empty() || resources.iter().any(|r| current_resources.contains(r));
                if is_global || conflict || current_wave.len() >= self.max_concurrency {
                    if !current_wave.is_empty() {
                        waves.push(std::mem::take(&mut current_wave));
                        current_resources.clear();
                    }
                }
                current_resources.extend(resources);
                current_wave.push(call.clone());
                current_has_mutations = true;
                if is_global {
                    waves.push(std::mem::take(&mut current_wave));
                    current_resources.clear();
                    current_has_mutations = false;
                }
            }
        }
        if !current_wave.is_empty() {
            waves.push(current_wave);
        }
        waves
    }

    pub async fn execute_batch(
        &self,
        calls: &[ScheduledToolCall],
        cwd: &Path,
        authority: ExecutionAuthorityLevel,
        cancel_token: Option<&CancellationToken>,
    ) -> Vec<ToolExecutionRecord> {
        let waves = self.partition_waves(calls, cwd);
        let mut results = Vec::with_capacity(calls.len());
        for wave in waves {
            if cancel_token.map(|t| t.is_cancelled()).unwrap_or(false) {
                break;
            }
            if wave.len() == 1 {
                results.push(self.execute_single(&wave[0], cwd, authority, cancel_token).await);
            } else {
                let futs = wave
                    .iter()
                    .map(|call| self.execute_single(call, cwd, authority, cancel_token));
                results.extend(join_all(futs).await);
            }
        }
        results
    }

    pub async fn execute_single(
        &self,
        call: &ScheduledToolCall,
        cwd: &Path,
        authority: ExecutionAuthorityLevel,
        cancel_token: Option<&CancellationToken>,
    ) -> ToolExecutionRecord {
        let started = Instant::now();
        let canonical = canonical_tool_name(&call.name);
        let Some(spec) = self.get_spec(canonical) else {
            let err_msg = format!("Tool '{}' not found in registry", call.name);
            return ToolExecutionRecord {
                name: call.name.clone(),
                call_id: Some(call.id.clone()),
                args: Some(call.args.clone()),
                output: json!({ "error": err_msg }),
                success: false,
                denied: None,
                output_limited: None,
                error: Some(err_msg),
                duration_ms: started.elapsed().as_millis() as u64,
            };
        };
        self.unlock_tool_by_name(spec.name);

        let (prepared_args, validation) = self.parser.prepare_arguments(&call.args, &spec.parameters);
        if !validation.valid {
            let err_msg = format!(
                "Tool '{}' argument schema validation failed: {}",
                canonical,
                validation.errors.join("; ")
            );
            return ToolExecutionRecord {
                name: call.name.clone(),
                call_id: Some(call.id.clone()),
                args: Some(Value::Object(prepared_args)),
                output: json!({ "error": err_msg, "suggestions": validation.suggestions }),
                success: false,
                denied: None,
                output_limited: None,
                error: Some(err_msg),
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }

        let soft_loop = authority.is_autonomous() || !spec.is_mutating;
        if let Ok(mut lb) = self.loop_breaker.lock() {
            if let Err(loop_err) = lb.record_and_check(canonical, &prepared_args, soft_loop) {
                return ToolExecutionRecord {
                    name: call.name.clone(),
                    call_id: Some(call.id.clone()),
                    args: Some(Value::Object(prepared_args)),
                    output: json!({ "error": loop_err }),
                    success: false,
                    denied: None,
                    output_limited: None,
                    error: Some(loop_err),
                    duration_ms: started.elapsed().as_millis() as u64,
                };
            }
        }

        let cacheable = !spec.is_mutating && ToolExecutionCache::is_cacheable(canonical);
        let cache_key = if cacheable {
            let key = ToolExecutionCache::compute_key(canonical, &prepared_args, cwd);
            if let Ok(mut c) = self.cache.lock() {
                if let Some(cached_val) = c.lookup(&key) {
                    return ToolExecutionRecord {
                        name: call.name.clone(),
                        call_id: Some(call.id.clone()),
                        args: Some(Value::Object(prepared_args)),
                        output: cached_val,
                        success: true,
                        denied: None,
                        output_limited: None,
                        error: None,
                        duration_ms: started.elapsed().as_millis() as u64,
                    };
                }
            }
            Some(key)
        } else {
            None
        };

        let target_resources = self.extract_target_resources(call, cwd);

        match self
            .dispatch_builtin(canonical, &prepared_args, cwd, cancel_token)
            .await
        {
            Ok((val, denied)) => {
                if spec.is_mutating {
                    if let Ok(mut c) = self.cache.lock() {
                        c.invalidate_paths(&target_resources);
                    }
                } else if let Some(key) = cache_key {
                    if let Ok(mut c) = self.cache.lock() {
                        c.store(key, target_resources, val.clone());
                    }
                }
                ToolExecutionRecord {
                    name: call.name.clone(),
                    call_id: Some(call.id.clone()),
                    args: Some(Value::Object(prepared_args)),
                    output: val,
                    success: !denied,
                    denied: if denied { Some(true) } else { None },
                    output_limited: None,
                    error: if denied {
                        Some("Permission denied for this operation".to_string())
                    } else {
                        None
                    },
                    duration_ms: started.elapsed().as_millis() as u64,
                }
            }
            Err(err_msg) => ToolExecutionRecord {
                name: call.name.clone(),
                call_id: Some(call.id.clone()),
                args: Some(Value::Object(prepared_args)),
                output: json!({ "error": err_msg }),
                success: false,
                denied: None,
                output_limited: None,
                error: Some(err_msg),
                duration_ms: started.elapsed().as_millis() as u64,
            },
        }
    }

    async fn dispatch_builtin(
        &self,
        canonical: &str,
        args: &Map<String, Value>,
        cwd: &Path,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<(Value, bool), String> {
        match canonical {
            "get_workspace_diff" => {
                let turn_id = args.get("turnId").and_then(|v| v.as_str());
                let report = self
                    .journal
                    .lock()
                    .map_err(|_| "Journal lock poisoned")?
                    .generate_session_diff(cwd, turn_id);
                Ok((report, false))
            }
            "run_command" => {
                let cmd_str = args.get("command").and_then(|v| v.as_str()).ok_or("Missing 'command'")?;
                SafetyPolicy::check_command(cmd_str)?;
                let cmd_cwd = args
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(|s| resolve_workspace_path(cwd, s))
                    .unwrap_or_else(|| cwd.to_path_buf());
                let timeout_ms = args
                    .get("timeoutMs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(30_000)
                    .clamp(100, 300_000);
                execute_shell_command(cmd_str, &cmd_cwd, timeout_ms, cancel_token).await
            }
            "rollback_last_mutation" => {
                let all_turn = args.get("allInCurrentTurn").and_then(|v| v.as_bool()).unwrap_or(false);
                let mut journal = self.journal.lock().map_err(|_| "Journal lock poisoned")?;
                let res = if all_turn {
                    journal.rollback_turn(None)
                } else {
                    journal.rollback_last()
                };
                if let Ok(mut c) = self.cache.lock() {
                    c.clear();
                }
                Ok((serde_json::to_value(res).unwrap_or(Value::Null), false))
            }
            "save_memory" => {
                let key = args.get("key").and_then(|v| v.as_str()).ok_or("Missing 'key'")?;
                let value = args.get("value").and_then(|v| v.as_str()).ok_or("Missing 'value'")?;
                let threats = SafetyPolicy::scan_for_threats(value, "all");
                if !threats.is_empty() {
                    return Err(format!("Blocked memory write containing security threats: {:?}", threats));
                }
                let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("fact");
                let mut mem = self.memory_store.lock().map_err(|_| "Memory lock poisoned")?;
                let entry = mem.save_memory(key, value, category);
                Ok((json!({ "success": true, "entry": entry }), false))
            }
            "search_memory" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let mem = self.memory_store.lock().map_err(|_| "Memory lock poisoned")?;
                let results = mem.search_memories(query);
                Ok((json!({ "count": results.len(), "memories": results }), false))
            }
            "retrieve_spill_content" => {
                let spill_id = args.get("spillId").and_then(|v| v.as_str()).ok_or("Missing 'spillId'")?;
                let start_line = args.get("startLine").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                let end_line = args.get("endLine").and_then(|v| v.as_u64()).map(|v| v as usize);
                let gov = self.governor.lock().map_err(|_| "Governor lock poisoned")?;
                match gov.retrieve_spill(spill_id, start_line, end_line) {
                    Some(v) => Ok((v, false)),
                    None => Err(format!("Spill ID '{spill_id}' not found or expired")),
                }
            }
            "computer_use" => {
                let act = args.get("action").and_then(|v| v.as_str()).unwrap_or("capture").trim().to_ascii_lowercase();
                match act.as_str() {
                    "orchestrate" | "closed_loop" => {
                        let goal = args.get("goal").or_else(|| args.get("objective")).and_then(|v| v.as_str()).unwrap_or("Execute computer use operation");
                        let contract = SpatialActionContract {
                            action_name: args.get("target_action").or_else(|| args.get("sub_action")).and_then(|v| v.as_str()).unwrap_or("click").to_string(),
                            target_selector: args.get("target_id").or_else(|| args.get("selector")).or_else(|| args.get("ref")).or_else(|| args.get("element_id")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                            som_id: args.get("som_id").and_then(|v| v.as_u64()).map(|n| n as u32),
                            delivery_mode: args.get("delivery_mode").and_then(|v| v.as_str()).unwrap_or("background").to_string(),
                            payload_text: args.get("text").or_else(|| args.get("value")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                            keys: args.get("keys").or_else(|| args.get("key")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                            ..SpatialActionContract::default()
                        };
                        let res = self.computer_orchestrator.execute_closed_loop_cycle(goal, &contract)?;
                        Ok((res, false))
                    }
                    "observe" | "env_observe" => {
                        let obs = self.computer_orchestrator.observe();
                        Ok((serde_json::to_value(obs).unwrap_or(Value::Null), false))
                    }
                    "interpret" | "env_interpret" => {
                        let obs = self.computer_orchestrator.observe();
                        let interp = self.computer_orchestrator.interpret(&obs);
                        Ok((interp, false))
                    }
                    "consult_swarm" | "swarm_consult" => {
                        let goal = args.get("goal").or_else(|| args.get("objective")).and_then(|v| v.as_str()).unwrap_or("Environmental interpretation");
                        let app = args.get("app").and_then(|v| v.as_str()).unwrap_or("Heav3nsCode");
                        let req = SwarmConsultationRequest::new("consult-cli", goal, app, "Active Window");
                        let res = self.computer_orchestrator.consult(&req);
                        Ok((serde_json::to_value(res).unwrap_or(Value::Null), false))
                    }
                    _ => {
                        let val_args = Value::Object(args.clone());
                        let mut disp = self.display_state.lock().map_err(|_| "Display lock poisoned")?;
                        let res = disp.execute_action(&val_args)?;
                        Ok((res, false))
                    }
                }
            }
            "browser_use" => {
                let val_args = Value::Object(args.clone());
                let mut disp = self.display_state.lock().map_err(|_| "Display lock poisoned")?;
                let res = disp.execute_browser_action(&val_args)?;
                Ok((res, false))
            }
            "spatial_interact" => {
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("execute");
                let contract = SpatialActionContract {
                    action_name: args.get("target_action").or_else(|| args.get("sub_action")).and_then(|v| v.as_str()).unwrap_or("click").to_string(),
                    target_selector: args.get("target_id").or_else(|| args.get("selector")).or_else(|| args.get("ref")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    som_id: args.get("som_id").and_then(|v| v.as_u64()).map(|n| n as u32),
                    coordinates: if args.get("x").is_some() && args.get("y").is_some() {
                        let x = args.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let y = args.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        Some([x, y])
                    } else {
                        None
                    },
                    coordinate_space: args.get("coordinate_space").and_then(|v| v.as_str()).unwrap_or("screen").to_string(),
                    delivery_mode: args.get("delivery_mode").and_then(|v| v.as_str()).unwrap_or("background").to_string(),
                    required_confidence: args.get("required_confidence").and_then(|v| v.as_f64()).unwrap_or(0.50) as f32,
                    max_occlusion: args.get("max_occlusion").and_then(|v| v.as_f64()).unwrap_or(0.60),
                    expected_epoch: args.get("expected_epoch").and_then(|v| v.as_u64()),
                    payload_text: args.get("text").or_else(|| args.get("value")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    keys: args.get("keys").or_else(|| args.get("key")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    to_coordinates: None,
                    exclusion_zones: Vec::new(),
                    auto_scroll_if_needed: true,
                };

                match action {
                    "ground" => {
                        let disp = self.display_state.lock().map_err(|_| "Display lock poisoned")?;
                        let fresh = SpatialInteractionEngine::extract_scene_from_display(&disp);
                        let mut med = self.spatial_engine.mediator.lock().map_err(|_| "Mediator lock poisoned")?;
                        med.update_scene(fresh);
                        let res = med.ground_action(&contract);
                        Ok((serde_json::to_value(res).unwrap_or(Value::Null), false))
                    }
                    "plan" => {
                        let disp = self.display_state.lock().map_err(|_| "Display lock poisoned")?;
                        let fresh = SpatialInteractionEngine::extract_scene_from_display(&disp);
                        let mut med = self.spatial_engine.mediator.lock().map_err(|_| "Mediator lock poisoned")?;
                        med.update_scene(fresh);
                        let plan = med.plan_action(&contract);
                        Ok((serde_json::to_value(plan).unwrap_or(Value::Null), false))
                    }
                    "execute_plan" | "run_plan" => {
                        let res = self.spatial_engine.execute_plan(&contract)?;
                        Ok((res, false))
                    }
                    "query" => {
                        let origin = args.get("origin_id").or_else(|| args.get("target_id")).and_then(|v| v.as_str()).unwrap_or("");
                        let dir = args.get("direction").and_then(|v| v.as_str()).unwrap_or("right");
                        let matched = self.spatial_engine.query_directional(origin, dir);
                        Ok((json!({ "origin": origin, "direction": dir, "matched": matched }), false))
                    }
                    "reconcile" => {
                        let dom_els = args.get("interactiveElements").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                        let scene = self.spatial_engine.reconcile_dom_and_visual(&dom_els);
                        Ok((serde_json::to_value(scene).unwrap_or(Value::Null), false))
                    }
                    "audit" => {
                        Ok((self.spatial_engine.audit_health(), false))
                    }
                    "orchestrate" | "closed_loop" => {
                        let goal = args.get("goal").or_else(|| args.get("objective")).and_then(|v| v.as_str()).unwrap_or("Spatial closed-loop execution");
                        let res = self.computer_orchestrator.execute_closed_loop_cycle(goal, &contract)?;
                        Ok((res, false))
                    }
                    _ => {
                        let res = self.spatial_engine.execute_spatially_grounded_action(&contract)?;
                        Ok((res, false))
                    }
                }
            }
            "vision_inspect" => {
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("inspect");
                let w = args.get("width").and_then(|v| v.as_u64()).unwrap_or(800) as u32;
                let h = args.get("height").and_then(|v| v.as_u64()).unwrap_or(600) as u32;
                match action {
                    "generate_svg" => {
                        let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("HEAV3NS-MICRO");
                        let svg = generate_synthetic_svg(w, h, title);
                        let meta = decode_image_metadata(svg.as_bytes())?;
                        Ok((json!({ "svg": svg, "metadata": meta }), false))
                    }
                    "generate_bmp" => {
                        let bmp = generate_synthetic_bmp(w, h, 15, 23, 42);
                        let meta = decode_image_metadata(&bmp)?;
                        Ok((json!({ "byteLength": bmp.len(), "metadata": meta }), false))
                    }
                    _ => {
                        let path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path' for vision_inspect")?;
                        let meta = SkillsIngestor::inspect_workspace_image(cwd, path)?;
                        Ok((meta, false))
                    }
                }
            }
            "skill_forge" => {
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list_dag");
                let val_args = Value::Object(args.clone());
                let mut cat = self.skill_catalog.lock().map_err(|_| "Skill catalog lock poisoned")?;
                let res = match action {
                    "list_dag" | "dag" | "list" => cat.evaluate_dag(),
                    "forge" | "create" | "wizard" => cat.forge_skill(&val_args)?,
                    "apply_pack" | "power_up" => {
                        let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator");
                        let pack = args.get("pack").and_then(|v| v.as_str()).unwrap_or("tdd-enforcer");
                        cat.apply_power_up_pack(id, pack)?
                    }
                    "clone" | "fork" => {
                        let src = args.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator");
                        let new_id = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("computer-use-fork");
                        cat.clone_skill(src, new_id, None)?
                    }
                    "lint" => {
                        let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator");
                        cat.lint_and_autofix_skill(id, false)?
                    }
                    "autofix" | "doctor" => {
                        let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator");
                        cat.lint_and_autofix_skill(id, true)?
                    }
                    "sync_vault" | "sync" => {
                        let vault_files = SkillsIngestor::read_drop_vault_files(cwd);
                        let mut ingested = Vec::new();
                        for (fname, raw) in vault_files {
                            if let Ok(item) = cat.ingest_multi_format(&fname, &raw) {
                                ingested.push(item);
                            }
                        }
                        json!({
                            "syncedCount": ingested.len(),
                            "totalCatalogSkills": cat.nodes.len(),
                            "activeSoul": cat.active_soul.name,
                            "ingested": ingested
                        })
                    }
                    "export" => {
                        let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator");
                        let fmt = args.get("format").and_then(|v| v.as_str()).unwrap_or("markdown");
                        cat.export_node(id, fmt)?
                    }
                    "ingest" => {
                        let fname = args.get("id").and_then(|v| v.as_str()).unwrap_or("imported.md");
                        let content = args.get("content").or_else(|| args.get("prompt")).and_then(|v| v.as_str()).unwrap_or("");
                        cat.ingest_multi_format(fname, content)?
                    }
                    "soul_status" | "soul" => json!({ "activeSoul": cat.active_soul, "competencies": cat.competency_scores }),
                    other => return Err(format!("Unsupported skill_forge action '{other}'")),
                };
                Ok((res, false))
            }
            "engage_swarm" => {
                let objective = args.get("objective").and_then(|v| v.as_str()).unwrap_or("Execute complex objective");
                let strategy = args.get("strategy").and_then(|v| v.as_str());
                let max_spec = args.get("maxSpecialists").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
                let plan = classify_task_for_legion(objective, &[], strategy, max_spec);
                Ok((plan, false))
            }
            other => {
                let res = dispatch_fs_tool(other, args, cwd, &self.journal)?;
                if other == "batch_replace_regex" {
                    let dry_run = args.get("dryRun").and_then(|v| v.as_bool()).unwrap_or(false);
                    if !dry_run {
                        if let Ok(mut c) = self.cache.lock() {
                            c.clear();
                        }
                    }
                }
                Ok(res)
            }
        }
    }
}

impl IToolExecutor for ToolRegistry {
    fn execute_batch_contract<'a>(
        &'a self,
        calls: &'a [ScheduledToolCall],
        cwd: &'a Path,
        authority: ExecutionAuthorityLevel,
        cancel_token: Option<&'a CancellationToken>,
    ) -> BoxFuture<'a, Vec<ToolExecutionRecord>> {
        Box::pin(async move { self.execute_batch(calls, cwd, authority, cancel_token).await })
    }
}
