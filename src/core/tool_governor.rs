// [LAYER: CORE]
use crate::domain::types::skill_types::GovernedOutput;
use crate::plumbing::secret_redactor::DeterministicSecretRedactor;
use crate::plumbing::text_tokens::canonical_tool_name;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};

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
            joined.truncate(self.max_chars);
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

/// Detects repetitive identical tool call loops within a turn.
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
        if identical >= self.max_identical_calls && !soft_mode {
            return Err(format!(
                "Loop breaker triggered: tool '{tool_name}' called {identical} times with identical arguments"
            ));
        }
        Ok(())
    }
}
