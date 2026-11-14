// [LAYER: CORE]
use crate::core::computer_use_orchestrator::ComputerUseOrchestrator;
use crate::core::spatial_engine::SpatialInteractionEngine;
use crate::core::tool_governor::{FileReadTracker, ToolExecutionCache, ToolLoopBreaker, ToolOutputGovernor};
use crate::core::tool_specs::builtin_tool_specs;
use crate::domain::computer_use::VirtualDisplayState;
use crate::domain::interfaces::{BoxFuture, IToolExecutor};
use crate::domain::memory::MemoryStore;
use crate::domain::skill_tree::SkillTreeCatalog;
use crate::domain::swarm::classify_task_for_legion;
use crate::domain::tool_parser::ToolCallArgParser;
use crate::domain::types::posture_types::{evaluate_tool_posture, AuthorizationVerdict, RepoConstitution};
use crate::domain::types::progress::ToolExecutionRecord;
use crate::domain::types::runtime::{ApprovalGateCallback, CancellationToken, ExecutionAuthorityLevel, ExecutionPosture, UserApprovalChoice};
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
use crate::plumbing::vision_and_dom::{decode_image_metadata, generate_synthetic_bmp, generate_synthetic_svg};
use futures::future::join_all;
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Hyper-compressed Tool Registry, Wave Scheduler, Posture Gate, Staleness Tracker, and Sandboxed I/O Orchestrator.
pub struct ToolRegistry {
    specs: HashMap<&'static str, ToolSpec>,
    ordered_names: Vec<&'static str>,
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

