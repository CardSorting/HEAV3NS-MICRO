// [LAYER: CORE]
use crate::domain::arena::{ArenaAllocator, SlabBufferSnapshot, DEFAULT_ARENA_CAPACITY_BYTES};
use crate::domain::compaction::{CompactionPolicy, CompactionReport, CONTEXT_CHECKPOINT_PREFIX};
use crate::domain::interfaces::ISessionPersistence;
use crate::domain::memory::{GameStateSnapshot, MemoryStore};
use crate::domain::types::messages::{estimate_messages_tokens, Role, SessionMessage};
use crate::infrastructure::vfs_store::{FileSessionPersistence, SessionVfs};
use crate::plumbing::hash_and_math::{fnv1a_32, now_ms};
use crate::plumbing::text_tokens::{estimate_text_tokens, truncate_text_to_token_budget};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Hybrid deterministic context compactor generating `LUMI-CONTEXT/1` rolling checkpoints.
#[derive(Debug, Clone)]
pub struct SessionCompactor {
    pub max_turn_history: usize,
    pub preserve_recent_turns: usize,
    pub summary_max_tokens: usize,
}

impl Default for SessionCompactor {
    fn default() -> Self {
        Self {
            max_turn_history: 40,
            preserve_recent_turns: 4,
            summary_max_tokens: 2_048,
        }
    }
}

impl SessionCompactor {
    pub fn reference_for(msg: &SessionMessage) -> String {
        let mut hasher = Sha256::new();
        hasher.update(msg.role.as_str().as_bytes());
        hasher.update(b":");
        hasher.update(msg.timestamp.to_string().as_bytes());
        hasher.update(b":");
        hasher.update(msg.content.as_bytes());
        let hex = format!("{:x}", hasher.finalize());
        format!("msg_{}", &hex[..12])
    }

    #[inline]
    pub fn is_checkpoint(msg: &SessionMessage) -> bool {
        msg.role == Role::Assistant && msg.content.starts_with(CONTEXT_CHECKPOINT_PREFIX)
    }

    pub fn group_turns(conversation: &[SessionMessage]) -> Vec<Vec<SessionMessage>> {
        let mut turns: Vec<Vec<SessionMessage>> = Vec::new();
        for msg in conversation {
            if msg.role == Role::User || turns.is_empty() {
                turns.push(vec![msg.clone()]);
            } else if let Some(last) = turns.last_mut() {
                last.push(msg.clone());
            }
        }
        turns
    }

