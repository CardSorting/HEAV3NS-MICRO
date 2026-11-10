<!-- [LAYER: UI] -->
# Public Rust API Reference

<p align="center">
  <a href="README.md">API Hub</a> ·
  <a href="cli.md">CLI Reference</a> ·
  <a href="errors.md">Errors</a> ·
  <a href="../architecture/current.md">Architecture</a>
</p>

Comprehensive reference of the public types, contracts (`64` formal `I<Name>` traits, `22` port interfaces), and programmatic interfaces exposed by the `heav3ns-micro` crate across all Seven Sovereign Pillars.

---

## 1. Domain Port Interfaces (`src/domain/interfaces/mod.rs`)

All core interactions with infrastructure adapters and spatial/cognitive subsystems are decoupled through formal `I<Name>` traits under the Principle of Contractual Sovereignty (`ADR-0007`, `ADR-0011`, `ADR-0014`, `ADR-0015`):

### Provider, Model Registry, Persistence, CAS, VFS & Journal Contracts
```rust
pub trait ILlmProvider: Send + Sync {
    fn complete<'a>(&'a self, request: &'a ProviderRequest, on_progress: Option<&'a ProgressCallback>, cancel_token: Option<&'a CancellationToken>) -> BoxFuture<'a, Result<ProviderResponse, ProviderError>>;
}
pub trait IHttpProvider: ILlmProvider {}
pub trait IScriptedProvider: ILlmProvider { fn remaining_steps(&self) -> usize; }
pub trait IModelPricingRegistry: Send + Sync {
    fn get_catalog(&self) -> ModelPricingCatalog;
    fn ensure_fresh_background(&self);
    fn refresh_now<'a>(&'a self) -> BoxFuture<'a, Result<ModelPricingCatalog, String>>;
}
pub trait ISessionPersistence: Send + Sync {
    fn save_jsonl(&self, path: &Path, payload: &str) -> Result<bool, String>;
    fn load_jsonl(&self, path: &Path) -> Result<String, String>;
}
pub trait IFileSessionPersistence: ISessionPersistence {}
pub trait IWorkspaceSnapshotCas: Send + Sync {
    fn capture_pre_turn_tree(&self, session_id: &str, turn_index: u64) -> Result<String, String>;
    fn capture_post_turn_tree(&self, session_id: &str, turn_index: u64) -> Result<String, String>;
    fn compute_turn_diff(&self, pre_tree_hash: &str, post_tree_hash: &str) -> Result<Vec<CasFileDiff>, String>;
    fn undo_last_turn(&self, session_id: &str) -> Result<UndoReceipt, UndoError>;
    fn restore_snapshot(&self, tree_hash: &str) -> Result<usize, String>;
}
pub trait ISessionVfs: Send + Sync {
    fn export_staged_overlays(&self) -> Vec<VfsFileOverlay>;
    fn commit_staged(&mut self) -> Result<Vec<String>, String>;
}
pub trait ISkillsIngestor: Send + Sync {
    fn discover_manifests(workspace_root: &Path) -> Vec<SkillManifest>;
}
pub trait ITransactionJournal: Send + Sync {
    fn record_file_mutation(&mut self, tool_name: &str, target_path: &Path) -> String;
    fn rollback_last_mutation(&mut self) -> RollbackResult;
}
pub trait IWorkspacePromptProbe: Send + Sync {
    fn probe(workspace_root: &Path) -> WorkspacePromptFacts;
}
pub trait IToolExecutor: Send + Sync {
    fn execute_batch_contract<'a>(&'a self, calls: &'a [ScheduledToolCall], cwd: &'a Path, authority: ExecutionAuthorityLevel, cancel_token: Option<&'a CancellationToken>) -> BoxFuture<'a, Vec<ToolExecutionRecord>>;
}
```

### Spatial Action, `SPIDER`, `BROCCOLI` & `SYNTHETIC SWARM` Contracts
```rust
pub trait ISpatialMediator: Send + Sync {
    fn ground_contract(&self, contract: &SpatialActionContract) -> SpatialGroundingResult;
}
pub trait ISpatialGroundingGate: Send + Sync {
    fn ground(&self, contract: &SpatialActionContract) -> SpatialGroundingResult;
}
pub trait IExecutionAssuranceGate: Send + Sync {
    fn assure_pre(&self, grounded: &GroundedSpatialTarget, contract: &SpatialActionContract) -> Result<(), SpatialError>;
    fn verify_post(&mut self, action: &str, grounded: Option<&GroundedSpatialTarget>, before: &SpatialScene, after: &SpatialScene, duration_ms: u64) -> SpatialExecutionReceipt;
}
pub trait IEnvironmentalObserver: Send + Sync {
    fn observe_environment(&self) -> EnvironmentalObservation;
    fn detect_modal_trap(&self, obs: &EnvironmentalObservation) -> Option<String>;
}
pub trait IBehavioralKnowledgeStore: Send + Sync {
    fn lookup_pattern(&self, app: &str, objective: &str) -> Option<LearnedInteractionPattern>;
    fn record_outcome(&mut self, pattern_id: &str, app: &str, success: bool, epoch: u64);
    fn reconcile_state(&self, checkpoint_epoch: u64, current_epoch: u64, external_hash_changed: bool) -> EnvironmentalReconciliationReport;
}
pub trait IBehavioralStore: IBehavioralKnowledgeStore {}
pub trait ISwarmIntelligenceConsultant: Send + Sync {
    fn consult(&self, request: &SwarmConsultationRequest) -> SwarmConsultationResponse;
}
pub trait IComputerUseOrchestrator: Send + Sync {
    fn execute_closed_loop(&self, goal: &str, contract: &SpatialActionContract) -> Result<serde_json::Value, String>;
}
```

