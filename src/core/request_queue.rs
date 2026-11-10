// [LAYER: CORE]
use crate::core::session_store::{SessionCompactor, SessionStore};
use crate::domain::compaction::ContextBudgetInfo;
use crate::domain::memory::MemoryStore;
use crate::domain::prefix_and_manifest::format_append_only_context_update;
use crate::domain::prompt_spec::SystemPromptSpec;
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::runtime::{AgentConfig, ExecutionAuthorityLevel, ExecutionPosture};
use crate::domain::types::skill_types::SkillManifest;
use crate::infrastructure::vfs_store::{SkillsIngestor, WorkspacePromptProbe};
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::text_tokens::estimate_text_tokens;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone)]
pub struct QueueState {
    pub active: bool,
    pub pending_count: usize,
    pub pending_bytes: usize,
    pub max_pending: usize,
    pub max_prompt_bytes: usize,
    pub max_total_bytes: usize,
}

/// Bounded FIFO Request Queue and backpressure guard.
pub struct RuntimeRequestQueue {
    state: Mutex<QueueState>,
}

pub struct QueuePermit {
    queue: Arc<RuntimeRequestQueue>,
    bytes: usize,
}

impl Drop for QueuePermit {
    fn drop(&mut self) {
        let mut st = self.queue.state.lock().unwrap();
        st.pending_count = st.pending_count.saturating_sub(1);
        st.pending_bytes = st.pending_bytes.saturating_sub(self.bytes);
        if st.pending_count == 0 {
            st.active = false;
        }
    }
}

impl Default for RuntimeRequestQueue {
    fn default() -> Self {
        Self::new(32, 1_048_576, 8_388_608)
    }
}

impl RuntimeRequestQueue {
    pub const fn new(max_pending: usize, max_prompt_bytes: usize, max_total_bytes: usize) -> Self {
        Self {
            state: Mutex::new(QueueState {
                active: false,
                pending_count: 0,
                pending_bytes: 0,
                max_pending,
                max_prompt_bytes,
                max_total_bytes,
            }),
        }
    }

    pub fn try_acquire(self: &Arc<Self>, prompt_bytes: usize) -> Result<QueuePermit, String> {
        let mut st = self.state.lock().unwrap();
        if prompt_bytes > st.max_prompt_bytes {
            return Err(format!(
                "RUNTIME_REQUEST_TOO_LARGE: prompt is {} bytes (max {} bytes)",
                prompt_bytes, st.max_prompt_bytes
            ));
        }
        if st.pending_count >= st.max_pending || st.pending_bytes + prompt_bytes > st.max_total_bytes {
            return Err(format!(
                "RUNTIME_QUEUE_FULL: {} pending requests ({} bytes queued)",
                st.pending_count, st.pending_bytes
            ));
        }
        st.active = true;
        st.pending_count += 1;
        st.pending_bytes += prompt_bytes;
        Ok(QueuePermit {
            queue: Arc::clone(self),
            bytes: prompt_bytes,
        })
    }

    pub extern "Rust" fn snapshot(&self) -> QueueState {
        self.state.lock().unwrap().clone()
    }
}

/// Short-TTL mtime-aware cache for `SkillsIngestor::discover` so multi-round tool loops avoid redundant disk walks.
pub fn discover_skills_cached(workspace_root: &Path) -> Vec<SkillManifest> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, (u64, u128, Vec<SkillManifest>)>>> = OnceLock::new();
    let dir_mtime = std::fs::metadata(workspace_root.join(".agents").join("skills"))
        .or_else(|_| std::fs::metadata(workspace_root.join("skills")))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let now = now_ms();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some((cached_at, cached_mtime, list)) = guard.get(workspace_root) {
            if *cached_mtime == dir_mtime && now.saturating_sub(*cached_at) < 3_000 {
                return list.clone();
            }
        }
    }
    let fresh = SkillsIngestor::discover(workspace_root);
    if let Ok(mut guard) = cache.lock() {
        if guard.len() >= 32 {
            guard.clear();
        }
        guard.insert(workspace_root.to_path_buf(), (now, dir_mtime, fresh.clone()));
    }
    fresh
}

/// Composes sovereign system prompt, discovered `SKILL.md` manifests, append-only `<context_update>` envelope,
/// and token-budgeted history while keeping `Role::System` frozen at index 0 and preserving strict `user`-first alternation.
pub struct PromptComposer;

impl PromptComposer {
    pub fn compose(
        config: &AgentConfig,
        workspace_root: &Path,
        session: &SessionStore,
        memory: &MemoryStore,
        tool_defs_tokens: usize,
    ) -> (Vec<SessionMessage>, String) {
        let spec = SystemPromptSpec::for_turn(config, ExecutionAuthorityLevel::Autonomous, None);
        Self::compose_with_spec(config, workspace_root, session, memory, tool_defs_tokens, &spec)
    }

    pub fn compose_with_spec(
        config: &AgentConfig,
        workspace_root: &Path,
        session: &SessionStore,
        memory: &MemoryStore,
        tool_defs_tokens: usize,
        spec: &SystemPromptSpec,
    ) -> (Vec<SessionMessage>, String) {
        Self::compose_with_posture(
            config,
            workspace_root,
            session,
            memory,
            tool_defs_tokens,
            spec,
            ExecutionPosture::AcceptEdits,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn compose_with_posture(
        config: &AgentConfig,
        workspace_root: &Path,
        session: &SessionStore,
        memory: &MemoryStore,
        tool_defs_tokens: usize,
        spec: &SystemPromptSpec,
        posture: ExecutionPosture,
        pinned_system_override: Option<&str>,
    ) -> (Vec<SessionMessage>, String) {
        let sys_content = if let Some(pinned) = pinned_system_override {
            pinned.to_string()
        } else if config
            .system_prompt
            .contains("a specialist in The Legion working for a parent coordinator")
        {
            config.system_prompt.clone()
        } else {
            let skills = discover_skills_cached(workspace_root);
            let facts = WorkspacePromptProbe::probe_cached(workspace_root);
            spec.render(config, workspace_root, &facts, &skills)
        };

        let system_tokens = estimate_text_tokens(&sys_content);
        let budget = ContextBudgetInfo::calculate(config.context_window_tokens, config.max_output_tokens);
        let available_history_tokens = budget
            .available_input_tokens
            .saturating_sub(system_tokens + tool_defs_tokens)
            .max(512);

        let mut raw_messages = vec![SessionMessage::new(Role::System, sys_content)];
        for msg in session.messages() {
            if msg.role != Role::System {
                raw_messages.push(msg.clone());
            }
        }

        let mut composed =
            SessionCompactor::truncate_to_token_budget(&raw_messages, available_history_tokens + system_tokens);

        // Append-only volatile context delta (`LUMI-MEMORY/1` + `ExecutionPosture`) on the final User turn
        // so index 0 (`Role::System`) stays byte-pinned AND index 1 is NEVER an orphaned `Role::Assistant`!
        let mem_env = memory.format_memory_envelope();
        if let Some(ctx_block) = format_append_only_context_update(mem_env.as_deref(), posture) {
            if let Some(last_user) = composed.iter_mut().rev().find(|m| m.role == Role::User) {
                last_user.content = format!("{}\n\n{}", last_user.content, ctx_block);
            }
        }

        let summary_prompt = format!(
            "[system={}t, msgs={}, posture={}, budget={}t]",
            system_tokens,
            composed.len(),
            posture.slug(),
            available_history_tokens
        );
        (composed, summary_prompt)
    }
}
