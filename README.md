<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`) — The Sovereign Intelligence Runtime

**Hyper-Compressed, Architecturally Sovereign Rust Intelligence Runtime, Double-Wrapped Spatial Action Engine & Native macOS AppKit IDE (`HEAV3NS-CODE`)** — structured under the **HEAV3NS-JOY** 5-Zone Joyzoning architecture (`100%` Stability Index, $E_S = 0.0038$, `0` blocking violations, `0` advisories, all `280+` workspace files `< 800` lines Tier 1 Optimal).

<p align="center">
  <a href="doc.md">Sovereign Charter (doc.md)</a> ·
  <a href="AGENTS.md">AGENTS.md</a> ·
  <a href="SOUL.md">SOUL.md</a> ·
  <a href="ROADMAP.md">Aspirational Roadmap</a> ·
  <a href="docs/README.md">Documentation Hub</a> ·
  <a href="docs/papers/ascendant-intelligence.md">Ascendant Intelligence Paper</a> ·
  <a href="docs/PROJECT_MAP.md">Project Map</a> ·
  <a href="docs/CONSTITUTION.md">Constitution</a> ·
  <a href="VALIDATION_REPORT.md">Validation Report</a> ·
  <a href="native/PRODUCT.md">Native Product Spec</a>
</p>

---

## The Seven Sovereign Pillars (`HEAV3NS`)

> *"Not an agent. Not a swarm. Not an operating system. A computational civilization of cooperating intelligence — where the system doesn't merely execute instructions: it understands its environment, learns its structure, remembers its experience, and acts with intent."* — [`doc.md`](doc.md)

| Pillar | Designation | Core Implementation |
| :--- | :--- | :--- |
| **1. `HEAV3NS-MICRO`** | **The Distilled Intelligence Runtime** | [`AgentEngine`](src/core/engine.rs) multi-turn ReAct state machine, KV-cache-deterministic [`SystemPromptSpec`](src/domain/prompt_spec.rs) (`Standard`, `Compact`, `SwarmSpecialist`, `SideQuestion`), [`ContextInspectionSnapshot`](src/core/context_inspector.rs) (`/context` & `/prompt`), 3-stage self-healing [`ToolCallArgParser`](src/domain/tool_parser.rs), and `FT-01..FT-05` [`flight_runner`](src/core/flight_runner.rs). |
| **2. `SPIDER`** | **Structural Perception & Environmental Understanding** | [`environmental_diff.rs`](src/plumbing/environmental_diff.rs) (`analyze_spider_structural_relationships`, `classify_element_affordance`, `detect_modal_or_dialog`), [`EnvironmentalObservation`](src/domain/types/environmental_types.rs), and `spider.check` AST audits. |
| **3. `REWIND`** | **Temporal Cognition, Continuity & Recoverability** | 16-MiB [`ArenaAllocator`](src/domain/arena.rs) with $O(1)$ `SlabBufferSnapshot` rewind, pure-Rust SHA-256 [`WorkspaceSnapshotCas`](src/infrastructure/workspace_cas.rs) (`WouldBlock`-protected `/undo`, `/restore`, non-Git `/diff`), two-phase [`SessionVfs`](src/infrastructure/vfs_store.rs) baseline verification, [`TransactionJournal`](src/infrastructure/transaction_journal.rs) rollback, and 32-deep [`VirtualDisplayState`](src/domain/computer_use.rs) undo/redo. |
| **4. `BROCCOLI`** | **Persistent Knowledge, Memory & Historical Evolution** | [`BehavioralKnowledgeStore`](src/infrastructure/behavioral_store.rs) with Laplace-smoothed confidence calibration & [`LearnedWorkflowProcedure`](src/domain/types/learned_knowledge_types.rs) synthesis, hybrid TF-IDF [`MemoryStore`](src/domain/memory.rs), and Kahn-verified [`SkillTreeCatalog`](src/domain/skill_tree.rs) DAG. |
| **5. `SYNTHETIC SWARM`** | **Distributed Collective Intelligence** | 5-specialist parallel consultation ([`ComputerUseOrchestrator::consult`](src/core/computer_use_orchestrator.rs): `VisualPerceiver`, `StructuralAnalyst`, `WorkflowNavigator`, `RiskAssessor`, `RecoverySpecialist` + `ConflictResolver`) and 7-specialist [`Legion`](src/domain/swarm.rs) topological formations with exclusive file reservations & bounded handoffs. |
| **6. `COMPUTER USE`** | **Embodied Agency: Observation, Interpretation, Action, Adaptation** | 9-stage closed-loop [`ComputerUseOrchestrator`](src/core/computer_use_orchestrator.rs), Double-Wrapped [`SpatialActionMediator`](src/domain/spatial_mediator.rs) (`Wrapper A` Grounding + `Wrapper B` Assurance), [`SpatialInteractionEngine`](src/core/spatial_engine.rs) (off-screen auto-scroll & multi-step `execute_plan`), and zero-trust `browser_use`. |
| **7. `THE HARNESS`** | **The Computational Substrate That Binds It All Together** | 31 native tools (`tool_search` 8-slot LRU + `execute_tools` 32-step batch) & DAG Wave Scheduler ([`ToolRegistry`](src/core/tool_registry.rs)), 5-stop [`ExecutionPosture`](src/domain/types/posture_types.rs) (`Shift+Tab` / fail-closed `RepoConstitution`), macOS Seatbelt [`seatbelt_profile.rs`](src/plumbing/seatbelt_profile.rs), [`TurnReceipt`](src/domain/turn_receipt.rs) (`heav3ns.receipt/v1`), multiplexed `"v": 1` stdio JSON-RPC worker ([`stdio_worker.rs`](src/ui/stdio_worker.rs)), and Native macOS AppKit IDE ([`native/`](native/)). |

---

## 5-Zone Joyzoning Architecture (`80` Rust Modules · `307` Workspace Files)

Every source file belongs to one of five strictly enforced architectural zones, verified by `joy-check --theme strict`:

| Zone | Path | Responsibility & Invariants |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](src/plumbing/mod.rs) (`12` modules), `native/...` | Stateless leaf primitives: `hash_and_math.rs`, `text_tokens.rs`, `json_repair.rs`, `secret_redactor.rs`, `seatbelt_profile.rs`, `shell_guard.rs`, `fuzzy_patch.rs`, `vision_and_dom.rs`, `spatial_geometry.rs`, `environmental_diff.rs`, `display_telemetry.rs`. **0 upward imports, 0 mutable state (`SPI-205`).** |
| **`DOMAIN`** | [`src/domain/`](src/domain/mod.rs) (`30` modules) | Pure entities & `41` formal `I<Name>` traits ([`interfaces/`](src/domain/interfaces/mod.rs)), behavioral & value types ([`types/`](src/domain/types/mod.rs), [`posture_types.rs`](src/domain/types/posture_types.rs)), [`SystemPromptSpec`](src/domain/prompt_spec.rs), [`PrefixStabilityTracker`](src/domain/prefix_and_manifest.rs), [`BillingMetering`](src/domain/pricing_and_taxonomy.rs), [`TurnReceipt`](src/domain/turn_receipt.rs), [`ArenaAllocator`](src/domain/arena.rs), [`VirtualDisplayState`](src/domain/computer_use.rs), [`SpatialActionMediator`](src/domain/spatial_mediator.rs), [`SkillTreeCatalog`](src/domain/skill_tree.rs), [`MemoryStore`](src/domain/memory.rs), [`CompactionPolicy`](src/domain/compaction.rs), [`SwarmAssignment`](src/domain/swarm.rs), and [`ToolCallArgParser`](src/domain/tool_parser.rs). **0% I/O (`SPI-206`), `<= 3` structs/file (`SPI-203`).** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](src/infrastructure/mod.rs) (`10` modules) | Concrete I/O adapters: [`HttpProvider`](src/infrastructure/http_provider.rs), [`ScriptedProvider`](src/infrastructure/scripted_provider.rs), [`SessionVfs`](src/infrastructure/vfs_store.rs), [`WorkspaceSnapshotCas`](src/infrastructure/workspace_cas.rs), [`BehavioralKnowledgeStore`](src/infrastructure/behavioral_store.rs), [`TransactionJournal`](src/infrastructure/transaction_journal.rs), [`execute_shell_command_sandboxed`](src/infrastructure/shell_runner.rs), [`dispatch_fs_tool`](src/infrastructure/fs_tools.rs), and [`search_tools`](src/infrastructure/search_tools.rs). |
| **`CORE`** | [`src/core/`](src/core/mod.rs) (`20` modules) | Use-case orchestration: [`AgentEngine`](src/core/engine.rs), [`turn_orchestrator`](src/core/turn_orchestrator.rs), [`batch_executor`](src/core/batch_executor.rs), [`context_inspector`](src/core/context_inspector.rs), [`ComputerUseOrchestrator`](src/core/computer_use_orchestrator.rs), [`SpatialInteractionEngine`](src/core/spatial_engine.rs), [`slash_and_swarm`](src/core/slash_and_swarm.rs), [`SessionStore`](src/core/session_store.rs), [`ToolRegistry`](src/core/tool_registry.rs), [`tool_specs`](src/core/tool_specs.rs), [`ToolOutputGovernor`](src/core/tool_governor.rs), [`RuntimeRequestQueue`](src/core/request_queue.rs), [`flight_runner`](src/core/flight_runner.rs), [`benchmark_runner`](src/core/benchmark_runner.rs), and test suites (`substrate_tests`, `spatial_tests`, `computer_use_tests`, `zenith_tests`, `codewhale_zenith_tests`). |
| **`UI`** | [`src/ui/`](src/ui/mod.rs) (`8` modules w/ roots), [`native/`](native/) | CLI presenter (`cli_presenter.rs`, `onboarding.rs`), multiplexed stdio JSON-RPC worker (`stdio_worker.rs`, `worker_handlers.rs`), desktop RPC router (`rpc_dispatch.rs`), AppKit Native Desktop IDE (`native/Sources/Heav3nsCode/`), and Composition Roots ([`lib.rs`](src/lib.rs), [`main.rs`](src/main.rs)). |

---

## Quick Start & Verification

```bash
# 1. Run Rust unit, spatial, computer-use, zenith, codewhale & integration test suites (77 tests)
cargo test

# 2. Run Native macOS AppKit test suite (321 tests)
cd native && swift test && cd ..

# 3. Inspect live prompt & context budget HUD, outbound request manifest, receipts, and Flight Suite
cargo run -- context
cargo run -- preview-request
cargo run -- receipts
cargo run -- doctor
cargo run -- --flight
cargo run -- --smoke

# 4. Run strict Joyzoning architectural audit & Stability Doctor (100% SI, Grade A+)
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --doctor

# 5. Run documentation guardrails check
node bin/docs-check.mjs
```
