<!-- [LAYER: UI] -->
# Project Map & Architecture (`HEAV3NS-MICRO`)

This map reflects the complete layout and organization of the **HEAV3NS-MICRO** workspace (`/Users/bozoegg/Desktop/HEAV3NS-MICRO`), encompassing the 82-module Rust sovereign intelligence runtime and the AppKit Native Desktop IDE client (`HEAV3NS-CODE`).

## Workspace Overview

| Directory / File | Architectural Zone | Sovereign Pillars & Role |
| :--- | :--- | :--- |
| **`doc.md` & `ROADMAP.md`** | `UI` | Foundational charter of **HEAV3NS — The Sovereign Intelligence Architecture** (`THE ASCENDANT COMPUTATIONAL INTELLIGENCE`) and multi-horizon engineering roadmap |
| **`src/plumbing/`** (`12` modules) | `PLUMBING` | Stateless leaf primitives (`0%` state, `0` upward imports): `hash_and_math.rs`, `text_tokens.rs`, `json_repair.rs`, `secret_redactor.rs`, `seatbelt_profile.rs`, `shell_guard.rs`, `fuzzy_patch.rs`, `vision_and_dom.rs`, `spatial_geometry.rs`, `environmental_diff.rs`, `display_telemetry.rs`, `mod.rs` |
| **`src/domain/`** (`14` modules) | `DOMAIN` | Pure domain models & state machines (`0%` I/O, `<= 3` structs/file): `prompt_spec.rs`, `pricing_and_taxonomy.rs`, `prefix_and_manifest.rs`, `turn_receipt.rs`, `arena.rs` (`REWIND`), `memory.rs` (`BROCCOLI`), `compaction.rs`, `swarm.rs` (`SYNTHETIC SWARM`), `tool_parser.rs`, `computer_use.rs`, `skill_tree.rs`, `spatial_action.rs` & `spatial_mediator.rs` (`COMPUTER USE`), `mod.rs` |
| **`src/domain/interfaces/`** (`1` module) | `DOMAIN` | `22` formal Dependency Inversion port traits (`mod.rs`): `ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal`, `IWorkspaceSnapshotCas`, `IModelPricingRegistry`, `ISpatialMediator`, `ISpatialGroundingGate`, `IExecutionAssuranceGate`, `IEnvironmentalObserver`, `IBehavioralKnowledgeStore`, `IBehavioralStore`, `ISwarmIntelligenceConsultant`, `IEnvironmentalDiffer`, `ISpatialGeometry`, `IComputerUseOrchestrator`, `IWorkspacePromptProbe` |
| **`src/domain/types/`** (`16` modules) | `DOMAIN` | Pure domain value & behavioral types (`model_catalog.rs`, `posture_types.rs` [`ExecutionPosture`, `RepoConstitution`], `messages.rs`, `runtime.rs`, `progress.rs`, `tick_result.rs`, `vfs_types.rs`, `tool_types.rs`, `tool_schedule.rs`, `skill_types.rs`, `computer_use_types.rs` [`UiElement`, `VirtualWindow`], `spatial_types.rs`, `environmental_types.rs`, `swarm_intelligence_types.rs`, `learned_knowledge_types.rs`, `mod.rs`) |
| **`src/infrastructure/`** (`11` modules) | `INFRASTRUCTURE` | Concrete I/O adapters implementing `domain/interfaces`: `http_provider.rs`, `pricing_updater.rs` (`ModelPricingRegistry`), `scripted_provider.rs`, `vfs_store.rs` (`SessionVfs`, `SkillsIngestor`, `WorkspacePromptProbe`, `FileSessionPersistence`), `workspace_cas.rs` (`WorkspaceSnapshotCas`), `transaction_journal.rs` (`TransactionJournal`), `behavioral_store.rs` (`BehavioralKnowledgeStore`), `shell_runner.rs`, `fs_tools.rs`, `search_tools.rs`, `mod.rs` |
| **`src/core/`** (`20` modules) | `CORE` | Use-case orchestration: `engine.rs` (`AgentEngine`), `turn_orchestrator.rs`, `batch_executor.rs`, `context_inspector.rs` (`/context`, `/prompt`, `/preview-request`), `computer_use_orchestrator.rs` (`ComputerUseOrchestrator` 9-stage closed loop), `spatial_engine.rs` (`SpatialInteractionEngine`), `slash_and_swarm.rs`, `session_store.rs`, `tool_governor.rs` (`FileReadTracker`, `ToolLoopBreaker`), `tool_specs.rs` (`31` built-in tools), `tool_registry.rs`, `request_queue.rs`, `flight_runner.rs`, `benchmark_runner.rs`, `substrate_tests.rs`, `spatial_tests.rs`, `computer_use_tests.rs`, `zenith_tests.rs`, `codewhale_zenith_tests.rs`, `mod.rs` |
| **`src/ui/` & Roots** (`8` modules) | `UI` | Presenters, multiplexed protocol handlers, and Composition Roots: `ui/cli_presenter.rs`, `ui/onboarding.rs`, `ui/stdio_worker.rs` (`"v": 1` JSON-RPC framing), `ui/worker_handlers.rs`, `ui/rpc_dispatch.rs`, `ui/mod.rs`, `lib.rs`, `main.rs` |
| **`native/Sources/Heav3nsCode/`** | `UI` / `PLUMBING` | Native macOS AppKit IDE client modularized under Tier 1 Optimal caps (`< 800` lines) with `Shift+Tab` posture stepping, `⌘K` Omnibar shortcut badges, 3-choice session approvals, live `TurnReceiptSummaryBar`, and `dispose()` lifecycle hygiene |
| **`native/Tests/Heav3nsCodeTests/`** | `UI` | Comprehensive `321`-test AppKit test suite (`0` failures) |
| **`docs/`** | `UI` / `DOMAIN` | Diátaxis + Institutional Memory documentation hub (`README.md`, `papers/`, `adr/`, `playbooks/`, `postmortems/`, `wiki/`, `kb/`, `api/`) |

---

## Unified Execution Architecture

```
native/Sources/Heav3nsCode/ (AppKit Native Desktop IDE · 321 Tests)
  │
  │ stdio JSON-RPC (v: 1)
  ▼
src/main.rs & src/lib.rs (Composition Roots)
  └─ src/ui/stdio_worker.rs -> src/ui/rpc_dispatch.rs
       ├─ src/core/engine.rs (AgentEngine::tick · HEAV3NS-MICRO)
       │    ├─ src/domain/types/posture_types.rs (ExecutionPosture & RepoConstitution)
       │    ├─ src/domain/types/model_catalog.rs (ModelPricingCatalog)
       │    └─ src/infrastructure/pricing_updater.rs (ModelPricingRegistry)
       ├─ src/core/computer_use_orchestrator.rs (9-Stage Closed-Loop · COMPUTER USE · SPIDER · SWARM)
       │    ├─ src/core/spatial_engine.rs (SpatialInteractionEngine · Auto-Scroll · execute_plan)
       │    │    └─ src/domain/spatial_mediator.rs (Wrapper A Grounding + Wrapper B Assurance)
       │    └─ src/infrastructure/behavioral_store.rs (BehavioralKnowledgeStore · BROCCOLI · REWIND)
       ├─ src/core/session_store.rs (ArenaAllocator · MemoryStore · SessionVfs · REWIND)
       ├─ src/infrastructure/workspace_cas.rs (WorkspaceSnapshotCas · SHA-256 CAS · /undo)
       └─ src/core/tool_registry.rs (31 Native Tools · DAG Wave Scheduler · THE HARNESS)
```
