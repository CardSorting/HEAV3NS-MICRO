<!-- [LAYER: UI] -->
# Project Map & Architecture (`HEAV3NS-MICRO`)

This map reflects the complete layout and organization of the **HEAV3NS-MICRO** workspace (`/Users/bozoegg/Desktop/HEAV3NS-MICRO`), encompassing the 70-module Rust sovereign intelligence runtime and the AppKit Native Desktop IDE client (`HEAV3NS-CODE`).

## Workspace Overview

| Directory / File | Architectural Zone | Sovereign Pillars & Role |
| :--- | :--- | :--- |
| **`doc.md` & `ROADMAP.md`** | `UI` | Foundational charter of **HEAV3NS — The Sovereign Intelligence Architecture** (`THE ASCENDANT COMPUTATIONAL INTELLIGENCE`) and multi-horizon engineering roadmap |
| **`src/plumbing/`** (`11` modules) | `PLUMBING` | Stateless leaf primitives (`0%` state, `0` upward imports): `hash_and_math.rs`, `text_tokens.rs`, `json_repair.rs`, `secret_redactor.rs`, `shell_guard.rs`, `fuzzy_patch.rs`, `vision_and_dom.rs`, `spatial_geometry.rs`, `environmental_diff.rs`, `display_telemetry.rs`, `mod.rs` |
| **`src/domain/`** (`11` modules) | `DOMAIN` | Pure domain models & state machines (`0%` I/O, `<= 3` structs/file): `prompt_spec.rs` (`HEAV3NS-MICRO`), `arena.rs` (`REWIND`), `memory.rs` (`BROCCOLI`), `compaction.rs` (`HEAV3NS-MICRO`), `swarm.rs` (`SYNTHETIC SWARM`), `tool_parser.rs` (`HEAV3NS-MICRO`), `computer_use.rs` (`COMPUTER USE`), `skill_tree.rs` (`BROCCOLI`), `spatial_action.rs` & `spatial_mediator.rs` (`COMPUTER USE`), `mod.rs` |
| **`src/domain/interfaces/`** (`1` module) | `DOMAIN` | `19` formal Dependency Inversion port traits (`mod.rs`): `ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal`, `ISpatialMediator`, `ISpatialGroundingGate`, `IExecutionAssuranceGate`, `IEnvironmentalObserver`, `IBehavioralKnowledgeStore`, `IBehavioralStore`, `ISwarmIntelligenceConsultant`, `IEnvironmentalDiffer`, `ISpatialGeometry`, `IComputerUseOrchestrator` |
| **`src/domain/types/`** (`14` modules) | `DOMAIN` | Pure domain value & behavioral types (`20` entity `I<Name>` traits, `<= 3` structs/file): `messages.rs`, `runtime.rs`, `progress.rs`, `tick_result.rs`, `vfs_types.rs`, `tool_types.rs`, `tool_schedule.rs`, `skill_types.rs`, `computer_use_types.rs` (`UiElement`, `VirtualWindow`), `spatial_types.rs`, `environmental_types.rs`, `swarm_intelligence_types.rs`, `learned_knowledge_types.rs`, `mod.rs` |
| **`src/infrastructure/`** (`9` modules) | `INFRASTRUCTURE` | Concrete I/O adapters implementing `domain/interfaces`: `http_provider.rs`, `scripted_provider.rs`, `vfs_store.rs` (`SessionVfs`, `SkillsIngestor`, `WorkspacePromptProbe`, `FileSessionPersistence`), `transaction_journal.rs` (`TransactionJournal`), `behavioral_store.rs` (`BehavioralKnowledgeStore`), `shell_runner.rs`, `fs_tools.rs`, `search_tools.rs`, `mod.rs` |
| **`src/core/`** (`17` modules) | `CORE` | Use-case orchestration: `engine.rs` (`AgentEngine`), `context_inspector.rs` (`/context` & `/prompt`), `computer_use_orchestrator.rs` (`ComputerUseOrchestrator` 9-stage closed loop), `spatial_engine.rs` (`SpatialInteractionEngine` Double-Wrapped dispatch, off-screen auto-scroll & `execute_plan`), `slash_and_swarm.rs` (`run_swarm` & 19 slash commands), `session_store.rs`, `tool_governor.rs`, `tool_specs.rs` (`29` built-in tools), `tool_registry.rs` (`ToolRegistry`, sticky schema routing & wave scheduler), `request_queue.rs`, `flight_runner.rs`, `benchmark_runner.rs`, `substrate_tests.rs`, `spatial_tests.rs`, `computer_use_tests.rs`, `zenith_tests.rs`, `mod.rs` |
| **`src/ui/` & Roots** (`7` modules) | `UI` | Presenters, protocol handlers, and Composition Roots: `ui/cli_presenter.rs`, `ui/onboarding.rs`, `ui/stdio_worker.rs` (`"v": 1` JSON-RPC framing), `ui/rpc_dispatch.rs`, `ui/mod.rs`, `lib.rs`, `main.rs` |
| **`native/Sources/Heav3nsCode/`** | `UI` / `PLUMBING` | Native macOS AppKit IDE client modularized under Tier 1 Optimal caps (`< 800` lines) with `dispose()` lifecycle hygiene across all 30 Swift classes |
| **`native/Tests/Heav3nsCodeTests/`** | `UI` | Comprehensive `315`-test AppKit test suite (`0` failures) |
| **`docs/`** | `UI` / `DOMAIN` | Diátaxis + Institutional Memory documentation hub (`README.md`, `papers/`, `adr/`, `playbooks/`, `postmortems/`, `wiki/`, `kb/`, `api/`) |

---

## Unified Execution Architecture

```
native/Sources/Heav3nsCode/ (AppKit Native Desktop IDE · 311 Tests)
  │
  │ stdio JSON-RPC (v: 1)
  ▼
src/main.rs & src/lib.rs (Composition Roots)
  └─ src/ui/stdio_worker.rs -> src/ui/rpc_dispatch.rs
       ├─ src/core/engine.rs (AgentEngine::tick · HEAV3NS-MICRO)
       ├─ src/core/computer_use_orchestrator.rs (9-Stage Closed-Loop · COMPUTER USE · SPIDER · SWARM)
       │    ├─ src/core/spatial_engine.rs (SpatialInteractionEngine · Auto-Scroll · execute_plan)
       │    │    └─ src/domain/spatial_mediator.rs (Wrapper A Grounding + Wrapper B Assurance)
       │    └─ src/infrastructure/behavioral_store.rs (BehavioralKnowledgeStore · BROCCOLI · REWIND)
       ├─ src/core/session_store.rs (ArenaAllocator · MemoryStore · SessionVfs · REWIND)
       └─ src/core/tool_registry.rs (29 Native Tools · DAG Wave Scheduler · THE HARNESS)
```
