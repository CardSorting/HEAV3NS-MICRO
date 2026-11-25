// [LAYER: CORE]
use crate::core::request_queue::RuntimeRequestQueue;
use crate::core::session_store::{SessionCompactor, SessionStore};
use crate::core::tool_registry::ToolRegistry;
use crate::domain::interfaces::{
    ICiGateRunner, ICodexAccountStore, ILlmProvider, IWorkspaceProfileLoader,
    IWorkspaceSnapshotCas, ProgressCallback,
};
use crate::domain::memory::{GameStateSnapshot, MemoryStore};
use crate::domain::prefix_and_manifest::PrefixStabilityTracker;
use crate::domain::profiles::{AgentProfile, CiGateReceipt};
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::progress::{
    EngineProgressEvent, EngineProgressMetadata, EngineProgressPhase, EngineProgressStatus,
};
use crate::domain::types::runtime::{
    AgentConfig, ExecutionPosture,
};
use crate::infrastructure::codex_oauth::CodexAccountStore;
use crate::infrastructure::http_provider::HttpProvider;
use crate::infrastructure::joy_ci_gate::JoyCiGateRunner;
use crate::infrastructure::vfs_store::SessionVfs;
use crate::infrastructure::workspace_cas::WorkspaceSnapshotCas;
use crate::plumbing::hash_and_math::now_ms;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::core::engine::AgentEngine;

impl AgentEngine {
    pub extern "Rust" fn new(
        workspace_root: impl Into<PathBuf>,
        config: AgentConfig,
        provider: Arc<dyn ILlmProvider>,
    ) -> Self {
        let root = workspace_root.into();
        let memory = Arc::new(Mutex::new(MemoryStore::new()));
        let tools = Arc::new(ToolRegistry::new(Arc::clone(&memory)));
        Self::with_shared_substrate(root, config, provider, memory, tools)
    }

    pub fn with_shared_substrate(
        workspace_root: PathBuf,
        config: AgentConfig,
        provider: Arc<dyn ILlmProvider>,
        memory: Arc<Mutex<MemoryStore>>,
        tools: Arc<ToolRegistry>,
    ) -> Self {
        let mut session = SessionStore::new("default", &workspace_root, None);
        session.add_message(SessionMessage::new(
            Role::System,
            config.system_prompt.clone(),
        ));
        let cas = WorkspaceSnapshotCas::open(&workspace_root);
        let initial_receipts = cas.load_recent_receipts(32);
        let custom_profiles = JoyCiGateRunner.load_workspace_profiles(&workspace_root);
        let initial_profile = config
            .profile_id
            .as_deref()
            .and_then(|id| AgentProfile::resolve_with_custom(id, &custom_profiles))
            .unwrap_or_else(AgentProfile::default_profile);
        let initial_posture = initial_profile.default_posture;
        Self {
            workspace_root,
            config: Mutex::new(config),
            session: Mutex::new(session),
            vfs: Mutex::new(SessionVfs::new()),
            memory,
            compactor: SessionCompactor::default(),
            tools,
            provider: Mutex::new(provider),
            queue: Arc::new(RuntimeRequestQueue::default()),
            snapshots: Mutex::new(Vec::new()),
            snapshot_cas_map: Mutex::new(BTreeMap::new()),
            cas: Mutex::new(cas),
            prefix_tracker: Mutex::new(PrefixStabilityTracker::default()),
            pinned_system_prompt: Mutex::new(None),
            last_prefix_status: Mutex::new("uninitialized".to_string()),
            active_posture: Mutex::new(initial_posture),
            active_profile: Mutex::new(initial_profile),
            ci_gate_runner: Arc::new(JoyCiGateRunner),
            has_custom_ci_gate: false,
            turn_receipts: Mutex::new(initial_receipts),
            frame_counter: AtomicU64::new(0),
            event_seq: AtomicU64::new(0),
            forbid_swarm_delegation: false,
        }
    }

