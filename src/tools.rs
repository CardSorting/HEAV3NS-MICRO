use crate::parser::{
    canonical_tool_name, levenshtein, DeterministicSecretRedactor, ParamSchema, ParamType,
    ToolCallArgParser,
};
use crate::session::{fnv1a_32, render_unified_diff, MemoryStore, SkillsIngestor};
use crate::{now_ms, CancellationToken, ExecutionAuthorityLevel, ToolExecutionRecord};
use futures::future::join_all;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationTransaction {
    pub id: String,
    pub turn_id: String,
    pub tool_name: String,
    pub target_path: PathBuf,
    pub previous_content: Option<Vec<u8>>,
    pub created_new_file: bool,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    #[serde(rename = "rolledBackCount")]
    pub rolled_back_count: usize,
    #[serde(rename = "restoredPaths")]
    pub restored_paths: Vec<String>,
    pub errors: Vec<String>,
}

/// Atomic Mutation Transaction Journal for instant one-shot and turn-level rollbacks + workspace diffing.
#[derive(Debug, Default)]
pub struct TransactionJournal {
    transactions: Vec<MutationTransaction>,
    current_turn_id: String,
    checkpoints: HashMap<String, usize>,
    seq: u64,
}

impl TransactionJournal {
    pub fn new() -> Self {
        Self { transactions: Vec::new(), current_turn_id: "turn_0".to_string(), checkpoints: HashMap::new(), seq: 0 }
    }
    pub fn set_turn_id(&mut self, turn_id: impl Into<String>) { self.current_turn_id = turn_id.into(); }
    pub fn current_turn_id(&self) -> &str { &self.current_turn_id }
    pub fn history(&self) -> &[MutationTransaction] { &self.transactions }

    pub fn record_mutation(&mut self, tool_name: &str, target_path: &Path) -> String {
        self.seq += 1;
        let id = format!("tx_{}_{}", self.seq, now_ms());
        let (previous_content, created_new_file) = match fs::read(target_path) {
            Ok(bytes) => (Some(bytes), false),
            Err(_) => (None, true),
        };
        self.transactions.push(MutationTransaction {
            id: id.clone(),
            turn_id: self.current_turn_id.clone(),
            tool_name: tool_name.to_string(),
            target_path: target_path.to_path_buf(),
            previous_content,
            created_new_file,
            timestamp: now_ms(),
        });
        id
    }

    pub fn rollback_last(&mut self) -> RollbackResult {
        let Some(tx) = self.transactions.pop() else {
            return RollbackResult { rolled_back_count: 0, restored_paths: Vec::new(), errors: vec!["No transactions in journal to rollback".to_string()] };
        };
        let (mut restored, mut errors) = (Vec::new(), Vec::new());
        Self::apply_revert(&tx, &mut restored, &mut errors);
        RollbackResult { rolled_back_count: 1, restored_paths: restored, errors }
    }

    pub fn rollback_turn(&mut self, turn_id: Option<&str>) -> RollbackResult {
        let target_turn = turn_id.unwrap_or(&self.current_turn_id).to_string();
        let (mut to_revert, mut remaining) = (Vec::new(), Vec::new());
        for tx in self.transactions.drain(..) {
            if tx.turn_id == target_turn { to_revert.push(tx); } else { remaining.push(tx); }
        }
        self.transactions = remaining;
        if to_revert.is_empty() {
            return RollbackResult { rolled_back_count: 0, restored_paths: Vec::new(), errors: vec![format!("No transactions found for turn '{target_turn}'")] };
        }
        let (mut restored, mut errors) = (Vec::new(), Vec::new());
        for tx in to_revert.iter().rev() {
            Self::apply_revert(tx, &mut restored, &mut errors);
        }
        RollbackResult { rolled_back_count: to_revert.len(), restored_paths: restored, errors }
    }

    pub fn create_checkpoint(&mut self, label: &str) -> String {
        let id = format!("chk_{label}_{}", self.transactions.len());
        self.checkpoints.insert(id.clone(), self.transactions.len());
        id
    }

    pub fn restore_checkpoint(&mut self, id: &str) -> RollbackResult {
        let Some(&idx) = self.checkpoints.get(id) else {
            return RollbackResult { rolled_back_count: 0, restored_paths: Vec::new(), errors: vec![format!("Checkpoint '{id}' does not exist")] };
        };
        if self.transactions.len() <= idx {
            return RollbackResult { rolled_back_count: 0, restored_paths: Vec::new(), errors: Vec::new() };
        }
        let to_revert: Vec<MutationTransaction> = self.transactions.drain(idx..).collect();
        let (mut restored, mut errors) = (Vec::new(), Vec::new());
        for tx in to_revert.iter().rev() {
            Self::apply_revert(tx, &mut restored, &mut errors);
        }
        RollbackResult { rolled_back_count: to_revert.len(), restored_paths: restored, errors }
    }

    pub fn generate_session_diff(&self, cwd: &Path, turn_id: Option<&str>) -> Value {
        let mut earliest_before: HashMap<PathBuf, Option<Vec<u8>>> = HashMap::new();
        for tx in &self.transactions {
            if let Some(tid) = turn_id {
                if tx.turn_id != tid { continue; }
            }
            earliest_before.entry(tx.target_path.clone()).or_insert_with(|| tx.previous_content.clone());
        }
        let mut diffs = Vec::new();
        let mut files = Vec::new();
        let mut paths: Vec<PathBuf> = earliest_before.keys().cloned().collect();
        paths.sort();
        for p in paths {
            let before_bytes = earliest_before.get(&p).cloned().flatten();
            let before_str = before_bytes.as_deref().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
            let after_str = fs::read_to_string(&p).unwrap_or_default();
            if before_str != after_str || !p.exists() {
                let rel = p.strip_prefix(cwd).unwrap_or(&p).to_string_lossy().into_owned();
                diffs.push(render_unified_diff(&rel, &before_str, &after_str));
                files.push(rel);
            }
        }
        json!({
            "totalFilesChanged": files.len(),
            "files": files,
            "unifiedDiff": diffs.join("\n"),
        })
    }

    fn apply_revert(tx: &MutationTransaction, restored: &mut Vec<String>, errors: &mut Vec<String>) {
        if tx.created_new_file {
            if let Err(e) = fs::remove_file(&tx.target_path) {
                if tx.target_path.exists() {
                    errors.push(format!("Failed to remove {}: {e}", tx.target_path.display()));
                    return;
                }
            }
            restored.push(format!("[Deleted] {}", tx.target_path.display()));
        } else if let Some(ref prev) = tx.previous_content {
            if let Some(parent) = tx.target_path.parent() { let _ = fs::create_dir_all(parent); }
            match fs::write(&tx.target_path, prev) {
                Ok(_) => restored.push(format!("[Restored] {}", tx.target_path.display())),
                Err(e) => errors.push(format!("Failed to restore {}: {e}", tx.target_path.display())),
            }
        }
    }
}

/// Deterministic SHA-256 keyed in-memory read cache with path invalidation.
#[derive(Debug, Default)]
pub struct ToolExecutionCache {
    entries: HashMap<String, (Vec<PathBuf>, Value)>,
    pub hits: u64,
    pub misses: u64,
}

impl ToolExecutionCache {
    pub fn is_cacheable(tool_name: &str) -> bool {
        matches!(
            canonical_tool_name(tool_name),
            "view_file" | "list_dir" | "file_info" | "find_files" | "grep_search" | "directory_tree" | "search_symbols"
        )
    }

