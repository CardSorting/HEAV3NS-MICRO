// [LAYER: CORE]
use crate::domain::types::runtime::ExecutionPosture;
use crate::domain::types::skill_types::GovernedOutput;
use crate::plumbing::hash_and_math::fnv1a_32;
use crate::plumbing::secret_redactor::DeterministicSecretRedactor;
use crate::plumbing::text_tokens::{canonical_tool_name, safe_utf8_prefix};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

/// Recorded file fingerprint from `view_file` used to block blind overwrites and detect external disk edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileReadStamp {
    pub byte_len: u64,
    pub fnv1a_hash: u32,
    pub full_read: bool,
}

/// Tracks files inspected in the current session (`Codewhale FileReadTracker` staleness & read-before-write invariant).
#[derive(Debug, Default)]
pub struct FileReadTracker {
    stamps: HashMap<PathBuf, FileReadStamp>,
}

impl FileReadTracker {
    pub fn record_disk_file(&mut self, path: &Path, full_read: bool) {
        let key = normalize_tracker_key(path);
        if let Ok(bytes) = fs::read(&key) {
            let prev_full = self.stamps.get(&key).map(|s| s.full_read).unwrap_or(false);
            self.stamps.insert(
                key,
                FileReadStamp {
                    byte_len: bytes.len() as u64,
                    fnv1a_hash: fnv1a_32(&bytes),
                    full_read: full_read || prev_full,
                },
            );
        } else {
            self.stamps.remove(&key);
        }
    }

    pub fn remove_path(&mut self, path: &Path) {
        let key = normalize_tracker_key(path);
        self.stamps.remove(&key);
    }

    /// Validates that an existing file has not drifted on disk since it was last read, and in interactive
    /// postures (`Ask` / `AcceptEdits`) requires inspecting non-empty existing files before overwriting them.
    pub fn verify_before_mutation(
        &self,
        canonical_tool: &str,
        path: &Path,
        posture: ExecutionPosture,
    ) -> Result<(), String> {
        let is_file_edit = matches!(
            canonical_tool,
            "write_file" | "replace_file_content" | "multi_replace_file_content" | "edit_file_anchored"
        );
        if !is_file_edit {
            return Ok(());
        }
        let key = normalize_tracker_key(path);
        let Ok(disk_bytes) = fs::read(&key) else {
            // New file creation is always allowed
            return Ok(());
        };
        if let Some(stamp) = self.stamps.get(&key) {
            let current_hash = fnv1a_32(&disk_bytes);
            let current_len = disk_bytes.len() as u64;
            if current_hash != stamp.fnv1a_hash || current_len != stamp.byte_len {
                return Err(format!(
                    "Stale file conflict on '{}': file changed externally since last view_file (expected {:08x}/{}B, found {:08x}/{}B). Call view_file first to inspect external edits.",
                    path.display(),
                    stamp.fnv1a_hash,
                    stamp.byte_len,
                    current_hash,
                    current_len
                ));
            }
            return Ok(());
        }
        if !disk_bytes.is_empty()
            && matches!(posture, ExecutionPosture::Ask | ExecutionPosture::AcceptEdits)
        {
            return Err(format!(
                "Read-before-write guard: existing file '{}' ({} bytes) has not been read in this session. Call view_file first before modifying it.",
                path.display(),
                disk_bytes.len()
            ));
        }
        Ok(())
    }

    pub fn reset_stamps(&mut self) {
        self.stamps.clear();
    }
}

fn normalize_tracker_key(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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
            "view_file"
                | "list_dir"
                | "file_info"
                | "find_files"
                | "grep_search"
                | "directory_tree"
                | "search_symbols"
        )
    }

    pub fn compute_key(tool_name: &str, args: &Map<String, Value>, cwd: &Path) -> String {
        let mut hasher = Sha256::new();
        hasher.update(canonical_tool_name(tool_name).as_bytes());
        hasher.update(b":");
        hasher.update(cwd.to_string_lossy().as_bytes());
        hasher.update(b":");
        if let Ok(s) = serde_json::to_string(args) {
            hasher.update(s.as_bytes());
        }
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
        if self.entries.len() >= 512 {
            self.entries.clear();
        }
        self.entries.insert(key, (paths, value));
    }

    pub fn invalidate_paths(&mut self, mutated: &[PathBuf]) {
        if mutated.is_empty() {
            self.entries.clear();
            return;
        }
        self.entries.retain(|_, (cached_paths, _)| {
            !cached_paths.is_empty()
                && !cached_paths.iter().any(|cp| {
                    mutated
                        .iter()
                        .any(|mp| cp == mp || cp.starts_with(mp) || mp.starts_with(cp))
                })
        });
    }

    pub extern "Rust" fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Clamps oversized tool outputs to head+tail previews, redacts secrets, and retains full outputs in an in-memory spill vault.