    pub fn without_swarm_delegation(mut self) -> Self {
        self.forbid_swarm_delegation = true;
        self.tools
            .forbid_swarm
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self
    }

    pub fn forbids_swarm_delegation(&self) -> bool {
        self.forbid_swarm_delegation
    }

    pub fn with_defaults(workspace_root: impl Into<PathBuf>) -> Self {
        let config = AgentConfig::default();
        let provider = Arc::new(HttpProvider::new(
            config.provider.clone(),
            config.api_endpoint.clone(),
            config.api_key.clone(),
            config.provider_timeout_ms,
        ));
        Self::new(workspace_root, config, provider)
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Returns the active ChatGPT / OpenAI Codex OAuth account store via its domain interface.
    pub fn default_account_store() -> Arc<dyn ICodexAccountStore> {
        Arc::new(CodexAccountStore::default_store())
    }
    pub fn tools(&self) -> &Arc<ToolRegistry> {
        &self.tools
    }
    pub fn session(&self) -> &Mutex<SessionStore> {
        &self.session
    }
    pub fn vfs(&self) -> &Mutex<SessionVfs> {
        &self.vfs
    }
    pub fn memory(&self) -> &Arc<Mutex<MemoryStore>> {
        &self.memory
    }
    pub fn queue(&self) -> &Arc<RuntimeRequestQueue> {
        &self.queue
    }
    pub fn compactor(&self) -> &SessionCompactor {
        &self.compactor
    }

    pub(crate) fn provider_arc(&self) -> Arc<dyn ILlmProvider> {
        Arc::clone(&self.provider.lock().unwrap_or_else(|e| e.into_inner()))
    }

    pub fn swap_provider_and_config(
        &self,
        new_cfg: AgentConfig,
        new_provider: Arc<dyn ILlmProvider>,
    ) {
        *self.config.lock().unwrap_or_else(|e| e.into_inner()) = new_cfg;
        *self.provider.lock().unwrap_or_else(|e| e.into_inner()) = new_provider;
        *self
            .pinned_system_prompt
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        self.prefix_tracker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .declare_repin("provider_or_model_switch");
    }

    pub fn config_snapshot(&self) -> AgentConfig {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_posture(&self, posture: ExecutionPosture) {
        *self
            .active_posture
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(posture);
    }

    pub fn current_posture(&self) -> ExecutionPosture {
        self.active_posture
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unwrap_or(ExecutionPosture::AcceptEdits)
    }

    pub fn step_posture(&self) -> ExecutionPosture {
        let next = self.current_posture().step_next();
        self.set_posture(next);
        next
    }

    pub fn with_ci_gate_runner(mut self, runner: Arc<dyn ICiGateRunner>) -> Self {
        self.ci_gate_runner = runner;
        self.has_custom_ci_gate = true;
        self
    }

    pub fn active_profile(&self) -> AgentProfile {
        self.active_profile
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn list_profiles(&self) -> Vec<AgentProfile> {
        let mut profiles = AgentProfile::builtin_profiles();
        for custom in JoyCiGateRunner.load_workspace_profiles(&self.workspace_root) {
            if let Some(existing) = profiles.iter_mut().find(|p| p.id == custom.id) {
                *existing = custom;
            } else {
                profiles.push(custom);
            }
        }
        profiles
    }

    pub fn set_profile(&self, profile_id: &str) -> Result<AgentProfile, String> {
        let custom = JoyCiGateRunner.load_workspace_profiles(&self.workspace_root);
        let resolved = AgentProfile::resolve_with_custom(profile_id, &custom)
            .ok_or_else(|| format!("Unknown agent profile '{}'", profile_id))?;
        *self
            .active_profile
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = resolved.clone();
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .profile_id = Some(resolved.id.clone());
        if let Some(p) = resolved.default_posture {
            self.set_posture(p);
        }
        *self
            .pinned_system_prompt
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        self.prefix_tracker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .declare_repin("profile_switch");
        Ok(resolved)
    }

    pub fn run_ci_gate_now(&self) -> CiGateReceipt {
        let profile = self.active_profile();
        self.ci_gate_runner
            .run_ci_gate(&self.workspace_root, &profile.ci_gate)
    }

    pub(crate) fn set_active_model_name(&self, name: &str) {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .model_name = name.to_string();
        *self
            .pinned_system_prompt
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        self.prefix_tracker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .declare_repin("model_switch");
    }

    pub fn set_reasoning_effort(&self, effort: Option<String>) {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reasoning_effort = effort.filter(|s| !s.trim().is_empty());
    }

    pub fn set_fallback_providers(&self, fallbacks: Vec<String>) {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .fallback_providers = fallbacks;
    }

    pub fn snapshots_list(&self) -> Vec<GameStateSnapshot> {
        self.snapshots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub(crate) fn snapshot_count(&self) -> usize {
        self.snapshots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    pub(crate) fn snapshot_cas_map_clone(&self) -> BTreeMap<usize, String> {
        self.snapshot_cas_map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn create_snapshot(&self) -> GameStateSnapshot {
        let session = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let vfs = self.vfs.lock().unwrap_or_else(|e| e.into_inner());
        let memory = self.memory.lock().unwrap_or_else(|e| e.into_inner());
        let snap = session.create_snapshot(&vfs, &memory);
        let mut snaps = self.snapshots.lock().unwrap_or_else(|e| e.into_inner());
        let idx = snaps.len();
        snaps.push(snap.clone());
        if let Ok(cas_id) = self
            .cas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .capture_snapshot(&snap.snapshot_id, snap.frame_index)
        {
            self.snapshot_cas_map
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(idx, cas_id);
        }
        snap
    }

    pub fn rewind_to_snapshot(&self, index: usize) -> Result<GameStateSnapshot, String> {
        let snap = {
            let snaps = self.snapshots.lock().unwrap_or_else(|e| e.into_inner());
            snaps
                .get(index)
                .cloned()
                .ok_or_else(|| format!("Snapshot #{index} not found (total: {})", snaps.len()))?
        };
        if let Some(cas_id) = self
            .snapshot_cas_map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&index)
            .cloned()
        {
            self.cas
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .restore_full_snapshot(&cas_id)?;
        }
        self.tools
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.tools
            .read_tracker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reset_stamps();
        self.tools
            .journal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        let future_cas: std::collections::HashSet<String> = self
            .snapshot_cas_map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|(&k, _)| k > index)
            .map(|(_, v)| v.clone())
            .collect();
        self.snapshots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .truncate(index + 1);
        self.snapshot_cas_map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|&k, _| k <= index);
        self.turn_receipts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|r| {
                r.snapshot_after
                    .as_ref()
                    .is_none_or(|a| !future_cas.contains(a))
            });
        let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let mut vfs = self.vfs.lock().unwrap_or_else(|e| e.into_inner());
        let mut memory = self.memory.lock().unwrap_or_else(|e| e.into_inner());
        session.rewind_to_snapshot(&snap, &mut vfs, &mut memory);
        *self
            .pinned_system_prompt
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        self.prefix_tracker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .declare_repin("snapshot_rewind");
        Ok(snap)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_progress_internal(
        &self,
        cb: &Option<ProgressCallback>,
        activity_id: &str,
        phase: EngineProgressPhase,
        status: EngineProgressStatus,
        message: impl Into<String>,
        detail: Option<String>,
        started_ms: u64,
        metadata: Option<EngineProgressMetadata>,
    ) {
        if let Some(callback) = cb {
            let seq = self.event_seq.fetch_add(1, Ordering::Relaxed) + 1;
            let now = now_ms();
            callback(EngineProgressEvent {
                activity_id: activity_id.to_string(),
                phase,
                status,
                message: message.into(),
                detail,
                timestamp: now,
                elapsed_ms: Some(now.saturating_sub(started_ms)),
                sequence: seq,
                metadata,
            });
        }
    }

}