    pub fn compute_key(tool_name: &str, args: &Map<String, Value>, cwd: &Path) -> String {
        let mut hasher = Sha256::new();
        hasher.update(canonical_tool_name(tool_name).as_bytes());
        hasher.update(b":");
        hasher.update(cwd.to_string_lossy().as_bytes());
        hasher.update(b":");
        if let Ok(s) = serde_json::to_string(args) { hasher.update(s.as_bytes()); }
        format!("{:x}", hasher.finalize())
    }

    pub fn lookup(&mut self, key: &str) -> Option<Value> {
        if let Some((_, val)) = self.entries.get(key) {
            self.hits += 1;
            Some(val.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn store(&mut self, key: String, paths: Vec<PathBuf>, value: Value) {
        if self.entries.len() >= 512 { self.entries.clear(); }
        self.entries.insert(key, (paths, value));
    }

    pub fn invalidate_paths(&mut self, mutated: &[PathBuf]) {
        if mutated.is_empty() {
            self.entries.clear();
            return;
        }
        self.entries.retain(|_, (cached_paths, _)| {
            !cached_paths.is_empty()
                && !cached_paths.iter().any(|cp| mutated.iter().any(|mp| cp == mp || cp.starts_with(mp) || mp.starts_with(cp)))
        });
    }

    pub fn clear(&mut self) { self.entries.clear(); }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedOutput {
    pub output_text: String,
    pub was_truncated: bool,
    pub spill_id: Option<String>,
}

/// Clamps oversized tool outputs to head+tail previews, redacts secrets, and retains full outputs in an in-memory spill vault.
#[derive(Debug, Default)]
pub struct ToolOutputGovernor {
    spills: HashMap<String, Vec<String>>,
    pub max_chars: usize,
    pub max_lines: usize,
    seq: u64,
}

impl ToolOutputGovernor {
    pub fn new() -> Self {
        Self { spills: HashMap::new(), max_chars: 16_000, max_lines: 250, seq: 0 }
    }

    pub fn govern(&mut self, output: &Value) -> GovernedOutput {
        let raw_unredacted = match output {
            Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };
        let raw = DeterministicSecretRedactor::redact(&raw_unredacted);
        let lines: Vec<&str> = raw.lines().collect();
        if raw.len() <= self.max_chars && lines.len() <= self.max_lines {
            return GovernedOutput { output_text: raw, was_truncated: false, spill_id: None };
        }
        self.seq += 1;
        let spill_id = format!("spill_{}", self.seq);
        let owned_lines: Vec<String> = lines.iter().map(|s| (*s).to_string()).collect();
        if self.spills.len() >= 64 { self.spills.clear(); }
        self.spills.insert(spill_id.clone(), owned_lines);

        let head_count = (self.max_lines / 2).max(20).min(lines.len());
        let tail_count = (self.max_lines / 4).max(10).min(lines.len().saturating_sub(head_count));
        let mut preview = Vec::with_capacity(head_count + tail_count + 1);
        preview.extend_from_slice(&lines[..head_count]);
        preview.push("... [output truncated; call retrieve_spill_content with spillId to inspect more] ...");
        if tail_count > 0 {
            preview.extend_from_slice(&lines[lines.len() - tail_count..]);
        }
        let mut joined = preview.join("\n");
        if joined.len() > self.max_chars { joined.truncate(self.max_chars); }
        GovernedOutput { output_text: joined, was_truncated: true, spill_id: Some(spill_id) }
    }

    pub fn retrieve_spill(&self, spill_id: &str, start_line: usize, end_line: Option<usize>) -> Option<Value> {
        let lines = self.spills.get(spill_id)?;
        let total = lines.len();
        let start = start_line.max(1).min(total.max(1));
        let end = end_line.unwrap_or(start + 150).min(total);
        let slice = if start <= end && total > 0 { lines[start - 1..end].join("\n") } else { String::new() };
        Some(json!({ "found": true, "spillId": spill_id, "startLine": start, "endLine": end, "totalLines": total, "content": slice }))
    }

    pub fn clear(&mut self) { self.spills.clear(); }
}

/// Detects repetitive identical tool call loops within a turn.
#[derive(Debug)]
pub struct ToolLoopBreaker {
    recent_fingerprints: VecDeque<String>,
    pub max_identical_calls: usize,
}

impl Default for ToolLoopBreaker {
    fn default() -> Self {
        Self { recent_fingerprints: VecDeque::with_capacity(16), max_identical_calls: 3 }
    }
}

impl ToolLoopBreaker {
    pub fn reset(&mut self) { self.recent_fingerprints.clear(); }

    pub fn record_and_check(&mut self, tool_name: &str, args: &Map<String, Value>, soft_mode: bool) -> Result<(), String> {
        let fp = format!("{}:{}", canonical_tool_name(tool_name), serde_json::to_string(args).unwrap_or_default());
        if self.recent_fingerprints.len() >= 16 { self.recent_fingerprints.pop_front(); }
        self.recent_fingerprints.push_back(fp.clone());
        let identical = self.recent_fingerprints.iter().filter(|&x| x == &fp).count();
        if identical >= self.max_identical_calls && !soft_mode {
            return Err(format!("Loop breaker triggered: tool '{tool_name}' called {identical} times with identical arguments"));
        }
        Ok(())
    }
}

pub struct SafetyPolicy;

impl SafetyPolicy {
    pub fn check_command(cmd: &str) -> Result<(), String> {
        let lower = cmd.trim().to_lowercase();
        let forbidden = ["rm -rf /", "rm -rf /*", "mkfs", "dd if=/dev/zero", ":(){:|:&};:", "chmod -r 777 /"];
        for pat in forbidden {
            if lower.contains(pat) {
                return Err(format!("Safety policy blocked destructive command pattern: '{pat}'"));
            }
        }
        Ok(())
    }

    pub fn check_path(path: &Path) -> Result<(), String> {
        let s = path.to_string_lossy();
        if DeterministicSecretRedactor::is_sensitive_path(&s) {
            return Err(format!("Safety policy blocked access to sensitive credential path: '{s}'"));
        }
        Ok(())
    }
}

/// Whitespace-tolerant fuzzy patch locator (`ToolErrorAutoHealer::healFuzzyPatch`).
pub fn heal_fuzzy_patch(file_content: &str, target_snippet: &str) -> Option<(String, f64)> {
    if file_content.is_empty() || target_snippet.is_empty() { return None; }
    if file_content.contains(target_snippet) {
        return Some((target_snippet.to_string(), 1.0));
    }
    let file_lines: Vec<&str> = file_content.lines().collect();
    let target_lines: Vec<&str> = target_snippet.lines().filter(|l| !l.trim().is_empty()).collect();
    if target_lines.is_empty() || file_lines.len() < target_lines.len() { return None; }

    for i in 0..=(file_lines.len() - target_lines.len()) {
        if (0..target_lines.len()).all(|j| file_lines[i + j].trim() == target_lines[j].trim()) {
            let matched = file_lines[i..i + target_lines.len()].join("\n");
            if file_content.contains(&matched) {
                return Some((matched, 0.95));
            }
        }
    }
    None
}

/// Suggests nearby filenames in the same parent directory when a path lookup fails with ENOENT.
pub fn suggest_nearby_files(missing_path: &Path) -> Vec<String> {
    let Some(parent) = missing_path.parent() else { return Vec::new() };
    let target_name = missing_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    let Ok(rd) = fs::read_dir(parent) else { return Vec::new() };
    let mut candidates: Vec<(usize, String)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let dist = levenshtein(&name.to_lowercase(), &target_name);
            if dist <= 4 || name.to_lowercase().contains(&target_name) { Some((dist, name)) } else { None }
        })
        .collect();
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    candidates.into_iter().take(4).map(|(_, n)| n).collect()
}

// ============================================================================
// DETERMINISTIC UNIFIED DIFF & V4A MULTI-FILE PATCH ENGINE
// ============================================================================

#[derive(Debug, Clone)]
pub struct PatchHunkLine {
    pub prefix: char, // ' ', '-', '+'
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct PatchOperation {
    pub op_type: &'static str, // "add", "update", "delete"
    pub file_path: String,
    pub hunks: Vec<Vec<PatchHunkLine>>,
    pub add_content: Option<String>,
}

pub struct DeterministicPatchEngine;

impl DeterministicPatchEngine {
    pub fn parse(patch_text: &str) -> Vec<PatchOperation> {
        if patch_text.contains("*** Begin Patch") || patch_text.contains("*** Update File:") || patch_text.contains("*** Add File:") {
            Self::parse_v4a(patch_text)
        } else {
            Self::parse_unified(patch_text)
        }
    }

    pub fn parse_v4a(patch_text: &str) -> Vec<PatchOperation> {
        let mut ops = Vec::new();
        let mut cur_type: Option<&'static str> = None;
        let mut cur_path = String::new();
        let mut cur_hunks: Vec<Vec<PatchHunkLine>> = Vec::new();
        let mut cur_hunk: Vec<PatchHunkLine> = Vec::new();
        let mut add_lines: Vec<String> = Vec::new();

        let flush = |ops: &mut Vec<PatchOperation>,
                     cur_type: &mut Option<&'static str>,
                     cur_path: &mut String,
                     cur_hunks: &mut Vec<Vec<PatchHunkLine>>,
                     cur_hunk: &mut Vec<PatchHunkLine>,
                     add_lines: &mut Vec<String>| {
            if !cur_hunk.is_empty() {
                cur_hunks.push(std::mem::take(cur_hunk));
            }
            if let Some(t) = cur_type.take() {
                if !cur_path.is_empty() {
                    let add_content = if t == "add" { Some(std::mem::take(add_lines).join("\n") + "\n") } else { None };
                    ops.push(PatchOperation {
                        op_type: t,
                        file_path: std::mem::take(cur_path),
                        hunks: std::mem::take(cur_hunks),
                        add_content,
                    });
                }
            }
            cur_hunk.clear();
            cur_hunks.clear();
            add_lines.clear();
        };

        for line in patch_text.lines() {
            let trimmed = line.trim();
            if trimmed == "*** Begin Patch" || trimmed == "*** End Patch" {
                if trimmed == "*** End Patch" {
                    flush(&mut ops, &mut cur_type, &mut cur_path, &mut cur_hunks, &mut cur_hunk, &mut add_lines);
                }
                continue;
            }
            if let Some(p) = trimmed.strip_prefix("*** Update File:").or_else(|| trimmed.strip_prefix("*** File:")) {
                flush(&mut ops, &mut cur_type, &mut cur_path, &mut cur_hunks, &mut cur_hunk, &mut add_lines);
                cur_type = Some("update");
                cur_path = p.trim().to_string();
                continue;
            }
            if let Some(p) = trimmed.strip_prefix("*** Add File:") {
                flush(&mut ops, &mut cur_type, &mut cur_path, &mut cur_hunks, &mut cur_hunk, &mut add_lines);
                cur_type = Some("add");
                cur_path = p.trim().to_string();
                continue;
            }
            if let Some(p) = trimmed.strip_prefix("*** Delete File:") {
                flush(&mut ops, &mut cur_type, &mut cur_path, &mut cur_hunks, &mut cur_hunk, &mut add_lines);
                cur_type = Some("delete");
                cur_path = p.trim().to_string();
                continue;
            }
            if trimmed.starts_with("@@") {
                if !cur_hunk.is_empty() {
                    cur_hunks.push(std::mem::take(&mut cur_hunk));
                }
                continue;
            }
            match cur_type {
                Some("add") => {
                    add_lines.push(line.strip_prefix('+').unwrap_or(line).to_string());
                }
                Some("update") => {
                    if let Some(first) = line.chars().next() {
                        if matches!(first, ' ' | '-' | '+') {
                            cur_hunk.push(PatchHunkLine { prefix: first, content: line[first.len_utf8()..].to_string() });
                        }
                    }
                }
                _ => {}
            }
        }
        flush(&mut ops, &mut cur_type, &mut cur_path, &mut cur_hunks, &mut cur_hunk, &mut add_lines);
        ops
    }

    pub fn parse_unified(diff_text: &str) -> Vec<PatchOperation> {
        let mut ops = Vec::new();
        let mut old_file = String::new();
        let mut cur_file = String::new();
        let mut cur_type: &'static str = "update";
        let mut is_old_null = false;
        let mut cur_hunks: Vec<Vec<PatchHunkLine>> = Vec::new();
        let mut cur_hunk: Vec<PatchHunkLine> = Vec::new();

        let flush = |ops: &mut Vec<PatchOperation>,
                     cur_file: &mut String,
                     cur_type: &mut &'static str,
                     cur_hunks: &mut Vec<Vec<PatchHunkLine>>,
                     cur_hunk: &mut Vec<PatchHunkLine>| {
            if !cur_hunk.is_empty() {
                cur_hunks.push(std::mem::take(cur_hunk));
            }
            if !cur_file.is_empty() && (!cur_hunks.is_empty() || *cur_type == "delete") {
                let add_content = if *cur_type == "add" {
                    let lines: Vec<String> = cur_hunks.iter().flatten().filter(|l| l.prefix == '+').map(|l| l.content.clone()).collect();
                    Some(lines.join("\n") + "\n")
                } else {
                    None
                };
                ops.push(PatchOperation {
                    op_type: *cur_type,
                    file_path: std::mem::take(cur_file),
                    hunks: std::mem::take(cur_hunks),
                    add_content,
                });
            }
            cur_hunk.clear();
            cur_hunks.clear();
        };

        for line in diff_text.lines() {
            if let Some(rest) = line.strip_prefix("--- ") {
                flush(&mut ops, &mut cur_file, &mut cur_type, &mut cur_hunks, &mut cur_hunk);
                let p = rest.trim();
                is_old_null = p == "/dev/null";
                old_file = p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(p).to_string();
                continue;
            }
            if let Some(rest) = line.strip_prefix("+++ ") {
                let p = rest.trim();
                let is_new_null = p == "/dev/null";
                let new_file = p.strip_prefix("b/").or_else(|| p.strip_prefix("a/")).unwrap_or(p).to_string();
                cur_file = if is_new_null { old_file.clone() } else { new_file };
                cur_type = if is_old_null { "add" } else if is_new_null { "delete" } else { "update" };
                continue;
            }
            if line.starts_with("@@ ") {
                if !cur_hunk.is_empty() {
                    cur_hunks.push(std::mem::take(&mut cur_hunk));
                }
                continue;
            }
            if !cur_file.is_empty() {
                if let Some(first) = line.chars().next() {
                    if matches!(first, ' ' | '-' | '+') {
                        cur_hunk.push(PatchHunkLine { prefix: first, content: line[first.len_utf8()..].to_string() });
                    }
                }
            }
        }
        flush(&mut ops, &mut cur_file, &mut cur_type, &mut cur_hunks, &mut cur_hunk);
        ops
    }

    pub fn apply_hunks(original: &str, hunks: &[Vec<PatchHunkLine>]) -> Result<String, String> {
        let mut current = original.to_string();
        for (idx, hunk) in hunks.iter().enumerate() {
            let target_lines: Vec<&str> = hunk.iter().filter(|l| l.prefix != '+').map(|l| l.content.as_str()).collect();
            let replacement_lines: Vec<&str> = hunk.iter().filter(|l| l.prefix != '-').map(|l| l.content.as_str()).collect();
            let target_block = target_lines.join("\n");
            let replacement_block = replacement_lines.join("\n");
            if target_block.is_empty() {
                current.push_str(&replacement_block);
                continue;
            }
            if let Some((matched, _)) = heal_fuzzy_patch(&current, &target_block) {
                current = current.replacen(&matched, &replacement_block, 1);
            } else {
                return Err(format!("Hunk #{} failed to match in target file", idx + 1));
            }
        }
        Ok(current)
    }
}

// ============================================================================
// TOOL SPECS, DYNAMIC ROUTER & WAVE SCHEDULER
// ============================================================================

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub is_mutating: bool,
    pub is_core: bool,
    pub parameters: Vec<(&'static str, ParamSchema)>,
}

impl ToolSpec {
    pub fn to_openai_tool(&self) -> Value {
        let mut props = Map::new();
        let mut required = Vec::new();
        for &(pname, ref pschema) in &self.parameters {
            props.insert(pname.to_string(), json!({ "type": pschema.r#type.as_str(), "description": pschema.description }));
            if pschema.required { required.push(Value::String(pname.to_string())); }
        }
        json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": { "type": "object", "properties": props, "required": required }
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct ScheduledToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

/// Hyper-compressed Tool Registry, Wave Scheduler, Cache, Governor, and Native I/O Executor.
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
    pub fn new(memory_store: Arc<Mutex<MemoryStore>>) -> Self {
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
        registry.register_builtins();
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
                    || (matches!(spec.name, "list_skills" | "use_skill") && (lower.contains("skill") || lower.contains("workflow")))
                    || (matches!(spec.name, "apply_unified_diff" | "get_workspace_diff") && (lower.contains("diff") || lower.contains("patch") || lower.contains("git")))
                    || (matches!(spec.name, "search_symbols" | "batch_replace_regex") && (lower.contains("symbol") || lower.contains("outline") || lower.contains("rename") || lower.contains("refactor") || lower.contains("struct") || lower.contains("fn") || lower.contains("class")))
            })
            .map(|s| s.to_openai_tool())
            .collect()
    }

    pub fn begin_turn(&self, turn_count: u64) {
        if let Ok(mut j) = self.journal.lock() { j.set_turn_id(format!("turn_{turn_count}")); }
        if let Ok(mut lb) = self.loop_breaker.lock() { lb.reset(); }
    }

    pub fn end_task(&self) {
        if let Ok(mut c) = self.cache.lock() { c.clear(); }
        if let Ok(mut g) = self.governor.lock() { g.clear(); }
    }

    fn register_builtins(&mut self) {
        use ParamType::*;
        let builtins = [
            ToolSpec {
                name: "view_file",
                description: "Read contents of a file with optional line slicing and byte offset",
                is_mutating: false,
                is_core: true,
                parameters: vec![
                    ("path", ParamSchema::req(String, "Absolute or relative file path")),
                    ("startLine", ParamSchema::opt(Integer, "1-indexed starting line")),
                    ("endLine", ParamSchema::opt(Integer, "1-indexed ending line")),
                    ("contentOffset", ParamSchema::opt(Integer, "Byte offset to slice from")),
                ],
            },
            ToolSpec {
                name: "write_file",
                description: "Write content to a file, creating parent directories automatically",
                is_mutating: true,
                is_core: true,
                parameters: vec![
                    ("path", ParamSchema::req(String, "Target file path")),
                    ("content", ParamSchema::req(String, "Full file content")),
                ],
            },
            ToolSpec {
                name: "replace_file_content",
                description: "Replace a target block of text in a file with whitespace-tolerant fuzzy self-healing",
                is_mutating: true,
                is_core: true,
                parameters: vec![
                    ("path", ParamSchema::req(String, "Target file path")),
                    ("target", ParamSchema::req(String, "Target text to replace")),
                    ("replacement", ParamSchema::req(String, "Replacement text")),
                ],
            },
            ToolSpec {
                name: "multi_replace_file_content",
                description: "Apply multiple non-contiguous text replacements to a file atomically",
                is_mutating: true,
                is_core: true,
                parameters: vec![
                    ("path", ParamSchema::req(String, "Target file path")),
                    ("chunks", ParamSchema::req(Array, "Array of { target, replacement } objects")),
                ],
            },
            ToolSpec {
                name: "edit_file_anchored",
                description: "Apply FNV-1a hash-anchored line edit to a file",
                is_mutating: true,
                is_core: false,
                parameters: vec![
                    ("path", ParamSchema::req(String, "Target file path")),
                    ("line", ParamSchema::req(Integer, "1-indexed target line number")),
                    ("expectedAnchor", ParamSchema::req(String, "Expected 8-char hex FNV-1a hash of the target line")),
                    ("replacement", ParamSchema::req(String, "Replacement line content")),
                ],
            },
            ToolSpec {
                name: "apply_unified_diff",
                description: "Atomically apply a standard Unified Diff (--- a/, +++ b/) or V4A multi-file patch (*** Begin Patch)",
                is_mutating: true,
                is_core: true,
                parameters: vec![
                    ("diff", ParamSchema::req(String, "Unified diff or V4A patch text")),
                ],
            },
            ToolSpec {
                name: "delete_file",
                description: "Delete a file on disk with transaction journal backup",
                is_mutating: true,
                is_core: true,
                parameters: vec![("path", ParamSchema::req(String, "Target file path to delete"))],
            },
            ToolSpec {
                name: "move_file",
                description: "Move or rename a file across the workspace",
                is_mutating: true,
                is_core: false,
                parameters: vec![
                    ("source", ParamSchema::req(String, "Source file path")),
                    ("target", ParamSchema::req(String, "Destination file path")),
                ],
            },
            ToolSpec {
                name: "list_dir",
                description: "List files and directories inside a directory",
                is_mutating: false,
                is_core: true,
                parameters: vec![("path", ParamSchema::opt(String, "Directory path (defaults to workspace root)"))],
            },
            ToolSpec {
                name: "find_files",
                description: "Find files matching a substring or glob pattern across the workspace",
                is_mutating: false,
                is_core: true,
                parameters: vec![
                    ("query", ParamSchema::req(String, "File name substring or glob pattern")),
                    ("path", ParamSchema::opt(String, "Base directory to search within")),
                    ("maxDepth", ParamSchema::opt(Integer, "Maximum recursion depth (default 10)")),
                ],
            },
            ToolSpec {
                name: "file_info",
                description: "Get file metadata, size, line count, and SHA-256 digest",
                is_mutating: false,
                is_core: false,
                parameters: vec![("path", ParamSchema::req(String, "Target file path"))],
            },
            ToolSpec {
                name: "directory_tree",
                description: "Render a hierarchical tree view of a directory",
                is_mutating: false,
                is_core: false,
                parameters: vec![
                    ("path", ParamSchema::opt(String, "Root directory path")),
                    ("maxDepth", ParamSchema::opt(Integer, "Maximum depth (default 3)")),
                ],
            },
            ToolSpec {
                name: "grep_search",
                description: "Search file contents across a directory or file using regular expressions",
                is_mutating: false,
                is_core: true,
                parameters: vec![
                    ("query", ParamSchema::req(String, "Regex pattern or literal text to search for")),
                    ("path", ParamSchema::opt(String, "Directory or file path to search within")),
                    ("caseInsensitive", ParamSchema::opt(Boolean, "Case-insensitive matching (default false)")),
                    ("maxResults", ParamSchema::opt(Integer, "Maximum matches to return (default 100)")),
                ],
            },
            ToolSpec {
                name: "search_symbols",
                description: "Extract and search codebase symbols (fn, struct, enum, trait, class, interface, function, type, def) or file outline",
                is_mutating: false,
                is_core: true,
                parameters: vec![
                    ("query", ParamSchema::opt(String, "Symbol name substring filter (omit for full file/workspace outline)")),
                    ("path", ParamSchema::opt(String, "Specific file or subdirectory to inspect")),
                    ("kind", ParamSchema::opt(String, "Optional symbol kind filter: function | struct | enum | trait | class | interface | type")),
                    ("limit", ParamSchema::opt(Integer, "Maximum symbols to return (default 60)")),
                ],
            },
            ToolSpec {
                name: "batch_replace_regex",
                description: "Perform regex or symbol find-and-replace across workspace files with dry-run and rollback support",
                is_mutating: true,
                is_core: false,
                parameters: vec![
                    ("target", ParamSchema::req(String, "Regex pattern or symbol name to find")),
                    ("replacement", ParamSchema::req(String, "Replacement string")),
                    ("path", ParamSchema::opt(String, "Subdirectory or file path (default: workspace root)")),
                    ("dryRun", ParamSchema::opt(Boolean, "Preview matches without modifying files on disk")),
                ],
            },
            ToolSpec {
                name: "get_workspace_diff",
                description: "Generate unified diff of all file modifications recorded in the current turn or session",
                is_mutating: false,
                is_core: false,
                parameters: vec![("turnId", ParamSchema::opt(String, "Optional turn ID filter"))],
            },
            ToolSpec {
                name: "run_command",
                description: "Execute a shell command in the workspace with timeout and cancellation support",
                is_mutating: true,
                is_core: true,
                parameters: vec![
                    ("command", ParamSchema::req(String, "Shell command line to execute")),
                    ("cwd", ParamSchema::opt(String, "Working directory override")),
                    ("timeoutMs", ParamSchema::opt(Integer, "Execution timeout in milliseconds (default 30000)")),
                ],
            },
            ToolSpec {
                name: "rollback_last_mutation",
                description: "Atomically rollback the most recent file mutation or all mutations in the current turn",
                is_mutating: true,
                is_core: true,
                parameters: vec![("allInCurrentTurn", ParamSchema::opt(Boolean, "If true, rolls back all mutations in the current turn"))],
            },
            ToolSpec {
                name: "save_memory",
                description: "Persist a key-value fact, rule, or architectural decision into session memory",
                is_mutating: false,
                is_core: true,
                parameters: vec![
                    ("key", ParamSchema::req(String, "Unique memory key")),
                    ("value", ParamSchema::req(String, "Memory content to store")),
                    ("category", ParamSchema::opt(String, "Category: fact, rule, troubleshooting, or ki")),
                ],
            },
            ToolSpec {
                name: "search_memory",
                description: "Search persisted session memories by keyword or category",
                is_mutating: false,
                is_core: true,
                parameters: vec![("query", ParamSchema::opt(String, "Search query substring"))],
            },
            ToolSpec {
                name: "retrieve_spill_content",
                description: "Retrieve paginated line ranges from a truncated tool output spill ID",
                is_mutating: false,
                is_core: true,
                parameters: vec![
                    ("spillId", ParamSchema::req(String, "Spill ID returned by truncated tool output")),
                    ("startLine", ParamSchema::opt(Integer, "1-indexed start line")),
                    ("endLine", ParamSchema::opt(Integer, "1-indexed end line")),
                ],
            },
            ToolSpec {
                name: "list_skills",
                description: "Discover available workspace SKILL.md manifests and instructions metadata",
                is_mutating: false,
                is_core: false,
                parameters: vec![],
            },
            ToolSpec {
                name: "use_skill",
                description: "Load the full SKILL.md instructions for a named workspace skill",
                is_mutating: false,
                is_core: false,
                parameters: vec![("name", ParamSchema::req(String, "Exact skill name to activate"))],
            },
        ];
        for spec in builtins {
            self.register(spec);
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
            return if calls.is_empty() { Vec::new() } else { vec![calls.to_vec()] };
        }
        let (mut waves, mut current_wave, mut current_resources, mut current_has_mutations) =
            (Vec::new(), Vec::new(), HashSet::new(), false);

        for call in calls {
            let canonical = canonical_tool_name(&call.name);
            let is_mutating = self.get_spec(canonical).map(|s| s.is_mutating).unwrap_or(true);
            let is_global = matches!(canonical, "run_command" | "rollback_last_mutation" | "apply_unified_diff" | "batch_replace_regex");
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
        if !current_wave.is_empty() { waves.push(current_wave); }
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
            if cancel_token.map(|t| t.is_cancelled()).unwrap_or(false) { break; }
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
            let err_msg = format!("Tool '{}' argument schema validation failed: {}", canonical, validation.errors.join("; "));
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

        match self.dispatch_builtin(canonical, &prepared_args, cwd, cancel_token).await {
            Ok((val, denied)) => {
                if spec.is_mutating {
                    if let Ok(mut c) = self.cache.lock() { c.invalidate_paths(&target_resources); }
                } else if let Some(key) = cache_key {
                    if let Ok(mut c) = self.cache.lock() { c.store(key, target_resources, val.clone()); }
                }
                ToolExecutionRecord {
                    name: call.name.clone(),
                    call_id: Some(call.id.clone()),
                    args: Some(Value::Object(prepared_args)),
                    output: val,
                    success: !denied,
                    denied: if denied { Some(true) } else { None },
                    output_limited: None,
                    error: if denied { Some("Permission denied for this operation".to_string()) } else { None },
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
            "view_file" => {
                let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
                let resolved = resolve_workspace_path(cwd, raw_path);
                SafetyPolicy::check_path(&resolved)?;
                let content = fs::read_to_string(&resolved).map_err(|e| {
                    let nearby = suggest_nearby_files(&resolved);
                    if nearby.is_empty() {
                        format!("Failed to read '{}': {e}", resolved.display())
                    } else {
                        format!("Failed to read '{}': {e}. Nearby files: {:?}", resolved.display(), nearby)
                    }
                })?;
                let offset = args.get("contentOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let sliced = if offset > 0 && offset < content.len() { &content[offset..] } else { content.as_str() };
                let lines: Vec<&str> = sliced.lines().collect();
                let total_lines = lines.len();
                let start_line = args.get("startLine").and_then(|v| v.as_u64()).unwrap_or(1).max(1) as usize;
                let end_line = args.get("endLine").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(total_lines).min(total_lines);
                let out_content = if total_lines == 0 || start_line > end_line {
                    String::new()
                } else {
                    lines[start_line - 1..end_line].join("\n")
                };
                Ok((json!({ "path": resolved.to_string_lossy(), "content": out_content, "startLine": start_line, "endLine": end_line, "totalLines": total_lines }), false))
            }
            "write_file" => {
                let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let resolved = resolve_workspace_path(cwd, raw_path);
                SafetyPolicy::check_path(&resolved)?;
                if let Ok(mut j) = self.journal.lock() { j.record_mutation("write_file", &resolved); }
                if let Some(parent) = resolved.parent() {
                    fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dirs: {e}"))?;
                }
                fs::write(&resolved, content.as_bytes()).map_err(|e| format!("Failed to write '{}': {e}", resolved.display()))?;
                Ok((json!({ "success": true, "path": resolved.to_string_lossy(), "bytesWritten": content.len() }), false))
            }
            "replace_file_content" => {
                let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
                let target = args.get("target").and_then(|v| v.as_str()).ok_or("Missing 'target'")?;
                let replacement = args.get("replacement").and_then(|v| v.as_str()).unwrap_or("");
                let resolved = resolve_workspace_path(cwd, raw_path);
                SafetyPolicy::check_path(&resolved)?;
                let original = fs::read_to_string(&resolved).map_err(|e| format!("Failed to read '{}': {e}", resolved.display()))?;
                let (matched_target, confidence) = heal_fuzzy_patch(&original, target)
                    .ok_or_else(|| format!("Target content not found in '{raw_path}' (even with whitespace-tolerant fuzzy matching)"))?;
                if let Ok(mut j) = self.journal.lock() { j.record_mutation("replace_file_content", &resolved); }
                let updated = original.replacen(&matched_target, replacement, 1);
                fs::write(&resolved, updated.as_bytes()).map_err(|e| format!("Failed to write '{}': {e}", resolved.display()))?;
                Ok((json!({ "success": true, "path": resolved.to_string_lossy(), "confidence": confidence, "autoHealed": confidence < 1.0 }), false))
            }
            "multi_replace_file_content" => {
                let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
                let chunks = args.get("chunks").and_then(|v| v.as_array()).ok_or("Missing 'chunks' array")?;
                if chunks.is_empty() { return Err("No replacement chunks provided".to_string()); }
                let resolved = resolve_workspace_path(cwd, raw_path);
                SafetyPolicy::check_path(&resolved)?;
                let mut current = fs::read_to_string(&resolved).map_err(|e| format!("Failed to read '{}': {e}", resolved.display()))?;
                let mut applied = 0usize;
                for (idx, chunk) in chunks.iter().enumerate() {
                    let obj = chunk.as_object().ok_or_else(|| format!("Chunk #{idx} is not an object"))?;
                    let target = obj.get("target").or_else(|| obj.get("targetContent")).and_then(|v| v.as_str()).ok_or_else(|| format!("Chunk #{idx} missing 'target'"))?;
                    let replacement = obj.get("replacement").or_else(|| obj.get("replacementContent")).and_then(|v| v.as_str()).unwrap_or("");
                    let (matched, _) = heal_fuzzy_patch(&current, target).ok_or_else(|| format!("Chunk #{idx} target content not found in '{raw_path}'"))?;
                    current = current.replacen(&matched, replacement, 1);
                    applied += 1;
                }
                if let Ok(mut j) = self.journal.lock() { j.record_mutation("multi_replace_file_content", &resolved); }
                fs::write(&resolved, current.as_bytes()).map_err(|e| format!("Failed to write '{}': {e}", resolved.display()))?;
                Ok((json!({ "success": true, "path": resolved.to_string_lossy(), "replacementsApplied": applied }), false))
            }
            "edit_file_anchored" => {
                let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
                let line_num = args.get("line").and_then(|v| v.as_u64()).ok_or("Missing 'line'")? as usize;
                let expected_anchor = args.get("expectedAnchor").and_then(|v| v.as_str()).ok_or("Missing 'expectedAnchor'")?;
                let replacement = args.get("replacement").and_then(|v| v.as_str()).unwrap_or("");
                let resolved = resolve_workspace_path(cwd, raw_path);
                SafetyPolicy::check_path(&resolved)?;
                let original = fs::read_to_string(&resolved).map_err(|e| format!("Failed to read '{}': {e}", resolved.display()))?;
                let mut lines: Vec<String> = original.lines().map(|s| s.to_string()).collect();
                if line_num == 0 || line_num > lines.len() {
                    return Err(format!("Line {line_num} out of bounds (file has {} lines)", lines.len()));
                }
                let actual_hash = format!("{:08x}", fnv1a_32(lines[line_num - 1].trim().as_bytes()));
                if !actual_hash.eq_ignore_ascii_case(expected_anchor.trim()) {
                    return Err(format!("Anchor mismatch at line {line_num}: expected '{expected_anchor}', got '{actual_hash}'"));
                }
                if let Ok(mut j) = self.journal.lock() { j.record_mutation("edit_file_anchored", &resolved); }
                lines[line_num - 1] = replacement.to_string();
                let mut updated = lines.join("\n");
                if original.ends_with('\n') { updated.push('\n'); }
                fs::write(&resolved, updated.as_bytes()).map_err(|e| format!("Failed to write '{}': {e}", resolved.display()))?;
                Ok((json!({ "success": true, "path": resolved.to_string_lossy(), "line": line_num }), false))
            }
            "apply_unified_diff" => {
                let diff_text = args.get("diff").and_then(|v| v.as_str()).ok_or("Missing 'diff'")?;
                let ops = DeterministicPatchEngine::parse(diff_text);
                if ops.is_empty() { return Err("No valid Unified Diff or V4A patch operations found".to_string()); }
                let (mut modified, mut errors) = (Vec::new(), Vec::new());
                for op in &ops {
                    let resolved = resolve_workspace_path(cwd, &op.file_path);
                    if let Err(e) = SafetyPolicy::check_path(&resolved) {
                        errors.push(e);
                        continue;
                    }
                    match op.op_type {
                        "delete" => {
                            if let Ok(mut j) = self.journal.lock() { j.record_mutation("apply_unified_diff", &resolved); }
                            let _ = fs::remove_file(&resolved);
                            modified.push(op.file_path.clone());
                        }
                        "add" => {
                            let content = op.add_content.clone().unwrap_or_default();
                            if let Ok(mut j) = self.journal.lock() { j.record_mutation("apply_unified_diff", &resolved); }
                            if let Some(p) = resolved.parent() { let _ = fs::create_dir_all(p); }
                            match fs::write(&resolved, content.as_bytes()) {
                                Ok(_) => modified.push(op.file_path.clone()),
                                Err(e) => errors.push(format!("Failed to add '{}': {e}", op.file_path)),
                            }
                        }
                        _ => {
                            match fs::read_to_string(&resolved) {
                                Ok(orig) => match DeterministicPatchEngine::apply_hunks(&orig, &op.hunks) {
                                    Ok(updated) => {
                                        if let Ok(mut j) = self.journal.lock() { j.record_mutation("apply_unified_diff", &resolved); }
                                        match fs::write(&resolved, updated.as_bytes()) {
                                            Ok(_) => modified.push(op.file_path.clone()),
                                            Err(e) => errors.push(format!("Failed to write '{}': {e}", op.file_path)),
                                        }
                                    }
                                    Err(e) => errors.push(format!("Patch mismatch in '{}': {e}", op.file_path)),
                                },
                                Err(e) => errors.push(format!("Failed to read '{}': {e}", op.file_path)),
                            }
                        }
                    }
                }
                if !errors.is_empty() && modified.is_empty() {
                    return Err(errors.join("; "));
                }
                Ok((json!({ "success": errors.is_empty(), "modifiedFiles": modified, "errors": errors }), false))
            }
            "delete_file" => {
                let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
                let resolved = resolve_workspace_path(cwd, raw_path);
                SafetyPolicy::check_path(&resolved)?;
                if !resolved.exists() { return Err(format!("File '{}' does not exist", resolved.display())); }
                if let Ok(mut j) = self.journal.lock() { j.record_mutation("delete_file", &resolved); }
                if resolved.is_dir() {
                    fs::remove_dir_all(&resolved).map_err(|e| format!("Failed to remove directory: {e}"))?;
                } else {
                    fs::remove_file(&resolved).map_err(|e| format!("Failed to delete file: {e}"))?;
                }
                Ok((json!({ "success": true, "path": resolved.to_string_lossy() }), false))
            }
            "move_file" => {
                let src_raw = args.get("source").and_then(|v| v.as_str()).ok_or("Missing 'source'")?;
                let dst_raw = args.get("target").and_then(|v| v.as_str()).ok_or("Missing 'target'")?;
                let src = resolve_workspace_path(cwd, src_raw);
                let dst = resolve_workspace_path(cwd, dst_raw);
                SafetyPolicy::check_path(&src)?;
                SafetyPolicy::check_path(&dst)?;
                if let Some(parent) = dst.parent() { let _ = fs::create_dir_all(parent); }
                fs::rename(&src, &dst).map_err(|e| format!("Failed to move '{}' -> '{}': {e}", src.display(), dst.display()))?;
                Ok((json!({ "success": true, "source": src.to_string_lossy(), "target": dst.to_string_lossy() }), false))
            }
            "list_dir" => {
                let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let resolved = resolve_workspace_path(cwd, raw_path);
                let rd = fs::read_dir(&resolved).map_err(|e| format!("Failed to list '{}': {e}", resolved.display()))?;
                let mut entries = Vec::new();
                for entry in rd.flatten() {
                    let meta = entry.metadata().ok();
                    entries.push(json!({
                        "name": entry.file_name().to_string_lossy(),
                        "isDir": meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                        "size": meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    }));
                }
                entries.sort_by(|a, b| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")));
                Ok((json!({ "path": resolved.to_string_lossy(), "entries": entries }), false))
            }
            "find_files" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let max_depth = args.get("maxDepth").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let resolved = resolve_workspace_path(cwd, raw_path);
                let mut matches = Vec::new();
                walk_find_files(&resolved, &resolved, &query, 0, max_depth, &mut matches);
                matches.sort();
                Ok((json!({ "query": query, "count": matches.len(), "files": matches }), false))
            }
            "file_info" => {
                let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
                let resolved = resolve_workspace_path(cwd, raw_path);
                if !resolved.exists() {
                    return Ok((json!({ "exists": false, "path": resolved.to_string_lossy() }), false));
                }
                let meta = fs::metadata(&resolved).map_err(|e| format!("Stat failed: {e}"))?;
                let mut sha256_hex = None;
                let mut lines_count = None;
                if meta.is_file() && meta.len() <= 10 * 1024 * 1024 {
                    if let Ok(bytes) = fs::read(&resolved) {
                        let mut hasher = Sha256::new();
                        hasher.update(&bytes);
                        sha256_hex = Some(format!("{:x}", hasher.finalize()));
                        lines_count = Some(bytes.iter().filter(|&&b| b == b'\n').count() + 1);
                    }
                }
                Ok((json!({ "exists": true, "path": resolved.to_string_lossy(), "isDir": meta.is_dir(), "sizeBytes": meta.len(), "lines": lines_count, "sha256": sha256_hex }), false))
            }
            "directory_tree" => {
                let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let max_depth = args.get("maxDepth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                let resolved = resolve_workspace_path(cwd, raw_path);
                let mut lines = vec![resolved.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| resolved.to_string_lossy().into_owned())];
                build_directory_tree(&resolved, "", 0, max_depth, &mut lines);
                Ok((json!({ "tree": lines.join("\n") }), false))
            }
            "grep_search" => {
                let query = args.get("query").and_then(|v| v.as_str()).ok_or("Missing 'query'")?;
                let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let case_insensitive = args.get("caseInsensitive").and_then(|v| v.as_bool()).unwrap_or(false);
                let max_results = args.get("maxResults").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
                let resolved = resolve_workspace_path(cwd, raw_path);
                let hits = execute_grep_search(&resolved, query, case_insensitive, max_results)?;
                Ok((json!({ "query": query, "count": hits.len(), "matches": hits }), false))
            }
            "search_symbols" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let kind_filter = args.get("kind").and_then(|v| v.as_str()).map(|s| s.to_lowercase());
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(60) as usize;
                let resolved = resolve_workspace_path(cwd, raw_path);
                let symbols = extract_workspace_symbols(&resolved, cwd, &query, kind_filter.as_deref(), limit);
                Ok((json!({ "count": symbols.len(), "symbols": symbols }), false))
            }
            "batch_replace_regex" => {
                let find_pat = args.get("target").and_then(|v| v.as_str()).ok_or("Missing 'target'")?;
                let replace_with = args.get("replacement").and_then(|v| v.as_str()).unwrap_or("");
                let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let dry_run = args.get("dryRun").and_then(|v| v.as_bool()).unwrap_or(false);
                let re = Regex::new(find_pat).map_err(|e| format!("Invalid regex '{find_pat}': {e}"))?;
                let resolved = resolve_workspace_path(cwd, raw_path);
                let mut files = Vec::new();
                collect_source_files(&resolved, &mut files);
                let mut modified = Vec::new();
                let mut total_replacements = 0usize;
                for file in files {
                    if SafetyPolicy::check_path(&file).is_err() { continue; }
                    if let Ok(content) = fs::read_to_string(&file) {
                        let count = re.find_iter(&content).count();
                        if count > 0 {
                            total_replacements += count;
                            let rel = file.strip_prefix(cwd).unwrap_or(&file).to_string_lossy().into_owned();
                            modified.push(json!({ "path": rel, "matches": count }));
                            if !dry_run {
                                if let Ok(mut j) = self.journal.lock() { j.record_mutation("batch_replace_regex", &file); }
                                let updated = re.replace_all(&content, replace_with).into_owned();
                                let _ = fs::write(&file, updated.as_bytes());
                            }
                        }
                    }
                }
                if !dry_run && !modified.is_empty() {
                    if let Ok(mut c) = self.cache.lock() { c.clear(); }
                }
                Ok((json!({ "success": true, "dryRun": dry_run, "filesMatched": modified.len(), "totalReplacements": total_replacements, "files": modified }), false))
            }
            "get_workspace_diff" => {
                let turn_id = args.get("turnId").and_then(|v| v.as_str());
                let report = self.journal.lock().map_err(|_| "Journal lock poisoned")?.generate_session_diff(cwd, turn_id);
                Ok((report, false))
            }
            "run_command" => {
                let cmd_str = args.get("command").and_then(|v| v.as_str()).ok_or("Missing 'command'")?;
                SafetyPolicy::check_command(cmd_str)?;
                let cmd_cwd = args.get("cwd").and_then(|v| v.as_str()).map(|s| resolve_workspace_path(cwd, s)).unwrap_or_else(|| cwd.to_path_buf());
                let timeout_ms = args.get("timeoutMs").and_then(|v| v.as_u64()).unwrap_or(30_000).clamp(100, 300_000);
                execute_shell_command(cmd_str, &cmd_cwd, timeout_ms, cancel_token).await
            }
            "rollback_last_mutation" => {
                let all_turn = args.get("allInCurrentTurn").and_then(|v| v.as_bool()).unwrap_or(false);
                let mut journal = self.journal.lock().map_err(|_| "Journal lock poisoned")?;
                let res = if all_turn { journal.rollback_turn(None) } else { journal.rollback_last() };
                if let Ok(mut c) = self.cache.lock() { c.clear(); }
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
            "list_skills" => {
                let manifests = SkillsIngestor::discover(cwd);
                let list: Vec<Value> = manifests.into_iter().map(|m| json!({ "name": m.name, "description": m.description, "path": m.path })).collect();
                Ok((json!({ "count": list.len(), "skills": list }), false))
            }
            "use_skill" => {
                let name = args.get("name").and_then(|v| v.as_str()).ok_or("Missing 'name'")?.trim();
                let manifests = SkillsIngestor::discover(cwd);
                if let Some(found) = manifests.iter().find(|m| m.name.eq_ignore_ascii_case(name)) {
                    Ok((json!({ "success": true, "name": found.name, "location": found.path, "instructions": found.instructions }), false))
                } else {
                    let available: Vec<String> = manifests.into_iter().map(|m| m.name).collect();
                    Err(format!("Skill '{name}' not found. Available skills: {available:?}"))
                }
            }
            other => Err(format!("Unsupported tool '{other}'")),
        }
    }
}

pub fn resolve_workspace_path(cwd: &Path, raw: &str) -> PathBuf {
    let p = Path::new(raw.trim());
    if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) }
}

fn is_ignored_dir(name: &str) -> bool {
    matches!(name, ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | "__pycache__" | ".venv")
}

fn walk_find_files(root: &Path, current: &Path, query: &str, depth: usize, max_depth: usize, out: &mut Vec<String>) {
    if depth > max_depth || out.len() >= 500 { return; }
    let Ok(rd) = fs::read_dir(current) else { return };
    let clean_query = query.trim_matches('*');
    for entry in rd.flatten() {
        if out.len() >= 500 { break; }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            if !is_ignored_dir(&file_name) {
                walk_find_files(root, &path, query, depth + 1, max_depth, out);
            }
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned();
            if clean_query.is_empty() || file_name.to_lowercase().contains(clean_query) || rel.to_lowercase().contains(clean_query) {
                out.push(rel);
            }
        }
    }
}

fn build_directory_tree(dir: &Path, prefix: &str, depth: usize, max_depth: usize, lines: &mut Vec<String>) {
    if depth >= max_depth || lines.len() >= 400 { return; }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut filtered: Vec<_> = rd.flatten().filter(|e| !is_ignored_dir(&e.file_name().to_string_lossy())).collect();
    filtered.sort_by(|a, b| match (a.path().is_dir(), b.path().is_dir()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.file_name().cmp(&b.file_name()),
    });
    let count = filtered.len();
    for (idx, entry) in filtered.into_iter().enumerate() {
        let is_last = idx + 1 == count;
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
        let is_dir = entry.path().is_dir();
        let name = entry.file_name().to_string_lossy().into_owned();
        lines.push(format!("{prefix}{connector}{name}{}", if is_dir { "/" } else { "" }));
        if is_dir {
            build_directory_tree(&entry.path(), &child_prefix, depth + 1, max_depth, lines);
        }
    }
}

fn collect_source_files(target: &Path, out: &mut Vec<PathBuf>) {
    if out.len() >= 1_500 { return; }
    if target.is_file() {
        out.push(target.to_path_buf());
        return;
    }
    let Ok(rd) = fs::read_dir(target) else { return };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_ignored_dir(&name) || name.starts_with('.') { continue; }
        let p = entry.path();
        if p.is_dir() {
            collect_source_files(&p, out);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

fn extract_workspace_symbols(
    target: &Path,
    cwd: &Path,
    query: &str,
    kind_filter: Option<&str>,
    limit: usize,
) -> Vec<Value> {
    static SYM_RE: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let patterns = SYM_RE.get_or_init(|| {
        vec![
            ("function", Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?(?:async\s+)?(?:fn|function|def|func)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()),
            ("struct", Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()),
            ("enum", Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?(?:export\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()),
            ("trait", Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()),
            ("class", Regex::new(r"^\s*(?:export\s+)?(?:abstract\s+)?class\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()),
            ("interface", Regex::new(r"^\s*(?:export\s+)?interface\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()),
            ("type", Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?(?:export\s+)?type\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()),
        ]
    });

    let mut files = Vec::new();
    collect_source_files(target, &mut files);
    files.sort();
    let mut results = Vec::new();

    for file in files {
        let Ok(content) = fs::read_to_string(&file) else { continue };
        let rel = file.strip_prefix(cwd).unwrap_or(&file).to_string_lossy().into_owned();
        for (line_idx, line) in content.lines().enumerate() {
            for &(kind, ref re) in patterns {
                if let Some(kf) = kind_filter {
                    if kf != kind { continue; }
                }
                if let Some(caps) = re.captures(line) {
                    let sym_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    if query.is_empty() || sym_name.to_lowercase().contains(query) {
                        results.push(json!({
                            "name": sym_name,
                            "kind": kind,
                            "file": rel,
                            "line": line_idx + 1,
                            "signature": line.trim(),
                        }));
                        if results.len() >= limit { return results; }
                    }
                    break;
                }
            }
        }
    }
    results
}

fn execute_grep_search(root: &Path, query: &str, case_insensitive: bool, max_results: usize) -> Result<Vec<Value>, String> {
    let re = RegexBuilder::new(query)
        .case_insensitive(case_insensitive)
        .build()
        .or_else(|_| RegexBuilder::new(&regex::escape(query)).case_insensitive(case_insensitive).build())
        .map_err(|e| format!("Invalid regex pattern: {e}"))?;

    let mut hits = Vec::new();
    if root.is_file() {
        search_file_lines(root, root.parent().unwrap_or(root), &re, max_results, &mut hits);
    } else {
        walk_grep(root, root, &re, 0, 12, max_results, &mut hits);
    }
    Ok(hits)
}

fn walk_grep(root: &Path, current: &Path, re: &Regex, depth: usize, max_depth: usize, max_results: usize, hits: &mut Vec<Value>) {
    if depth > max_depth || hits.len() >= max_results { return; }
    let Ok(rd) = fs::read_dir(current) else { return };
    for entry in rd.flatten() {
        if hits.len() >= max_results { break; }
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            if !is_ignored_dir(&name) {
                walk_grep(root, &path, re, depth + 1, max_depth, max_results, hits);
            }
        } else if path.is_file() {
            search_file_lines(&path, root, re, max_results, hits);
        }
    }
}

fn search_file_lines(file: &Path, root: &Path, re: &Regex, max_results: usize, hits: &mut Vec<Value>) {
    let Ok(meta) = fs::metadata(file) else { return };
    if meta.len() > 2 * 1024 * 1024 { return; }
    let Ok(content) = fs::read_to_string(file) else { return };
    let rel = file.strip_prefix(root).unwrap_or(file).to_string_lossy().into_owned();
    for (idx, line) in content.lines().enumerate() {
        if hits.len() >= max_results { break; }
        if re.is_match(line) {
            hits.push(json!({
                "file": rel,
                "line": idx + 1,
                "content": if line.len() > 300 { &line[..300] } else { line },
            }));
        }
    }
}

async fn execute_shell_command(
    cmd_str: &str,
    cwd: &Path,
    timeout_ms: u64,
    cancel_token: Option<&CancellationToken>,
) -> Result<(Value, bool), String> {
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd_str)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Failed to spawn shell command: {e}"))?;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();

    let read_fut = async {
        let (mut out_buf, mut err_buf) = (Vec::new(), Vec::new());
        if let Some(ref mut out) = stdout_pipe { let _ = out.read_to_end(&mut out_buf).await; }
        if let Some(ref mut err) = stderr_pipe { let _ = err.read_to_end(&mut err_buf).await; }
        let status = child.wait().await;
        (status, out_buf, err_buf)
    };

    let timeout_dur = std::time::Duration::from_millis(timeout_ms);
    let outcome = if let Some(tok) = cancel_token {
        tokio::select! {
            _ = tok.cancelled() => return Err("Command execution cancelled by user".to_string()),
            res = tokio::time::timeout(timeout_dur, read_fut) => res,
        }
    } else {
        tokio::time::timeout(timeout_dur, read_fut).await
    };

    match outcome {
        Ok((Ok(status), out_bytes, err_bytes)) => {
            let exit_code = status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&out_bytes).into_owned();
            let stderr = String::from_utf8_lossy(&err_bytes).into_owned();
            Ok((json!({ "exitCode": exit_code, "success": status.success(), "stdout": stdout, "stderr": stderr }), false))
        }
        Ok((Err(e), _, _)) => Err(format!("Command execution failed: {e}")),
        Err(_) => Err(format!("Command timed out after {timeout_ms}ms")),
    }
}
