// [LAYER: CORE]
use crate::core::tool_governor::{ToolExecutionCache, ToolLoopBreaker, ToolOutputGovernor};
use crate::core::tool_specs::builtin_tool_specs;
use crate::domain::interfaces::{BoxFuture, IToolExecutor};
use crate::domain::memory::MemoryStore;
use crate::domain::tool_parser::ToolCallArgParser;
use crate::domain::types::progress::ToolExecutionRecord;
use crate::domain::types::runtime::{CancellationToken, ExecutionAuthorityLevel};
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::domain::types::tool_types::ToolSpec;
use crate::infrastructure::fs_tools::dispatch_fs_tool;
use crate::infrastructure::search_tools::resolve_workspace_path;
use crate::infrastructure::shell_runner::execute_shell_command;
use crate::infrastructure::transaction_journal::TransactionJournal;
use crate::plumbing::secret_redactor::SafetyPolicy;
use crate::plumbing::text_tokens::canonical_tool_name;
use futures::future::join_all;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
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
    pub max_concurrency: usize,
}

impl ToolRegistry {
    pub extern "Rust" fn new(memory_store: Arc<Mutex<MemoryStore>>) -> Self {
        let mut registry = Self {
            specs: HashMap::new(),
            ordered_names: Vec::new(),
            parser: ToolCallArgParser::new(),
            journal: Mutex::new(TransactionJournal::new()),
            cache: Mutex::new(ToolExecutionCache::default()),
            governor: Mutex::new(ToolOutputGovernor::new()),
            loop_breaker: Mutex::new(ToolLoopBreaker::default()),
            memory_store,
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

    /// Context-aware dynamic tool router: always includes core tools and activates domain tools matching prompt intent.
    pub fn select_relevant_tool_schemas(&self, prompt: &str) -> Vec<Value> {
        let lower = prompt.to_lowercase();
        self.list_specs()
            .into_iter()
            .filter(|spec| {
                spec.is_core
                    || lower.is_empty()
                    || lower.contains(spec.name)
                    || (matches!(spec.name, "list_skills" | "use_skill")
                        && (lower.contains("skill") || lower.contains("workflow")))
                    || (matches!(spec.name, "apply_unified_diff" | "get_workspace_diff")
                        && (lower.contains("diff") || lower.contains("patch") || lower.contains("git")))
                    || (matches!(spec.name, "search_symbols" | "batch_replace_regex")
                        && (lower.contains("symbol")
                            || lower.contains("outline")
                            || lower.contains("rename")
                            || lower.contains("refactor")
                            || lower.contains("struct")
                            || lower.contains("fn")
                            || lower.contains("class")))
            })
            .map(|s| s.to_openai_tool())
            .collect()
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