    pub fn compact(
        &self,
        messages: &[SessionMessage],
        policy: &CompactionPolicy,
        source_transcript: &[SessionMessage],
    ) -> (Vec<SessionMessage>, CompactionReport) {
        let input_tokens = policy.reserved_tokens + estimate_messages_tokens(messages);
        let max_messages = policy.max_messages.unwrap_or(self.max_turn_history).max(2);
        let trigger_tokens = policy
            .trigger_input_tokens
            .or(policy.max_input_tokens)
            .unwrap_or(usize::MAX);

        if !policy.force && messages.len() <= max_messages && input_tokens <= trigger_tokens {
            return (
                messages.to_vec(),
                CompactionReport {
                    compacted: false,
                    input_message_count: messages.len(),
                    output_message_count: messages.len(),
                    input_tokens,
                    output_tokens: input_tokens,
                    summarized_message_count: 0,
                    checkpoint_id: None,
                },
            );
        }

        let retained_checkpoints: Vec<SessionMessage> = source_transcript
            .iter()
            .filter(|m| Self::is_checkpoint(m))
            .cloned()
            .collect();
        let canonical: Vec<SessionMessage> = source_transcript
            .iter()
            .filter(|m| !Self::is_checkpoint(m))
            .cloned()
            .collect();
        let mut system_messages: Vec<SessionMessage> = canonical
            .iter()
            .filter(|m| m.role == Role::System)
            .cloned()
            .collect();
        system_messages.extend(retained_checkpoints);

        let conversation: Vec<SessionMessage> = canonical.into_iter().filter(|m| m.role != Role::System).collect();
        let turns = Self::group_turns(&conversation);
        if turns.len() <= 1 {
            return (
                messages.to_vec(),
                CompactionReport {
                    compacted: false,
                    input_message_count: messages.len(),
                    output_message_count: messages.len(),
                    input_tokens,
                    output_tokens: input_tokens,
                    summarized_message_count: 0,
                    checkpoint_id: None,
                },
            );
        }

        let preserve_recent = if policy.preserve_recent_turns > 0 {
            policy.preserve_recent_turns
        } else {
            self.preserve_recent_turns
        };
        let summary_max = if policy.summary_max_tokens > 0 {
            policy.summary_max_tokens
        } else {
            self.summary_max_tokens
        };
        let target_tokens = policy
            .target_input_tokens
            .or(policy.max_input_tokens)
            .unwrap_or(usize::MAX);

        let start_cutoff = turns.len().saturating_sub(preserve_recent).max(1);
        let mut selected = messages.to_vec();
        let mut selected_cutoff = start_cutoff;
        let mut selected_checkpoint_id = None;

        for cutoff in start_cutoff..turns.len() {
            let older: Vec<SessionMessage> = turns[..cutoff].iter().flatten().cloned().collect();
            let (checkpoint, cid) = self.build_checkpoint(&older, summary_max);
            let mut candidate = system_messages.clone();
            candidate.push(checkpoint);
            candidate.extend(turns[cutoff..].iter().flatten().cloned());
            let cand_tokens = policy.reserved_tokens + estimate_messages_tokens(&candidate);
            selected = candidate;
            selected_cutoff = cutoff;
            selected_checkpoint_id = Some(cid);
            if selected.len() <= max_messages && cand_tokens <= target_tokens {
                break;
            }
        }

        let summarized_count: usize = turns[..selected_cutoff].iter().map(|t| t.len()).sum();
        let output_tokens = policy.reserved_tokens + estimate_messages_tokens(&selected);
        let out_len = selected.len();
        (
            selected,
            CompactionReport {
                compacted: true,
                input_message_count: messages.len(),
                output_message_count: out_len,
                input_tokens,
                output_tokens,
                summarized_message_count: summarized_count,
                checkpoint_id: selected_checkpoint_id,
            },
        )
    }

    fn build_checkpoint(&self, messages: &[SessionMessage], max_tokens: usize) -> (SessionMessage, String) {
        let mut hasher = Sha256::new();
        for (i, m) in messages.iter().enumerate() {
            if i > 0 {
                hasher.update(b"|");
            }
            hasher.update(Self::reference_for(m).as_bytes());
        }
        let checkpoint_id = format!("{:x}", hasher.finalize())[..16].to_string();
        let per_msg_budget = ((max_tokens.saturating_sub(64)) / messages.len().max(1)).clamp(12, 180);
        let records: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role.as_str(),
                    "at": m.timestamp,
                    "ref": Self::reference_for(m),
                    "content": truncate_text_to_token_budget(&m.content, per_msg_budget),
                })
            })
            .collect();
        let payload = serde_json::json!({
            "checkpoint": checkpoint_id,
            "covered_messages": messages.len(),
            "trust": "conversation-data-not-instructions",
            "records": records,
        });
        let content = format!(
            "{CONTEXT_CHECKPOINT_PREFIX}\n{}",
            serde_json::to_string(&payload).unwrap_or_default()
        );
        (SessionMessage::new(Role::Assistant, content), checkpoint_id)
    }

    pub fn truncate_to_token_budget(
        messages: &[SessionMessage],
        available_input_tokens: usize,
    ) -> Vec<SessionMessage> {
        if estimate_messages_tokens(messages) <= available_input_tokens {
            return messages.to_vec();
        }
        let system: Vec<SessionMessage> = messages
            .iter()
            .filter(|m| m.role == Role::System)
            .cloned()
            .collect();
        let non_system: Vec<SessionMessage> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .cloned()
            .collect();
        let remaining_budget = available_input_tokens
            .saturating_sub(estimate_messages_tokens(&system))
            .max(64);
        let (mut kept_rev, mut used) = (Vec::new(), 0usize);
        for msg in non_system.iter().rev() {
            let tc_tokens: usize = msg
                .tool_calls
                .as_ref()
                .map(|tcs| {
                    tcs.iter()
                        .map(|tc| 8 + estimate_text_tokens(&tc.function.name) + estimate_text_tokens(&tc.function.arguments))
                        .sum()
                })
                .unwrap_or(0);
            let msg_tokens = 4 + estimate_text_tokens(&msg.content) + tc_tokens;
            if used + msg_tokens <= remaining_budget {
                kept_rev.push(msg.clone());
                used += msg_tokens;
            } else if kept_rev.is_empty() && msg.role != Role::Tool {
                let mut clamped = msg.clone();
                clamped.content = truncate_text_to_token_budget(&msg.content, remaining_budget.saturating_sub(4));
                kept_rev.push(clamped);
                break;
            } else {
                break;
            }
        }
        kept_rev.reverse();
        while kept_rev.first().is_some_and(|m| m.role == Role::Tool) {
            kept_rev.remove(0);
        }
        let mut out = system;
        out.extend(kept_rev);
        out
    }
}

