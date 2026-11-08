use crate::{now_ms, Role, SessionMessage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_ARENA_CAPACITY_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
pub const CONTEXT_CHECKPOINT_PREFIX: &str = "LUMI-CONTEXT/1";
pub const MEMORY_ENVELOPE_PREFIX: &str = "LUMI-MEMORY/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlabBufferSnapshot {
    #[serde(rename = "capacityBytes")]
    pub capacity_bytes: usize,
    #[serde(rename = "offsetWords")]
    pub offset_words: usize,
    #[serde(rename = "allocatedBytes")]
    pub allocated_bytes: usize,
    #[serde(rename = "resetCount")]
    pub reset_count: u64,
}

/// Zero-GC Contiguous Slab Allocator for mechanical sympathy and O(1) state rewind.
#[derive(Debug, Clone)]
pub struct ArenaAllocator {
    buffer: Vec<u8>,
    offset_words: usize,
    reset_count: u64,
}

impl Default for ArenaAllocator {
    fn default() -> Self {
        Self::new(DEFAULT_ARENA_CAPACITY_BYTES)
    }
}

impl ArenaAllocator {
    pub fn new(capacity_bytes: usize) -> Self {
        let aligned_cap = (capacity_bytes.max(64) + 3) & !3;
        Self { buffer: vec![0u8; aligned_cap], offset_words: 0, reset_count: 0 }
    }
    #[inline] pub fn capacity_words(&self) -> usize { self.buffer.len() / 4 }
    #[inline] pub fn capacity_bytes(&self) -> usize { self.buffer.len() }
    #[inline] pub fn allocated_bytes(&self) -> usize { self.offset_words * 4 }
    #[inline] pub fn offset_words(&self) -> usize { self.offset_words }
    #[inline] pub fn reset_count(&self) -> u64 { self.reset_count }

    pub fn allocate_node(&mut self, id: u32, flags: u32) -> Result<usize, String> {
        let ptr = self.offset_words;
        if ptr + 2 > self.capacity_words() {
            return Err(format!("ArenaAllocator slab exhausted (capacity: {} bytes)", self.capacity_bytes()));
        }
        let byte_offset = ptr * 4;
        self.buffer[byte_offset..byte_offset + 4].copy_from_slice(&id.to_le_bytes());
        self.buffer[byte_offset + 4..byte_offset + 8].copy_from_slice(&flags.to_le_bytes());
        self.offset_words += 2;
        Ok(ptr)
    }

    pub fn allocate_raw_bytes(&mut self, byte_len: usize) -> Result<usize, String> {
        let byte_offset = self.offset_words * 4;
        let words_needed = (byte_len + 3) / 4;
        if self.offset_words + words_needed > self.capacity_words() {
            return Err("ArenaAllocator slab exhausted for raw bytes allocation".to_string());
        }
        self.offset_words += words_needed;
        Ok(byte_offset)
    }

