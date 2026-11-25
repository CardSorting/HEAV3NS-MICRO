// [LAYER: CORE]
use crate::core::computer_use_orchestrator::ComputerUseOrchestrator;
use crate::core::spatial_engine::SpatialInteractionEngine;
use crate::core::tool_governor::{
    FileReadTracker, ToolExecutionCache, ToolLoopBreaker, ToolOutputGovernor,
};
use crate::core::tool_specs::builtin_tool_specs;
use crate::domain::computer_use::VirtualDisplayState;
use crate::domain::interfaces::{BoxFuture, IToolExecutor};
use crate::domain::memory::MemoryStore;
use crate::domain::skill_tree::SkillTreeCatalog;
use crate::domain::swarm::classify_task_for_legion;
use crate::domain::tool_parser::ToolCallArgParser;
use crate::domain::types::posture_types::RepoConstitution;
use crate::domain::types::progress::ToolExecutionRecord;
use crate::domain::types::runtime::{
    ApprovalGateCallback, CancellationToken, ExecutionAuthorityLevel, ExecutionPosture,
};
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::domain::types::tool_types::ToolSpec;
use crate::infrastructure::behavioral_store::BehavioralKnowledgeStore;
use crate::infrastructure::fs_tools::dispatch_fs_tool;
use crate::infrastructure::search_tools::resolve_workspace_path;
use crate::infrastructure::shell_runner::execute_shell_command_sandboxed;
use crate::infrastructure::transaction_journal::TransactionJournal;
use crate::infrastructure::vfs_store::SkillsIngestor;
use crate::plumbing::hash_and_math::lock_unpoison;
use crate::plumbing::secret_redactor::SafetyPolicy;
use crate::plumbing::text_tokens::canonical_tool_name;
use crate::plumbing::vision_and_dom::{
    decode_image_metadata, generate_synthetic_bmp, generate_synthetic_svg,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Hyper-compressed Tool Registry, Wave Scheduler, Posture Gate, Staleness Tracker, and Sandboxed I/O Orchestrator.
pub struct ToolRegistry {
    specs: HashMap<&'static str, ToolSpec>,
    ordered_names: Vec<&'static str>,
    cached_openai_schemas: HashMap<&'static str, Value>,
    pub parser: ToolCallArgParser,
    pub journal: Mutex<TransactionJournal>,
    pub cache: Mutex<ToolExecutionCache>,
    pub governor: Mutex<ToolOutputGovernor>,
    pub loop_breaker: Mutex<ToolLoopBreaker>,
    pub read_tracker: Mutex<FileReadTracker>,
    pub memory_store: Arc<Mutex<MemoryStore>>,
    pub display_state: Arc<Mutex<VirtualDisplayState>>,
    pub spatial_engine: Arc<SpatialInteractionEngine>,
    pub behavioral_store: Arc<Mutex<BehavioralKnowledgeStore>>,
    pub computer_orchestrator: Arc<ComputerUseOrchestrator>,
    pub skill_catalog: Mutex<SkillTreeCatalog>,
    pub unlocked_tools: Mutex<BTreeSet<&'static str>>,
    pub unlocked_lru: Mutex<Vec<&'static str>>,
    pub evicted_tools: Mutex<Vec<&'static str>>,
    pub active_posture: Mutex<Option<ExecutionPosture>>,
    pub session_allowlist: Mutex<Vec<String>>,
    pub constitution: Mutex<Option<RepoConstitution>>,
    pub approval_gate: Mutex<Option<ApprovalGateCallback>>,
    pub turn_approvals: Mutex<Vec<String>>,
    pub forbid_swarm: std::sync::atomic::AtomicBool,
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
            cached_openai_schemas: HashMap::new(),
            parser: ToolCallArgParser::new(),
            journal: Mutex::new(TransactionJournal::new()),
            cache: Mutex::new(ToolExecutionCache::default()),
            governor: Mutex::new(ToolOutputGovernor::new()),
            loop_breaker: Mutex::new(ToolLoopBreaker::default()),
            read_tracker: Mutex::new(FileReadTracker::default()),
            memory_store,
            display_state,
            spatial_engine,
            behavioral_store,
            computer_orchestrator,
            skill_catalog: Mutex::new(SkillTreeCatalog::new()),
            unlocked_tools: Mutex::new(BTreeSet::new()),
            unlocked_lru: Mutex::new(Vec::new()),
            evicted_tools: Mutex::new(Vec::new()),
            active_posture: Mutex::new(None),
            session_allowlist: Mutex::new(Vec::new()),
            constitution: Mutex::new(None),
            approval_gate: Mutex::new(None),
            turn_approvals: Mutex::new(Vec::new()),
            forbid_swarm: std::sync::atomic::AtomicBool::new(false),
            max_concurrency: 8,
        };
        for spec in builtin_tool_specs() {
            registry.register(spec);
        }
        registry
    }

    fn register(&mut self, spec: ToolSpec) {
        self.ordered_names.push(spec.name);
        self.cached_openai_schemas
            .insert(spec.name, spec.to_openai_tool());
        self.specs.insert(spec.name, spec);
    }

    pub fn get_spec(&self, name: &str) -> Option<&ToolSpec> {
        self.specs.get(canonical_tool_name(name))
    }

    pub fn list_specs(&self) -> Vec<&ToolSpec> {
        self.ordered_names
            .iter()
            .filter_map(|name| self.specs.get(name))
            .collect()
    }

    pub fn openai_tool_schemas(&self) -> Vec<Value> {
        self.list_specs()
            .iter()
            .map(|s| {
                self.cached_openai_schemas
                    .get(s.name)
                    .cloned()
                    .unwrap_or_else(|| s.to_openai_tool())
            })
            .collect()
    }

    pub fn select_relevant_tool_schemas(&self, prompt: &str) -> Vec<Value> {
        self.select_tool_schemas_for(prompt, false)
    }

    /// Selects relevant OpenAI tool schemas with bounded 8-slot LRU deferred unlocking and structural `engage_swarm` exclusion for specialists.
    pub fn select_tool_schemas_for(&self, prompt: &str, forbid_swarm: bool) -> Vec<Value> {
        self.select_or_peek_tool_schemas(prompt, forbid_swarm, true)
    }

    /// Read-only preview of tool schemas for `/context` & HUD inspection without mutating LRU state.
    pub fn peek_tool_schemas_for(&self, prompt: &str, forbid_swarm: bool) -> Vec<Value> {
        self.select_or_peek_tool_schemas(prompt, forbid_swarm, false)
    }

    fn select_or_peek_tool_schemas(
        &self,
        prompt: &str,
        forbid_swarm: bool,
        mutate_lru: bool,
    ) -> Vec<Value> {
        let lower = prompt.trim().to_lowercase();
        if lower.is_empty() {
            return self
                .list_specs()
                .into_iter()
                .filter(|spec| !forbid_swarm || spec.name != "engage_swarm")
                .map(|s| {
                self.cached_openai_schemas
                    .get(s.name)
                    .cloned()
                    .unwrap_or_else(|| s.to_openai_tool())
            })
                .collect();
        }
        let mut to_unlock = Vec::new();
        for spec in self.list_specs() {
            if spec.is_core || (forbid_swarm && spec.name == "engage_swarm") {
                continue;
            }
            let matched = lower.contains(spec.name)
                || (matches!(spec.name, "list_skills" | "use_skill" | "skill_forge")
                    && (lower.contains("skill")
                        || lower.contains("workflow")
                        || lower.contains("soul")
                        || lower.contains("forge")
                        || lower.contains("dag")))
                || (spec.name == "computer_use"
                    && (lower.contains("computer")
                        || lower.contains("click")
                        || lower.contains("window")
                        || lower.contains("display")
                        || lower.contains("gui")
                        || lower.contains("screen")
                        || lower.contains("clipboard")))
                || (spec.name == "browser_use"
                    && (lower.contains("browser")
                        || lower.contains("dom")
                        || lower.contains("url")
                        || lower.contains("http")
                        || lower.contains("web")
                        || lower.contains("navigate")
                        || lower.contains(".html")))
                || (spec.name == "vision_inspect"
                    && (lower.contains("vision")
                        || lower.contains("image")
                        || lower.contains("png")
                        || lower.contains("svg")
                        || lower.contains("jpeg")
                        || lower.contains("jpg")
                        || lower.contains("webp")
                        || lower.contains("bmp")))
                || (spec.name == "engage_swarm"
                    && (lower.contains("swarm")
                        || lower.contains("legion")
                        || lower.contains("specialist")
                        || lower.contains("parallel")))
                || (spec.name == "spatial_interact"
                    && (lower.contains("spatial")
                        || lower.contains("bounding")
                        || lower.contains("ground")
                        || lower.contains("occlu")
                        || lower.contains("geometry")
                        || lower.contains("assurance")))
                || (spec.name == "get_workspace_diff"
                    && (lower.contains("diff")
                        || lower.contains("patch")
                        || lower.contains("git")))
                || (spec.name == "batch_replace_regex"
                    && (lower.contains("symbol")
                        || lower.contains("rename")
                        || lower.contains("refactor")
                        || lower.contains("regex")))
                || (spec.name == "move_file"
                    && (lower.contains("move") || lower.contains("rename")))
                || (spec.name == "directory_tree" && lower.contains("tree"))
                || (spec.name == "file_info"
                    && (lower.contains("metadata")
                        || lower.contains("sha256")
                        || lower.contains("digest")))
                || (spec.name == "edit_file_anchored"
                    && (lower.contains("anchor") || lower.contains("fnv")));
            if matched {
                to_unlock.push(spec.name);
            }
        }
        if mutate_lru {
            for name in &to_unlock {
                self.touch_unlocked_lru(name);
            }
        }
        let unlocked = lock_unpoison(&self.unlocked_tools);
        self.list_specs()
            .into_iter()
            .filter(|spec| {
                (!forbid_swarm || spec.name != "engage_swarm")
                    && (spec.is_core
                        || unlocked.contains(spec.name)
                        || (!mutate_lru && to_unlock.contains(&spec.name)))
            })
            .map(|s| {
                self.cached_openai_schemas
                    .get(s.name)
                    .cloned()
                    .unwrap_or_else(|| s.to_openai_tool())
            })
            .collect()
    }

    pub fn unlock_tool_by_name(&self, name: &str) {
        if let Some(spec) = self.get_spec(name) {
            if !spec.is_core {
                self.touch_unlocked_lru(spec.name);
            }
        }
    }

    pub fn clear_turn_approval_gate(&self) {
        *lock_unpoison(&self.approval_gate) = None;
    }

    pub fn configure_turn_security(
        &self,
        posture: Option<ExecutionPosture>,
        constitution: Option<RepoConstitution>,
        approval_gate: Option<ApprovalGateCallback>,
    ) {
        *lock_unpoison(&self.active_posture) = posture;
        *lock_unpoison(&self.constitution) = constitution;
        *lock_unpoison(&self.approval_gate) = approval_gate;
        lock_unpoison(&self.turn_approvals).clear();
    }

    pub fn effective_posture_for(&self, authority: ExecutionAuthorityLevel) -> ExecutionPosture {
        if let Some(p) = *lock_unpoison(&self.active_posture) {
            return p;
        }
        match authority {
            ExecutionAuthorityLevel::Strict | ExecutionAuthorityLevel::Interactive => {
                ExecutionPosture::Ask
            }
            ExecutionAuthorityLevel::Balanced => ExecutionPosture::AcceptEdits,
            ExecutionAuthorityLevel::HighThroughput | ExecutionAuthorityLevel::Autonomous => {
                ExecutionPosture::AutoReview
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
        if let Ok(mut lru) = self.unlocked_lru.lock() {
            lru.clear();
        }
        if let Ok(mut ev) = self.evicted_tools.lock() {
            ev.clear();
        }
        if let Ok(mut rt) = self.read_tracker.lock() {
            rt.reset_stamps();
        }
    }

    pub fn extract_target_resources_from_map(
        &self,
        canonical: &str,
        obj: &serde_json::Map<String, Value>,
        cwd: &Path,
    ) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for key in [
            "path",
            "filePath",
            "file_path",
            "source",
            "sourcePath",
            "source_path",
            "src",
            "target",
            "target_file",
            "target_path",
            "cwd",
            "file",
            "targetFile",
            "targetPath",
            "filename",
            "from",
            "to",
            "dest",
            "destination",
            "inputPath",
            "outputPath",
            "directory",
            "dir",
            "subpath",
        ] {
            if let Some(Value::String(s)) = obj.get(key) {
                if !s.trim().is_empty() && !s.contains('\n') {
                    let p = crate::core::tool_governor::normalize_tracker_key(
                        &resolve_workspace_path(cwd, s),
                    );
                    if !paths.contains(&p) {
                        paths.push(p);
                    }
                }
            }
        }
        if canonical == "apply_unified_diff" {
            if let Some(diff_txt) = obj
                .get("diff")
                .or_else(|| obj.get("patch"))
                .and_then(Value::as_str)
            {
                for op in crate::plumbing::fuzzy_patch::DeterministicPatchEngine::parse(diff_txt) {
                    let p = crate::core::tool_governor::normalize_tracker_key(
                        &resolve_workspace_path(cwd, &op.file_path),
                    );
                    if !paths.contains(&p) {
                        paths.push(p);
                    }
                }
            }
        }
        paths
    }

    pub fn extract_target_resources(&self, call: &ScheduledToolCall, cwd: &Path) -> Vec<PathBuf> {
        let canonical = canonical_tool_name(&call.name);
        if let Some(obj) = call.args.as_object() {
            return self.extract_target_resources_from_map(canonical, obj, cwd);
        }
        if let Some(s) = call.args.as_str() {
            if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(s) {
                return self.extract_target_resources_from_map(canonical, &obj, cwd);
            }
        }
        Vec::new()
    }

    pub fn partition_waves(
        &self,
        calls: &[ScheduledToolCall],
        cwd: &Path,
    ) -> Vec<Vec<ScheduledToolCall>> {
        if calls.len() <= 1 {
            return if calls.is_empty() {
                Vec::new()
            } else {
                vec![calls.to_vec()]
            };
        }
        let (mut waves, mut current_wave, mut current_resources, mut current_has_mutations) =
            (Vec::new(), Vec::new(), HashSet::new(), false);

        let overlaps = |cur: &HashSet<PathBuf>, res: &[PathBuf]| {
            res.iter().any(|r| {
                cur.iter()
                    .any(|cr| r == cr || r.starts_with(cr) || cr.starts_with(r))
            })
        };
        for call in calls {
            let canonical = canonical_tool_name(&call.name);
            let is_mutating = self
                .get_spec(canonical)
                .map(|s| s.is_mutating)
                .unwrap_or(true);
            let is_global = matches!(
                canonical,
                "run_command"
                    | "execute_tools"
                    | "rollback_last_mutation"
                    | "apply_unified_diff"
                    | "batch_replace_regex"
            );
            let resources = self.extract_target_resources(call, cwd);

            if !is_mutating {
                let conflict = current_has_mutations
                    && (resources.is_empty() || overlaps(&current_resources, &resources));
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
                let isolate = is_global || resources.is_empty();
                let conflict = isolate || overlaps(&current_resources, &resources);
                if conflict || current_wave.len() >= self.max_concurrency {
                    if !current_wave.is_empty() {
                        waves.push(std::mem::take(&mut current_wave));
                        current_resources.clear();
                    }
                }
                current_resources.extend(resources);
                current_wave.push(call.clone());
                current_has_mutations = true;
                if isolate {
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

    pub(crate) fn failed_record(
        call: &ScheduledToolCall,
        args: Value,
        err_msg: String,
        denied: Option<bool>,
        started: Instant,
    ) -> ToolExecutionRecord {
        ToolExecutionRecord {
            name: call.name.clone(),
            call_id: Some(call.id.clone()),
            args: Some(args),
            output: json!({ "error": err_msg }),
            success: false,
            denied,
            output_limited: None,
            error: Some(err_msg),
            duration_ms: started.elapsed().as_millis() as u64,
        }
    }

    pub(crate) async fn dispatch_builtin(
        &self,
        canonical: &str,
        args: &Map<String, Value>,
        cwd: &Path,
        authority: ExecutionAuthorityLevel,
        posture: ExecutionPosture,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<(Value, bool), String> {
        match canonical {
            "tool_search" => {
                let query = args.get("query").and_then(Value::as_str).unwrap_or("");
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(4) as usize;
                let forbid = self.forbid_swarm.load(std::sync::atomic::Ordering::Relaxed);
                Ok((self.search_and_unlock_tools(query, limit, forbid), false))
            }
            "execute_tools" => {
                let calls = args.get("calls").ok_or("Missing 'calls' array")?;
                let stop_on_error = args
                    .get("stopOnError")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                self.execute_declarative_batch(calls, stop_on_error, cwd, authority, cancel_token)
                    .await
            }
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
                let cmd_str = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'command'")?;
                SafetyPolicy::check_command(cmd_str)?;
                let advisory = SafetyPolicy::detect_heredoc_file_redirection(cmd_str)
                    .or_else(|| SafetyPolicy::detect_terminal_file_read(cmd_str));
                let cmd_cwd = args
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(|s| resolve_workspace_path(cwd, s))
                    .unwrap_or_else(|| cwd.to_path_buf());
                let (Ok(cr), Ok(cc)) = (cwd.canonicalize(), cmd_cwd.canonicalize()) else {
                    return Err(format!("Invalid cwd '{}'", cmd_cwd.display()));
                };
                if !cc.starts_with(&cr) {
                    return Err(format!(
                        "Command cwd '{}' escapes workspace root",
                        cmd_cwd.display()
                    ));
                }
                let timeout_ms = args
                    .get("timeoutMs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(30_000)
                    .clamp(100, 300_000);
                let (mut out, denied) = execute_shell_command_sandboxed(
                    cmd_str,
                    &cmd_cwd,
                    timeout_ms,
                    posture.sandbox_mode(),
                    cancel_token,
                )
                .await?;
                if let (Some(adv), Some(obj)) = (advisory, out.as_object_mut()) {
                    obj.insert("shellGuardAdvisory".to_string(), Value::String(adv));
                }
                Ok((out, denied))
            }
            "rollback_last_mutation" => {
                let all_turn = args
                    .get("allInCurrentTurn")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let mut journal = self.journal.lock().map_err(|_| "Journal lock poisoned")?;
                let res = if all_turn {
                    journal.rollback_turn(None)
                } else {
                    journal.rollback_last()
                };
                if let Ok(mut c) = self.cache.lock() {
                    c.clear();
                }
                if let Ok(mut rt) = self.read_tracker.lock() {
                    rt.reset_stamps();
                }
                Ok((serde_json::to_value(res).unwrap_or(Value::Null), false))
            }
            "save_memory" => {
                let key = args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'key'")?;
                let value = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'value'")?;
                let category = args
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("fact");
                let mut threats = SafetyPolicy::scan_for_threats(value, "all");
                threats.extend(SafetyPolicy::scan_for_threats(key, "all"));
                threats.extend(SafetyPolicy::scan_for_threats(category, "all"));
                if !threats.is_empty() {
                    return Err(format!(
                        "Blocked memory write containing security threats: {:?}",
                        threats
                    ));
                }
                let mut mem = self
                    .memory_store
                    .lock()
                    .map_err(|_| "Memory lock poisoned")?;
                let entry = mem.save_memory(key, value, category);
                Ok((json!({ "success": true, "entry": entry }), false))
            }
            "search_memory" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let mem = self
                    .memory_store
                    .lock()
                    .map_err(|_| "Memory lock poisoned")?;
                let results = mem.search_memories(query);
                Ok((
                    json!({ "count": results.len(), "memories": results }),
                    false,
                ))
            }
            "retrieve_spill_content" => {
                let spill_id = args
                    .get("spillId")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'spillId'")?;
                let start_line =
                    args.get("startLine").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                let end_line = args
                    .get("endLine")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                let gov = self.governor.lock().map_err(|_| "Governor lock poisoned")?;
                match gov.retrieve_spill(spill_id, start_line, end_line) {
                    Some(v) => Ok((v, false)),
                    None => Err(format!("Spill ID '{spill_id}' not found or expired")),
                }
            }
            "computer_use" => self.dispatch_computer_use_tool(args),
            "browser_use" => {
                let val_args = Value::Object(args.clone());
                let mut disp = self
                    .display_state
                    .lock()
                    .map_err(|_| "Display lock poisoned")?;
                Ok((disp.execute_browser_action(&val_args)?, false))
            }
            "spatial_interact" => self.dispatch_spatial_tool(args),
            "vision_inspect" => {
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("inspect");
                let w = args.get("width").and_then(|v| v.as_u64()).unwrap_or(800) as u32;
                let h = args.get("height").and_then(|v| v.as_u64()).unwrap_or(600) as u32;
                match action {
                    "generate_svg" => {
                        let title = args
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("HEAV3NS-MICRO");
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
                        let path = args
                            .get("path")
                            .and_then(|v| v.as_str())
                            .ok_or("Missing 'path' for vision_inspect")?;
                        Ok((SkillsIngestor::inspect_workspace_image(cwd, path)?, false))
                    }
                }
            }
            "skill_forge" => self.dispatch_skill_forge_tool(args, cwd),
            "engage_swarm" => {
                let objective = args
                    .get("objective")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Execute complex objective");
                let strategy = args.get("strategy").and_then(|v| v.as_str());
                let max_spec = args
                    .get("maxSpecialists")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5) as usize;
                Ok((
                    classify_task_for_legion(objective, &[], strategy, max_spec),
                    false,
                ))
            }
            other => {
                let on_multi = tokio::runtime::Handle::try_current()
                    .map(|h| h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread)
                    .unwrap_or(false);
                let res = if on_multi {
                    tokio::task::block_in_place(|| {
                        dispatch_fs_tool(other, args, cwd, &self.journal)
                    })?
                } else {
                    dispatch_fs_tool(other, args, cwd, &self.journal)?
                };
                if other == "batch_replace_regex"
                    && !args
                        .get("dryRun")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                {
                    if let Ok(mut c) = self.cache.lock() {
                        c.clear();
                    }
                    if let Ok(mut rt) = self.read_tracker.lock() {
                        rt.reset_stamps();
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
        Box::pin(async move {
            self.execute_batch(calls, cwd, authority, cancel_token)
                .await
        })
    }
}
