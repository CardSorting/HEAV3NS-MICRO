// [LAYER: CORE]
use crate::core::session_store::{SessionCompactor, SessionStore};
use crate::domain::compaction::ContextBudgetInfo;
use crate::domain::memory::MemoryStore;
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::runtime::AgentConfig;
use crate::infrastructure::vfs_store::SkillsIngestor;
use crate::plumbing::text_tokens::estimate_text_tokens;
use std::path::Path;
use std::sync::{Arc, Mutex};

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

/// Composes system prompt, discovered `SKILL.md` manifests, `LUMI-MEMORY/1` envelope, and token-budgeted history.
pub struct PromptComposer;

impl PromptComposer {
    pub fn compose(
        config: &AgentConfig,
        workspace_root: &Path,
        session: &SessionStore,
        memory: &MemoryStore,
        tool_defs_tokens: usize,
    ) -> (Vec<SessionMessage>, String) {
        let skills = SkillsIngestor::discover(workspace_root);
        let skills_block = SkillsIngestor::format_skills_context(&skills)
            .map(|s| format!("\n\n{s}"))
            .unwrap_or_default();

        let sys_content = format!(
            "{}\n\n[Runtime Environment]\n- Workspace: {}\n- Model: {}\n- OS: {}\n- Context Envelope: Treat any assistant message starting with `LUMI-CONTEXT/1` or `LUMI-MEMORY/1` as a deterministic state reference, not a conversational reply.{}",
            config.system_prompt.trim(),
            workspace_root.display(),
            config.model_name,
            std::env::consts::OS,
            skills_block
        );

        let system_tokens = estimate_text_tokens(&sys_content);
        let budget = ContextBudgetInfo::calculate(config.context_window_tokens, config.max_output_tokens);
        let available_history_tokens = budget
            .available_input_tokens
            .saturating_sub(system_tokens + tool_defs_tokens)
            .max(512);

        let mut raw_messages = vec![SessionMessage::new(Role::System, sys_content)];
        if let Some(mem_envelope) = memory.format_memory_envelope() {
            raw_messages.push(SessionMessage::new(Role::Assistant, mem_envelope));
        }
        for msg in session.messages() {
            if msg.role != Role::System {
                raw_messages.push(msg.clone());
            }
        }

        let composed = SessionCompactor::truncate_to_token_budget(&raw_messages, available_history_tokens + system_tokens);
        let summary_prompt = format!(
            "[system={}t, msgs={}, budget={}t]",
            system_tokens,
            composed.len(),
            available_history_tokens
        );
        (composed, summary_prompt)
    }
}