#[derive(Debug, Default)]
pub struct ToolOutputGovernor {
    spills: BTreeMap<String, Vec<String>>,
    pub max_chars: usize,
    pub max_lines: usize,
    seq: u64,
}

impl ToolOutputGovernor {
    pub const fn new() -> Self {
        Self {
            spills: BTreeMap::new(),
            max_chars: 16_000,
            max_lines: 250,
            seq: 0,
        }
    }

    pub fn govern(&mut self, output: &Value) -> GovernedOutput {
        let raw_unredacted = match output {
            Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };
        let raw = DeterministicSecretRedactor::redact(&raw_unredacted);
        let lines: Vec<&str> = raw.lines().collect();
        if raw.len() <= self.max_chars && lines.len() <= self.max_lines {
            return GovernedOutput {
                output_text: raw,
                was_truncated: false,
                spill_id: None,
            };
        }
        self.seq += 1;
        let spill_id = format!("spill_{}", self.seq);
        let owned_lines: Vec<String> = lines.iter().map(|s| (*s).to_string()).collect();
        if self.spills.len() >= 64 {
            self.spills.clear();
        }
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
        if joined.len() > self.max_chars {
            joined = safe_utf8_prefix(&joined, self.max_chars).to_string();
        }
        GovernedOutput {
            output_text: joined,
            was_truncated: true,
            spill_id: Some(spill_id),
        }
    }

    pub fn retrieve_spill(&self, spill_id: &str, start_line: usize, end_line: Option<usize>) -> Option<Value> {
        let lines = self.spills.get(spill_id)?;
        let total = lines.len();
        let start = start_line.max(1).min(total.max(1));
        let end = end_line.unwrap_or(start + 150).min(total);
        let slice = if start <= end && total > 0 {
            lines[start - 1..end].join("\n")
        } else {
            String::new()
        };
        Some(json!({
            "found": true,
            "spillId": spill_id,
            "startLine": start,
            "endLine": end,
            "totalLines": total,
            "content": slice
        }))
    }

    pub extern "Rust" fn clear(&mut self) {
        self.spills.clear();
    }
}

/// Detects repetitive identical tool call loops within a turn (enforcing a hard ceiling even in autonomous/read-only mode).
#[derive(Debug)]
pub struct ToolLoopBreaker {
    recent_fingerprints: VecDeque<String>,
    pub max_identical_calls: usize,
}

impl Default for ToolLoopBreaker {
    fn default() -> Self {
        Self {
            recent_fingerprints: VecDeque::with_capacity(16),
            max_identical_calls: 3,
        }
    }
}

impl ToolLoopBreaker {
    pub extern "Rust" fn reset(&mut self) {
        self.recent_fingerprints.clear();
    }

    pub fn record_and_check(
        &mut self,
        tool_name: &str,
        args: &Map<String, Value>,
        soft_mode: bool,
    ) -> Result<(), String> {
        let fp = format!(
            "{}:{}",
            canonical_tool_name(tool_name),
            serde_json::to_string(args).unwrap_or_default()
        );
        if self.recent_fingerprints.len() >= 16 {
            self.recent_fingerprints.pop_front();
        }
        self.recent_fingerprints.push_back(fp.clone());
        let identical = self.recent_fingerprints.iter().filter(|&x| x == &fp).count();
        let consecutive_tail = self
            .recent_fingerprints
            .iter()
            .rev()
            .take_while(|&x| x == &fp)
            .count();
        let hard_ceiling = (self.max_identical_calls + 1).max(4);
        if (!soft_mode && identical >= self.max_identical_calls)
            || consecutive_tail >= hard_ceiling
            || identical >= hard_ceiling + 1
        {
            return Err(format!(
                "Loop breaker triggered: tool '{tool_name}' called {identical} times ({consecutive_tail} consecutive) with identical arguments"
            ));
        }
        Ok(())
    }
}