    fn select_or_peek_tool_schemas(&self, prompt: &str, forbid_swarm: bool, mutate_lru: bool) -> Vec<Value> {
        let lower = prompt.trim().to_lowercase();
        if lower.is_empty() {
            return self
                .list_specs()
                .into_iter()
                .filter(|spec| !forbid_swarm || spec.name != "engage_swarm")
                .map(|s| s.to_openai_tool())
                .collect();
        }
        let mut to_unlock = Vec::new();
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
                    && (spec.is_core || unlocked.contains(spec.name) || (!mutate_lru && to_unlock.contains(&spec.name)))
            })
            .map(|s| s.to_openai_tool())
            .collect()
    }

    pub fn unlock_tool_by_name(&self, name: &str) {
        if let Some(spec) = self.get_spec(name) {
            if !spec.is_core {
                self.touch_unlocked_lru(spec.name);
            }
        }
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
            ExecutionAuthorityLevel::Strict | ExecutionAuthorityLevel::Interactive => ExecutionPosture::Ask,
            ExecutionAuthorityLevel::Balanced => ExecutionPosture::AcceptEdits,
            ExecutionAuthorityLevel::HighThroughput | ExecutionAuthorityLevel::Autonomous => ExecutionPosture::AutoReview,
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

    pub fn partition_waves(&self, calls: &[ScheduledToolCall], cwd: &Path) -> Vec<Vec<ScheduledToolCall>> {
        if calls.len() <= 1 {
            return if calls.is_empty() { Vec::new() } else { vec![calls.to_vec()] };
        }
        let (mut waves, mut current_wave, mut current_resources, mut current_has_mutations) =
            (Vec::new(), Vec::new(), HashSet::new(), false);

        for call in calls {
            let canonical = canonical_tool_name(&call.name);
            let is_mutating = self.get_spec(canonical).map(|s| s.is_mutating).unwrap_or(true);
            let is_global = matches!(
                canonical,
                "run_command" | "execute_tools" | "rollback_last_mutation" | "apply_unified_diff" | "batch_replace_regex"
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
        let mut cancelled = false;
        for wave in waves {
            if cancelled || cancel_token.map(|t| t.is_cancelled()).unwrap_or(false) {
                cancelled = true;
                let now = Instant::now();
                for call in &wave {
                    results.push(Self::failed_record(
                        call,
                        call.args.clone(),
                        "Tool execution cancelled by user before dispatch".to_string(),
                        None,
                        now,
                    ));
                }
                continue;
            }
            if wave.len() == 1 {
                results.push(self.execute_single(&wave[0], cwd, authority, cancel_token).await);
            } else {
                let futs = wave.iter().map(|call| self.execute_single(call, cwd, authority, cancel_token));
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
            return Self::failed_record(call, call.args.clone(), err_msg, None, started);
        };
        self.unlock_tool_by_name(spec.name);

        let (prepared_args, validation) = self.parser.prepare_arguments(&call.args, &spec.parameters);
        let prepared_val = Value::Object(prepared_args.clone());
        if !validation.valid {
            let err_msg = format!(
                "Tool '{}' argument schema validation failed: {}",
                canonical,
                validation.errors.join("; ")
            );
            return ToolExecutionRecord {
                name: call.name.clone(),
                call_id: Some(call.id.clone()),
                args: Some(prepared_val),
                output: json!({ "error": err_msg, "suggestions": validation.suggestions }),
                success: false,
                denied: None,
                output_limited: None,
                error: Some(err_msg),
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }

        // Monotonic 9-layer authorization reducer (`ExecutionPosture` + `RepoLaw` + Credential Guard + Session Allowlist)
        let posture = self.effective_posture_for(authority);
        if canonical != "execute_tools" {
            let eval = {
                let law = lock_unpoison(&self.constitution).clone();
                let allowed = lock_unpoison(&self.session_allowlist).clone();
                evaluate_tool_posture(posture, canonical, &prepared_val, spec.is_mutating, law.as_ref(), &allowed)
            };
            match eval.verdict {
                AuthorizationVerdict::Deny => {
                    return Self::failed_record(call, prepared_val, eval.reason, Some(true), started);
                }
                AuthorizationVerdict::AskUser => {
                    let gate = lock_unpoison(&self.approval_gate).clone();
                    let choice = if let Some(cb) = gate {
                        cb(canonical, &eval.reason, &eval.session_rule_key)
                    } else {
                        UserApprovalChoice::Deny
                    };
                    match choice {
                        UserApprovalChoice::AllowOnce => {
                            lock_unpoison(&self.turn_approvals)
                                .push(format!("{canonical}: allow_once ({})", eval.layer));
                        }
                        UserApprovalChoice::AllowSession => {
                            let mut allow = lock_unpoison(&self.session_allowlist);
                            if !allow.contains(&eval.session_rule_key) {
                                allow.push(eval.session_rule_key.clone());
                            }
                            lock_unpoison(&self.turn_approvals)
                                .push(format!("{canonical}: allow_session ({})", eval.session_rule_key));
                        }
                        UserApprovalChoice::Deny => {
                            let msg = format!(
                                "Approval required ({}) — denied: {}",
                                posture.display_label(),
                                eval.reason
                            );
                            return Self::failed_record(call, prepared_val, msg, Some(true), started);
                        }
                    }
                }
                AuthorizationVerdict::Allow => {}
            }
        }

        // Read-Before-Write & External Staleness Guard (`FileReadTracker`)
        if let Some(p_str) = prepared_args
            .get("path")
            .or_else(|| prepared_args.get("source"))
            .and_then(Value::as_str)
        {
            let resolved = resolve_workspace_path(cwd, p_str);
            if let Err(stale_err) = lock_unpoison(&self.read_tracker)
                .verify_before_mutation(canonical, &resolved, posture)
            {
                return Self::failed_record(call, prepared_val, stale_err, None, started);
            }
        }
        if canonical == "move_file" {
            if let Some(dst_str) = prepared_args.get("target").and_then(Value::as_str) {
                let resolved = resolve_workspace_path(cwd, dst_str);
                if let Err(stale_err) = lock_unpoison(&self.read_tracker)
                    .verify_before_mutation(canonical, &resolved, posture)
                {
                    return Self::failed_record(call, prepared_val, stale_err, None, started);
                }
            }
        }
        if canonical == "apply_unified_diff" {
            if let Some(diff_text) = prepared_args.get("diff").or_else(|| prepared_args.get("patch")).and_then(Value::as_str) {
                for op in crate::plumbing::fuzzy_patch::DeterministicPatchEngine::parse(diff_text) {
                    if op.op_type != "add" {
                        let resolved = resolve_workspace_path(cwd, &op.file_path);
                        if let Err(stale_err) = lock_unpoison(&self.read_tracker)
                            .verify_before_mutation(canonical, &resolved, posture)
                        {
                            return Self::failed_record(call, prepared_val, stale_err, None, started);
                        }
                    }
                }
            }
        }

        let soft_loop = authority.is_autonomous() || !spec.is_mutating;
        if let Ok(mut lb) = self.loop_breaker.lock() {
            if let Err(loop_err) = lb.record_and_check(canonical, &prepared_args, soft_loop) {
                return Self::failed_record(call, prepared_val, loop_err, None, started);
            }
        }

        let cacheable = !spec.is_mutating && ToolExecutionCache::is_cacheable(canonical);
        let cache_key = if cacheable {
            let key = ToolExecutionCache::compute_key(canonical, &prepared_args, cwd);
            if let Ok(mut c) = self.cache.lock() {
                if let Some(cached_val) = c.lookup(&key) {
                    if canonical == "view_file" {
                        if let Some(p_str) = prepared_args.get("path").and_then(Value::as_str) {
                            let full = prepared_args.get("startLine").is_none()
                                && prepared_args.get("endLine").is_none()
                                && prepared_args.get("contentOffset").is_none();
                            lock_unpoison(&self.read_tracker)
                                .record_disk_file(&resolve_workspace_path(cwd, p_str), full);
                        }
                    }
                    return ToolExecutionRecord {
                        name: call.name.clone(),
                        call_id: Some(call.id.clone()),
                        args: Some(prepared_val),
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
            .dispatch_builtin(canonical, &prepared_args, cwd, authority, posture, cancel_token)
            .await
        {
            Ok((val, denied)) => {
                if spec.is_mutating {
                    if let Ok(mut c) = self.cache.lock() {
                        c.invalidate_paths(&target_resources);
                    }
                    if !denied {
                        let mut rt = lock_unpoison(&self.read_tracker);
                        if canonical == "delete_file" {
                            for p in &target_resources {
                                rt.remove_path(p);
                            }
                        } else if canonical == "move_file" {
                            if let Some(src_str) = prepared_args.get("source").and_then(Value::as_str) {
                                rt.remove_path(&resolve_workspace_path(cwd, src_str));
                            }
                            if let Some(dst_str) = prepared_args.get("target").and_then(Value::as_str) {
                                rt.record_disk_file(&resolve_workspace_path(cwd, dst_str), true);
                            }
                        } else if matches!(
                            canonical,
                            "write_file"
                                | "replace_file_content"
                                | "multi_replace_file_content"
                                | "edit_file_anchored"
                                | "apply_unified_diff"
                        ) {
                            for p in &target_resources {
                                if p.exists() {
                                    rt.record_disk_file(p, true);
                                } else {
                                    rt.remove_path(p);
                                }
                            }
                        }
                    }
                } else {
                    if canonical == "view_file" && !denied {
                        let full = prepared_args.get("startLine").is_none()
                            && prepared_args.get("endLine").is_none()
                            && prepared_args.get("contentOffset").is_none();
                        for p in &target_resources {
                            lock_unpoison(&self.read_tracker).record_disk_file(p, full);
                        }
                    }
                    if let Some(key) = cache_key {
                        if let Ok(mut c) = self.cache.lock() {
                            c.store(key, target_resources, val.clone());
                        }
                    }
                }
                ToolExecutionRecord {
                    name: call.name.clone(),
                    call_id: Some(call.id.clone()),
                    args: Some(prepared_val),
                    output: val,
                    success: !denied,
                    denied: if denied { Some(true) } else { None },
                    output_limited: None,
                    error: if denied { Some("Permission denied for this operation".to_string()) } else { None },
                    duration_ms: started.elapsed().as_millis() as u64,
                }
            }
            Err(err_msg) => Self::failed_record(call, prepared_val, err_msg, None, started),
        }
    }

    fn failed_record(
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

    async fn dispatch_builtin(
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
                Ok((self.search_and_unlock_tools(query, limit, false), false))
            }
            "execute_tools" => {
                let calls = args.get("calls").ok_or("Missing 'calls' array")?;
                let stop_on_error = args.get("stopOnError").and_then(Value::as_bool).unwrap_or(true);
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
                let cmd_str = args.get("command").and_then(|v| v.as_str()).ok_or("Missing 'command'")?;
                SafetyPolicy::check_command(cmd_str)?;
                let advisory = SafetyPolicy::detect_heredoc_file_redirection(cmd_str)
                    .or_else(|| SafetyPolicy::detect_terminal_file_read(cmd_str));
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
                let all_turn = args.get("allInCurrentTurn").and_then(|v| v.as_bool()).unwrap_or(false);
                let mut journal = self.journal.lock().map_err(|_| "Journal lock poisoned")?;
                let res = if all_turn { journal.rollback_turn(None) } else { journal.rollback_last() };
                if let Ok(mut c) = self.cache.lock() {
                    c.clear();
                }
                if let Ok(mut rt) = self.read_tracker.lock() {
                    rt.reset_stamps();
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
            "computer_use" => self.dispatch_computer_use_tool(args),
            "browser_use" => {
                let val_args = Value::Object(args.clone());
                let mut disp = self.display_state.lock().map_err(|_| "Display lock poisoned")?;
                Ok((disp.execute_browser_action(&val_args)?, false))
            }
            "spatial_interact" => self.dispatch_spatial_tool(args),
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
                        Ok((SkillsIngestor::inspect_workspace_image(cwd, path)?, false))
                    }
                }
            }
            "skill_forge" => self.dispatch_skill_forge_tool(args, cwd),
            "engage_swarm" => {
                let objective = args.get("objective").and_then(|v| v.as_str()).unwrap_or("Execute complex objective");
                let strategy = args.get("strategy").and_then(|v| v.as_str());
                let max_spec = args.get("maxSpecialists").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
                Ok((classify_task_for_legion(objective, &[], strategy, max_spec), false))
            }
            other => {
                let on_multi = tokio::runtime::Handle::try_current()
                    .map(|h| h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread)
                    .unwrap_or(false);
                let res = if on_multi {
                    tokio::task::block_in_place(|| dispatch_fs_tool(other, args, cwd, &self.journal))?
                } else {
                    dispatch_fs_tool(other, args, cwd, &self.journal)?
                };
                if other == "batch_replace_regex" && !args.get("dryRun").and_then(|v| v.as_bool()).unwrap_or(false) {
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
        Box::pin(async move { self.execute_batch(calls, cwd, authority, cancel_token).await })
    }
}