    pub fn allocate_str(&mut self, s: &str) -> Result<(usize, usize), String> {
        let bytes = s.as_bytes();
        let offset = self.allocate_raw_bytes(bytes.len())?;
        self.buffer[offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok((offset, bytes.len()))
    }

    pub fn read_str(&self, byte_offset: usize, byte_len: usize) -> Result<&str, String> {
        if byte_offset + byte_len > self.buffer.len() {
            return Err(format!("Out of bounds read_str: offset {byte_offset}, len {byte_len}"));
        }
        std::str::from_utf8(&self.buffer[byte_offset..byte_offset + byte_len])
            .map_err(|e| format!("Invalid UTF-8 in arena slab: {e}"))
    }

    pub fn set_offset(&mut self, offset_words: usize) -> Result<(), String> {
        if offset_words > self.capacity_words() {
            return Err(format!("Invalid offset_words {offset_words}"));
        }
        self.offset_words = offset_words;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.offset_words = 0;
        self.reset_count += 1;
    }

    pub fn snapshot(&self) -> SlabBufferSnapshot {
        SlabBufferSnapshot {
            capacity_bytes: self.capacity_bytes(),
            offset_words: self.offset_words,
            allocated_bytes: self.allocated_bytes(),
            reset_count: self.reset_count,
        }
    }
}

/// Fast FNV-1a 32-bit hash for write deduplication & line anchoring.
#[inline]
pub fn fnv1a_32(data: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

#[inline]
pub fn estimate_text_tokens(text: &str) -> usize {
    if text.is_empty() { 0 } else { (text.len() + 3) / 4 }
}

#[inline]
pub fn estimate_messages_tokens(messages: &[SessionMessage]) -> usize {
    messages.iter().map(|m| 4 + estimate_text_tokens(&m.content)).sum()
}

pub fn truncate_text_to_token_budget(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(4);
    if text.len() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 16 {
        return text.chars().take(max_chars).collect();
    }
    let mut end = (max_chars - 15).min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...[truncated]", &text[..end])
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ContextBudgetInfo {
    pub context_window_tokens: usize,
    pub reserved_output_tokens: usize,
    pub safety_margin_tokens: usize,
    pub available_input_tokens: usize,
    pub compaction_trigger_tokens: usize,
    pub target_input_tokens: usize,
}

impl ContextBudgetInfo {
    pub fn calculate(context_window_tokens: usize, requested_output_tokens: usize) -> Self {
        let window = context_window_tokens.max(4_096);
        let reserved_out = requested_output_tokens.clamp(256, window / 2);
        let safety = (window / 20).clamp(256, 4_096);
        let available_in = window.saturating_sub(reserved_out + safety).max(1_024);
        Self {
            context_window_tokens: window,
            reserved_output_tokens: reserved_out,
            safety_margin_tokens: safety,
            available_input_tokens: available_in,
            compaction_trigger_tokens: (available_in * 80) / 100,
            target_input_tokens: (available_in * 60) / 100,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompactionPolicy {
    pub max_messages: Option<usize>,
    pub max_input_tokens: Option<usize>,
    pub trigger_input_tokens: Option<usize>,
    pub target_input_tokens: Option<usize>,
    pub reserved_tokens: usize,
    pub preserve_recent_turns: usize,
    pub summary_max_tokens: usize,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionReport {
    pub compacted: bool,
    pub input_message_count: usize,
    pub output_message_count: usize,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub summarized_message_count: usize,
    pub checkpoint_id: Option<String>,
}

/// Hybrid deterministic context compactor generating `LUMI-CONTEXT/1` rolling checkpoints.
#[derive(Debug, Clone)]
pub struct SessionCompactor {
    pub max_turn_history: usize,
    pub preserve_recent_turns: usize,
    pub summary_max_tokens: usize,
}

impl Default for SessionCompactor {
    fn default() -> Self {
        Self { max_turn_history: 40, preserve_recent_turns: 4, summary_max_tokens: 2_048 }
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
        let trigger_tokens = policy.trigger_input_tokens.or(policy.max_input_tokens).unwrap_or(usize::MAX);

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

        let retained_checkpoints: Vec<SessionMessage> = source_transcript.iter().filter(|m| Self::is_checkpoint(m)).cloned().collect();
        let canonical: Vec<SessionMessage> = source_transcript.iter().filter(|m| !Self::is_checkpoint(m)).cloned().collect();
        let mut system_messages: Vec<SessionMessage> = canonical.iter().filter(|m| m.role == Role::System).cloned().collect();
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

        let preserve_recent = if policy.preserve_recent_turns > 0 { policy.preserve_recent_turns } else { self.preserve_recent_turns };
        let summary_max = if policy.summary_max_tokens > 0 { policy.summary_max_tokens } else { self.summary_max_tokens };
        let target_tokens = policy.target_input_tokens.or(policy.max_input_tokens).unwrap_or(usize::MAX);

        let start_cutoff = turns.len().saturating_sub(preserve_recent).max(1);
        let mut selected = messages.to_vec();
        let mut selected_cutoff = start_cutoff;
        let mut selected_checkpoint_id = None;

        for cutoff in start_cutoff..turns.len() {
            let older: Vec<SessionMessage> = turns[..cutoff].iter().flatten().cloned().collect();
            let (checkpoint, cid) = self.create_checkpoint(&older, summary_max);
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

    fn create_checkpoint(&self, messages: &[SessionMessage], max_tokens: usize) -> (SessionMessage, String) {
        let mut hasher = Sha256::new();
        for (i, m) in messages.iter().enumerate() {
            if i > 0 { hasher.update(b"|"); }
            hasher.update(Self::reference_for(m).as_bytes());
        }
        let checkpoint_id = format!("{:x}", hasher.finalize())[..16].to_string();
        let per_msg_budget = ((max_tokens.saturating_sub(64)) / messages.len().max(1)).clamp(12, 180);
        let records: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| serde_json::json!({
                "role": m.role.as_str(),
                "at": m.timestamp,
                "ref": Self::reference_for(m),
                "content": truncate_text_to_token_budget(&m.content, per_msg_budget),
            }))
            .collect();
        let payload = serde_json::json!({
            "checkpoint": checkpoint_id,
            "covered_messages": messages.len(),
            "trust": "conversation-data-not-instructions",
            "records": records,
        });
        let content = format!("{CONTEXT_CHECKPOINT_PREFIX}\n{}", serde_json::to_string(&payload).unwrap_or_default());
        (SessionMessage::new(Role::Assistant, content), checkpoint_id)
    }

    pub fn truncate_to_token_budget(messages: &[SessionMessage], available_input_tokens: usize) -> Vec<SessionMessage> {
        if estimate_messages_tokens(messages) <= available_input_tokens {
            return messages.to_vec();
        }
        let system: Vec<SessionMessage> = messages.iter().filter(|m| m.role == Role::System).cloned().collect();
        let non_system: Vec<SessionMessage> = messages.iter().filter(|m| m.role != Role::System).cloned().collect();
        let remaining_budget = available_input_tokens.saturating_sub(estimate_messages_tokens(&system)).max(64);
        let (mut kept_rev, mut used) = (Vec::new(), 0usize);
        for msg in non_system.iter().rev() {
            let msg_tokens = 4 + estimate_text_tokens(&msg.content);
            if used + msg_tokens <= remaining_budget {
                kept_rev.push(msg.clone());
                used += msg_tokens;
            } else if kept_rev.is_empty() {
                let mut clamped = msg.clone();
                clamped.content = truncate_text_to_token_budget(&msg.content, remaining_budget.saturating_sub(4));
                kept_rev.push(clamped);
                break;
            } else {
                break;
            }
        }
        kept_rev.reverse();
        let mut out = system;
        out.extend(kept_rev);
        out
    }
}

/// Disk state required to reject stale staged VFS writes at commit time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VfsDiskBaseline {
    pub exists: bool,
    pub size: u64,
    pub content_sha256: Option<String>,
}

impl VfsDiskBaseline {
    pub fn capture(path: &Path) -> Self {
        if let Ok(bytes) = fs::read(path) {
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            Self { exists: true, size: bytes.len() as u64, content_sha256: Some(format!("{:x}", hasher.finalize())) }
        } else {
            Self { exists: false, size: 0, content_sha256: None }
        }
    }

    pub fn verify(&self, path: &Path) -> Result<(), String> {
        let current = Self::capture(path);
        if self.exists != current.exists || self.content_sha256 != current.content_sha256 {
            return Err(format!("Staged file changed on disk since review: {}. The staged change was preserved.", path.display()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsFileOverlay {
    pub path: String,
    pub content: String,
    #[serde(rename = "isDeleted")]
    pub is_deleted: bool,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none", rename = "diskBaseline")]
    pub disk_baseline: Option<VfsDiskBaseline>,
}

/// Staged Virtual File System Overlay for reviewable changesets and atomic commits.
#[derive(Debug, Clone, Default)]
pub struct SessionVfs {
    staged: HashMap<String, VfsFileOverlay>,
}

impl SessionVfs {
    pub fn new() -> Self { Self::default() }
    fn normalize_key(path: &str) -> String { Path::new(path).to_string_lossy().into_owned() }

    pub fn stage_write(&mut self, path: &str, content: impl Into<String>, baseline: Option<VfsDiskBaseline>) {
        let key = Self::normalize_key(path);
        self.staged.insert(key.clone(), VfsFileOverlay { path: key, content: content.into(), is_deleted: false, timestamp: now_ms(), disk_baseline: baseline });
    }

    pub fn stage_delete(&mut self, path: &str, baseline: Option<VfsDiskBaseline>) {
        let key = Self::normalize_key(path);
        self.staged.insert(key.clone(), VfsFileOverlay { path: key, content: String::new(), is_deleted: true, timestamp: now_ms(), disk_baseline: baseline });
    }

    pub fn get_file(&self, path: &str) -> Option<&VfsFileOverlay> { self.staged.get(&Self::normalize_key(path)) }
    pub fn has_staged(&self, path: &str) -> bool { self.staged.contains_key(&Self::normalize_key(path)) }
    pub fn unstage(&mut self, path: &str) -> bool { self.staged.remove(&Self::normalize_key(path)).is_some() }
    pub fn clear(&mut self) { self.staged.clear(); }

    pub fn export_staged(&self) -> Vec<VfsFileOverlay> {
        let mut list: Vec<VfsFileOverlay> = self.staged.values().cloned().collect();
        list.sort_by(|a, b| a.path.cmp(&b.path));
        list
    }

    pub fn generate_diff(&self, path: &str) -> Option<String> {
        let key = Self::normalize_key(path);
        let overlay = self.staged.get(&key)?;
        let original = fs::read_to_string(&key).unwrap_or_default();
        let updated = if overlay.is_deleted { "" } else { &overlay.content };
        Some(render_unified_diff(&key, &original, updated))
    }

    pub fn commit_all(&mut self) -> Result<Vec<String>, String> {
        let overlays = self.export_staged();
        for ov in &overlays {
            if let Some(ref baseline) = ov.disk_baseline {
                baseline.verify(Path::new(&ov.path))?;
            }
        }
        let mut committed = Vec::with_capacity(overlays.len());
        for ov in overlays {
            let target = Path::new(&ov.path);
            if ov.is_deleted {
                let _ = fs::remove_file(target);
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir {}: {e}", parent.display()))?;
                }
                fs::write(target, &ov.content).map_err(|e| format!("Failed to write {}: {e}", target.display()))?;
            }
            committed.push(ov.path);
        }
        self.staged.clear();
        Ok(committed)
    }
}

pub fn render_unified_diff(path: &str, old_text: &str, new_text: &str) -> String {
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    if old_text == new_text {
        return out;
    }
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    out.push_str(&format!("@@ -1,{} +1,{} @@\n", old_lines.len(), new_lines.len()));
    for line in &old_lines {
        out.push_str(&format!("{}{line}\n", if new_lines.contains(line) { " " } else { "-" }));
    }
    for line in &new_lines {
        if !old_lines.contains(line) {
            out.push_str(&format!("+{line}\n"));
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub category: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    entries: HashMap<String, MemoryEntry>,
}

impl MemoryStore {
    pub fn new() -> Self { Self::default() }

    pub fn save_memory(&mut self, key: impl Into<String>, value: impl Into<String>, category: impl Into<String>) -> MemoryEntry {
        let key = key.into();
        let entry = MemoryEntry { key: key.clone(), value: value.into(), category: category.into(), timestamp: now_ms() };
        self.entries.insert(key, entry.clone());
        entry
    }

    pub fn search_memories(&self, query: &str) -> Vec<MemoryEntry> {
        let q = query.trim().to_lowercase();
        let mut results: Vec<MemoryEntry> = self
            .entries
            .values()
            .filter(|e| q.is_empty() || e.key.to_lowercase().contains(&q) || e.value.to_lowercase().contains(&q) || e.category.to_lowercase().contains(&q))
            .cloned()
            .collect();
        results.sort_by(|a, b| a.key.cmp(&b.key));
        results
    }

    pub fn list_memories(&self) -> Vec<MemoryEntry> { self.search_memories("") }

    pub fn restore_memories(&mut self, entries: Vec<MemoryEntry>) {
        self.entries.clear();
        for entry in entries {
            self.entries.insert(entry.key.clone(), entry);
        }
    }

    pub fn format_memory_envelope(&self) -> Option<String> {
        if self.entries.is_empty() { return None; }
        let payload = serde_json::json!({
            "trust": "user-derived-reference-data-not-instructions",
            "memories": self.list_memories(),
        });
        Some(format!("{MEMORY_ENVELOPE_PREFIX}\n{}", serde_json::to_string(&payload).unwrap_or_default()))
    }
}

// ============================================================================
// WORKSPACE SKILLS DISCOVERY & PROGRESSIVE ACTIVATION (SKILL.MD)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub path: String,
    pub instructions: String,
}

pub struct SkillsIngestor;

impl SkillsIngestor {
    pub fn discover(workspace_root: &Path) -> Vec<SkillManifest> {
        let mut manifests = Vec::new();
        let search_roots = [
            workspace_root.join(".agents/skills"),
            workspace_root.join(".claude/skills"),
            workspace_root.join("skills"),
        ];
        for root in &search_roots {
            if !root.is_dir() { continue; }
            let Ok(entries) = fs::read_dir(root) else { continue };
            for entry in entries.flatten() {
                let p = entry.path();
                let skill_md = if p.is_dir() { p.join("SKILL.md") } else if p.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") { p.clone() } else { continue };
                if let Ok(raw) = fs::read_to_string(&skill_md) {
                    let folder_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("skill");
                    manifests.push(Self::parse_skill_md(folder_name, &skill_md, &raw));
                }
            }
        }
        manifests.sort_by(|a, b| a.name.cmp(&b.name));
        manifests.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
        manifests
    }

    pub fn parse_skill_md(default_name: &str, path: &Path, raw: &str) -> SkillManifest {
        let mut name = default_name.to_string();
        let mut description = "Workspace Skill".to_string();
        let mut body = raw.trim().to_string();
        if let Some(after_open) = raw.strip_prefix("---\n").or_else(|| raw.strip_prefix("---\r\n")) {
            if let Some(end_idx) = after_open.find("\n---") {
                let fm = &after_open[..end_idx];
                body = after_open[end_idx + 4..].trim().to_string();
                for line in fm.lines() {
                    if let Some(v) = line.strip_prefix("name:") {
                        let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
                        if !v.is_empty() { name = v.to_string(); }
                    } else if let Some(v) = line.strip_prefix("description:") {
                        let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
                        if !v.is_empty() { description = v.to_string(); }
                    }
                }
            }
        }
        SkillManifest { name, description, path: path.to_string_lossy().into_owned(), instructions: body }
    }

    pub fn format_skills_context(manifests: &[SkillManifest]) -> Option<String> {
        if manifests.is_empty() { return None; }
        let items: Vec<serde_json::Value> = manifests
            .iter()
            .take(64)
            .map(|m| serde_json::json!({ "name": m.name, "description": m.description }))
            .collect();
        Some(serde_json::json!({ "availableSkills": items }).to_string())
    }
}

/// Computes a fast deterministic SHA-256 fingerprint of workspace source files and manifests.
pub fn workspace_revision(workspace_root: &Path) -> String {
    let mut files: Vec<(String, String)> = Vec::new();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        if out.len() >= 2_000 { return; }
        let Ok(rd) = fs::read_dir(dir) else { return };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || matches!(name.as_str(), "node_modules" | "target" | "dist" | "build" | "vendor" | "coverage") {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if p.is_file() {
                if let Ok(bytes) = fs::read(&p) {
                    if bytes.len() <= 512 * 1024 {
                        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().into_owned();
                        let mut h = Sha256::new();
                        h.update(&bytes);
                        out.push((rel, format!("{:x}", h.finalize())));
                    }
                }
            }
        }
    }
    walk(workspace_root, workspace_root, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut root_hasher = Sha256::new();
    for (rel, digest) in files {
        root_hasher.update(rel.as_bytes());
        root_hasher.update(b":");
        root_hasher.update(digest.as_bytes());
        root_hasher.update(b"\n");
    }
    format!("{:x}", root_hasher.finalize())[..16].to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateSnapshot {
    #[serde(rename = "snapshotId")]
    pub snapshot_id: String,
    #[serde(rename = "frameIndex")]
    pub frame_index: u64,
    pub timestamp: u64,
    pub messages: Vec<SessionMessage>,
    pub transcript: Vec<SessionMessage>,
    #[serde(rename = "stagedFiles")]
    pub staged_files: Vec<VfsFileOverlay>,
    pub memories: Vec<MemoryEntry>,
    #[serde(rename = "slabSnapshot")]
    pub slab_snapshot: SlabBufferSnapshot,
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
    pub fn new(session_id: impl Into<String>, cwd: impl Into<PathBuf>, arena_capacity: Option<usize>) -> Self {
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

    #[inline] pub fn messages(&self) -> &[SessionMessage] { &self.messages }
    #[inline] pub fn transcript(&self) -> &[SessionMessage] { &self.transcript }
    #[inline] pub fn context_generation(&self) -> u64 { self.context_generation }
    #[inline] pub fn slab_snapshot(&self) -> SlabBufferSnapshot { self.arena.snapshot() }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.transcript.clear();
        self.transcript_index.clear();
        self.context_generation += 1;
        self.arena.reset();
    }

    pub fn truncate_active_messages(&mut self, count: usize) {
        let keep = self.messages.len().saturating_sub(count);
        self.messages.truncate(keep);
        self.rebuild_arena();
    }

    pub fn compact(&mut self, compactor: &SessionCompactor, policy: &CompactionPolicy) -> CompactionReport {
        let (compacted, report) = compactor.compact(&self.messages, policy, &self.transcript);
        if report.compacted {
            self.messages = compacted;
            self.context_generation += 1;
            self.rebuild_arena();
        }
        report
    }

    pub fn export_jsonl(&self) -> String {
        self.transcript.iter().filter_map(|m| serde_json::to_string(m).ok()).collect::<Vec<_>>().join("\n")
    }

    pub fn import_jsonl(&mut self, jsonl: &str) -> Result<(), String> {
        let (mut parsed, mut required_bytes) = (Vec::new(), 0usize);
        for (idx, line) in jsonl.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, payload.as_bytes()).map_err(|e| e.to_string())?;
        self.last_saved_hash = Some(hash);
        Ok(true)
    }

    pub fn load_from_file(&mut self, path: &Path) -> Result<(), String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
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

    pub fn rewind_to_snapshot(&mut self, snapshot: &GameStateSnapshot, vfs: &mut SessionVfs, memory: &mut MemoryStore) {
        self.turn_count = snapshot.frame_index;
        self.messages = snapshot.messages.clone();
        self.transcript = snapshot.transcript.clone();
        self.rebuild_transcript_index();
        self.context_generation += 1;
        let _ = self.arena.set_offset(snapshot.slab_snapshot.offset_words);
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
            self.transcript_index.insert(SessionCompactor::reference_for(msg), msg.clone());
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
