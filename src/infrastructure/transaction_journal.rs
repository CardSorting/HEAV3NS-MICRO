// [LAYER: INFRASTRUCTURE]
use crate::domain::types::tool_schedule::{MutationTransaction, RollbackResult};
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::text_tokens::render_unified_diff;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// Atomic Mutation Transaction Journal for instant one-shot and turn-level rollbacks + workspace diffing.
#[derive(Debug, Default)]
pub struct TransactionJournal {
    transactions: Vec<MutationTransaction>,
    current_turn_id: String,
    checkpoints: BTreeMap<String, usize>,
    seq: u64,
}

impl TransactionJournal {
    pub const fn new() -> Self {
        Self {
            transactions: Vec::new(),
            current_turn_id: String::new(),
            checkpoints: BTreeMap::new(),
            seq: 0,
        }
    }

    pub fn set_turn_id(&mut self, turn_id: impl Into<String>) {
        self.current_turn_id = turn_id.into();
    }

    pub fn current_turn_id(&self) -> &str {
        if self.current_turn_id.is_empty() {
            "turn_0"
        } else {
            &self.current_turn_id
        }
    }

    pub fn history(&self) -> &[MutationTransaction] {
        &self.transactions
    }

    pub fn record_mutation(&mut self, tool_name: &str, target_path: &Path) -> String {
        self.seq += 1;
        let id = format!("tx_{}_{}", self.seq, now_ms());
        let (previous_content, created_new_file) = match fs::metadata(target_path) {
            Ok(meta) if meta.len() <= 8 * 1024 * 1024 => match fs::read(target_path) {
                Ok(bytes) => (Some(bytes), false),
                Err(_) => (None, true),
            },
            Ok(_) => (None, false),
            Err(_) => (None, true),
        };
        let active_turn = self.current_turn_id().to_string();
        if self.transactions.len() >= 512 {
            self.transactions.remove(0);
            for idx in self.checkpoints.values_mut() {
                *idx = idx.saturating_sub(1);
            }
        }
        self.transactions.push(MutationTransaction {
            id: id.clone(),
            turn_id: active_turn,
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
            return RollbackResult {
                rolled_back_count: 0,
                restored_paths: Vec::new(),
                errors: vec!["No transactions in journal to rollback".to_string()],
            };
        };
        let (mut restored, mut errors) = (Vec::new(), Vec::new());
        Self::apply_revert(&tx, &mut restored, &mut errors);
        RollbackResult {
            rolled_back_count: 1,
            restored_paths: restored,
            errors,
        }
    }

    pub fn rollback_turn(&mut self, turn_id: Option<&str>) -> RollbackResult {
        let target_turn = turn_id.unwrap_or_else(|| self.current_turn_id()).to_string();
        let mut to_revert = Vec::new();
        let mut remaining = Vec::new();
        let mut removed_before: Vec<usize> = Vec::new();
        for (i, tx) in self.transactions.drain(..).enumerate() {
            if tx.turn_id == target_turn {
                removed_before.push(i);
                to_revert.push(tx);
            } else {
                remaining.push(tx);
            }
        }
        self.transactions = remaining;
        if !removed_before.is_empty() {
            for idx in self.checkpoints.values_mut() {
                let shift = removed_before.iter().filter(|&&r| r < *idx).count();
                *idx = idx.saturating_sub(shift);
            }
        }
        if to_revert.is_empty() {
            return RollbackResult {
                rolled_back_count: 0,
                restored_paths: Vec::new(),
                errors: vec![format!("No transactions found for turn '{target_turn}'")],
            };
        }
        let (mut restored, mut errors) = (Vec::new(), Vec::new());
        for tx in to_revert.iter().rev() {
            Self::apply_revert(tx, &mut restored, &mut errors);
        }
        RollbackResult {
            rolled_back_count: to_revert.len(),
            restored_paths: restored,
            errors,
        }
    }

    pub fn create_checkpoint(&mut self, label: &str) -> String {
        let id = format!("chk_{label}_{}", self.transactions.len());
        self.checkpoints.insert(id.clone(), self.transactions.len());
        id
    }

    pub fn restore_checkpoint(&mut self, id: &str) -> RollbackResult {
        let Some(&idx) = self.checkpoints.get(id) else {
            return RollbackResult {
                rolled_back_count: 0,
                restored_paths: Vec::new(),
                errors: vec![format!("Checkpoint '{id}' does not exist")],
            };
        };
        if self.transactions.len() <= idx {
            return RollbackResult {
                rolled_back_count: 0,
                restored_paths: Vec::new(),
                errors: Vec::new(),
            };
        }
        let to_revert: Vec<MutationTransaction> = self.transactions.drain(idx..).collect();
        self.checkpoints.retain(|_, v| *v <= idx);
        let (mut restored, mut errors) = (Vec::new(), Vec::new());
        for tx in to_revert.iter().rev() {
            Self::apply_revert(tx, &mut restored, &mut errors);
        }
        RollbackResult {
            rolled_back_count: to_revert.len(),
            restored_paths: restored,
            errors,
        }
    }

    pub fn generate_session_diff(&self, cwd: &Path, turn_id: Option<&str>) -> Value {
        let mut earliest_before: HashMap<PathBuf, Option<Vec<u8>>> = HashMap::new();
        for tx in &self.transactions {
            if let Some(tid) = turn_id {
                if tx.turn_id != tid {
                    continue;
                }
            }
            earliest_before
                .entry(tx.target_path.clone())
                .or_insert_with(|| tx.previous_content.clone());
        }
        let mut diffs = Vec::new();
        let mut files = Vec::new();
        let mut paths: Vec<PathBuf> = earliest_before.keys().cloned().collect();
        paths.sort();
        for p in paths {
            let before_bytes = earliest_before.get(&p).cloned().flatten();
            let before_str = before_bytes
                .as_deref()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
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
            match crate::infrastructure::search_tools::atomic_write_file(&tx.target_path, prev) {
                Ok(_) => restored.push(format!("[Restored] {}", tx.target_path.display())),
                Err(e) => errors.push(format!("Failed to restore {}: {e}", tx.target_path.display())),
            }
        }
    }
}