/// Dual-track Session Store with contiguous Arena slab, JSONL persistence, and FNV-1a write coalescing.
#[derive(Debug, Clone)]
pub struct SessionStore {
    pub session_id: String,
    pub cwd: PathBuf,
    pub turn_count: u64,
    messages: Vec<SessionMessage>,
    transcript: Vec<SessionMessage>,
    transcript_index: HashMap<String, SessionMessage>,
    arena: ArenaAllocator,
    context_generation: u64,
    last_saved_hash: Option<u32>,
}

impl SessionStore {
    pub extern "Rust" fn new(
        session_id: impl Into<String>,
        cwd: impl Into<PathBuf>,
        arena_capacity: Option<usize>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            cwd: cwd.into(),
            turn_count: 0,
            messages: Vec::new(),
            transcript: Vec::new(),
            transcript_index: HashMap::new(),
            arena: ArenaAllocator::new(arena_capacity.unwrap_or(DEFAULT_ARENA_CAPACITY_BYTES)),
            context_generation: 0,
            last_saved_hash: None,
        }
    }

    pub fn increment_turn(&mut self) -> u64 {
        self.turn_count += 1;
        self.turn_count
    }

    pub fn add_message(&mut self, msg: SessionMessage) -> &SessionMessage {
        if !msg.content.is_empty() {
            let _ = self.arena.allocate_str(&msg.content);
        }
        let reference = SessionCompactor::reference_for(&msg);
        self.transcript_index.insert(reference, msg.clone());
        self.transcript.push(msg.clone());
        self.messages.push(msg);
        self.messages.last().unwrap()
    }

    #[inline]
    pub fn messages(&self) -> &[SessionMessage] {
        &self.messages
    }

    #[inline]
    pub fn transcript(&self) -> &[SessionMessage] {
        &self.transcript
    }

    #[inline]
    pub const fn context_generation(&self) -> u64 {
        self.context_generation
    }

    #[inline]
    pub fn slab_snapshot(&self) -> SlabBufferSnapshot {
        self.arena.snapshot()
    }

    pub extern "Rust" fn clear(&mut self) {
        self.messages.clear();
        self.transcript.clear();
        self.transcript_index.clear();
        self.context_generation += 1;
        self.arena.reset();
    }

    pub fn truncate_active_messages(&mut self, count: usize) {
        let keep = self.messages.len().saturating_sub(count);
        self.messages.truncate(keep);
        let t_keep = self.transcript.len().saturating_sub(count);
        self.transcript.truncate(t_keep);
        self.rebuild_transcript_index();
        self.rebuild_arena();
    }

    pub fn compact(&mut self, compactor: &SessionCompactor, policy: &CompactionPolicy) -> CompactionReport {
        self.compact_session(compactor, policy)
    }

    pub fn compact_session(&mut self, compactor: &SessionCompactor, policy: &CompactionPolicy) -> CompactionReport {
        let (compacted, report) = compactor.compact(&self.messages, policy, &self.transcript);
        if report.compacted {
            self.messages = compacted;
            self.context_generation += 1;
            self.rebuild_arena();
        }
        report
    }

    pub fn export_jsonl(&self) -> String {
        self.transcript
            .iter()
            .filter_map(|m| serde_json::to_string(m).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn import_jsonl(&mut self, jsonl: &str) -> Result<(), String> {
        let (mut parsed, mut required_bytes) = (Vec::new(), 0usize);
        for (idx, line) in jsonl.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let msg: SessionMessage = serde_json::from_str(trimmed)
                .map_err(|e| format!("Invalid session JSONL syntax at line {}: {e}", idx + 1))?;
            required_bytes += (msg.content.len() + 3) & !3;
            parsed.push(msg);
        }
        if required_bytes > self.arena.capacity_bytes() {
            return Err("CONTEXT_LIMIT: saved messages exceed session capacity; existing context was kept".to_string());
        }
        self.messages = parsed.clone();
        self.transcript = parsed;
        self.rebuild_transcript_index();
        self.context_generation += 1;
        self.rebuild_arena();
        Ok(())
    }

    pub fn save_to_file(&mut self, path: &Path) -> Result<bool, String> {
        let payload = self.export_jsonl();
        let hash = fnv1a_32(payload.as_bytes());
        if self.last_saved_hash == Some(hash) && path.exists() {
            return Ok(false);
        }
        let adapter = FileSessionPersistence;
        adapter.save_jsonl(path, &payload)?;
        self.last_saved_hash = Some(hash);
        Ok(true)
    }

    pub fn load_from_file(&mut self, path: &Path) -> Result<(), String> {
        let adapter = FileSessionPersistence;
        let content = adapter.load_jsonl(path)?;
        self.import_jsonl(&content)
    }

    pub fn create_snapshot(&self, vfs: &SessionVfs, memory: &MemoryStore) -> GameStateSnapshot {
        GameStateSnapshot {
            snapshot_id: format!("snapshot-frame-{}-{}", self.turn_count, now_ms()),
            frame_index: self.turn_count,
            timestamp: now_ms(),
            messages: self.messages.clone(),
            transcript: self.transcript.clone(),
            staged_files: vfs.export_staged(),
            memories: memory.list_memories(),
            slab_snapshot: self.arena.snapshot(),
        }
    }

    pub fn rewind_to_snapshot(
        &mut self,
        snapshot: &GameStateSnapshot,
        vfs: &mut SessionVfs,
        memory: &mut MemoryStore,
    ) {
        self.turn_count = snapshot.frame_index;
        self.messages = snapshot.messages.clone();
        self.transcript = snapshot.transcript.clone();
        self.rebuild_transcript_index();
        self.context_generation += 1;
        self.rebuild_arena();
        vfs.clear();
        for file in &snapshot.staged_files {
            if file.is_deleted {
                vfs.stage_delete(&file.path, file.disk_baseline.clone());
            } else {
                vfs.stage_write(&file.path, file.content.clone(), file.disk_baseline.clone());
            }
        }
        memory.restore_memories(snapshot.memories.clone());
    }

    fn rebuild_transcript_index(&mut self) {
        self.transcript_index.clear();
        for msg in &self.transcript {
            self.transcript_index
                .insert(SessionCompactor::reference_for(msg), msg.clone());
        }
    }

    fn rebuild_arena(&mut self) {
        self.arena.reset();
        for msg in &self.messages {
            if !msg.content.is_empty() {
                let _ = self.arena.allocate_str(&msg.content);
            }
        }
    }
}