---

## 2. Core Engines & Orchestrators (`src/core/`)

### `AgentEngine` (`src/core/engine.rs`)
The central multi-turn ReAct state machine orchestrating agent turns, context compaction, sticky tool dispatch, and streaming progress events.
- `AgentEngine::with_defaults(workspace_root)`
- `AgentEngine::tick(&self, input: EngineTickInput) -> EngineTickResult`
- `AgentEngine::ask_side_question(&self, question: &str) -> Result<String, ProviderError>`
- `AgentEngine::without_swarm_delegation(&self) -> AgentEngine` (blocks recursive Legion delegation)
- `AgentEngine::inspect_context_snapshot(&self, current_prompt: &str) -> ContextInspectionSnapshot`
- `AgentEngine::render_context_hud(&self, arg: &str) -> String`
- `AgentEngine::render_prompt_inspector(&self, arg: &str) -> String`
- `AgentEngine::render_request_manifest(&self, arg: &str) -> String`
- `AgentEngine::render_tools_status(&self, filter: &str) -> String`
- `AgentEngine::undo_last_turn(&self) -> Result<UndoReceipt, UndoError>`
- `AgentEngine::refresh_model_catalog(&self) -> Result<ModelPricingCatalog, String>`
- `AgentEngine::handle_slash_command(&self, input: &str) -> Option<String>` (`35` command arms)
- `AgentEngine::run_swarm(&self, goal: &str) -> SwarmFormationResult`
- `AgentEngine::create_snapshot(&self) -> GameStateSnapshot` & `rewind_to_snapshot(&self, index: usize)`

### `ComputerUseOrchestrator` (`src/core/computer_use_orchestrator.rs`)
The 9-stage closed-loop embodied agency orchestrator (`OBSERVE -> ORIENT -> INTERPRET -> REASON -> PLAN -> ACT -> OBSERVE -> VERIFY -> LEARN`).
- `ComputerUseOrchestrator::observe(&self) -> EnvironmentalObservation`
- `ComputerUseOrchestrator::interpret(&self, obs: &EnvironmentalObservation) -> Value`
- `ComputerUseOrchestrator::consult(&self, req: &SwarmConsultationRequest) -> SwarmConsultationResponse`
- `ComputerUseOrchestrator::execute_closed_loop_cycle(&self, goal: &str, contract: &SpatialActionContract) -> Result<Value, String>`

### `SpatialInteractionEngine` (`src/core/spatial_engine.rs`)
- `SpatialInteractionEngine::execute_spatially_grounded_action(&self, contract: &SpatialActionContract) -> Result<Value, String>` (includes automatic off-screen pre-scroll remediation)
- `SpatialInteractionEngine::execute_plan(&self, contract: &SpatialActionContract) -> Result<Value, String>` (executes multi-step `SpatialActionPlan` sequences)
- `SpatialInteractionEngine::reconcile_dom_and_visual(&self, dom_elements: &[Value]) -> SpatialScene`
- `SpatialInteractionEngine::query_directional(&self, origin_id: &str, direction: &str) -> Option<SpatialTarget>`
- `SpatialInteractionEngine::audit_health(&self) -> Value`

---

## 3. UI & IPC Dispatch (`src/ui/`)

- **`run_stdio_worker` (`src/ui/stdio_worker.rs`)**: Runs the multiplexed `"v": 1` JSON-RPC 2.0 loop over standard I/O with `"prompt-inspect-v1"` negotiation, background task execution, and non-blocking handler routing.
- **`RpcDispatcher` (`src/ui/rpc_dispatch.rs`)**: Routes desktop requests (`files.*`, `changes.read`, `spider.check`, `memory.*`, `context.inspect`, `prompt.inspect`, `task.*`, `posture.*`, `approval.resolve`, `cas.*`, `receipts.*`, `request.preview`, `models.*`, `swarm.*`, `checkpoint.*`, `spatial.*`, `cua.*`).
