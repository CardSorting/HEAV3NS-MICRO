<!-- [LAYER: UI] -->
# ROADMAP — Toward The Ascendant Computational Intelligence (`HEAV3NS`)

<p align="center">
  <a href="doc.md">Sovereign Charter (doc.md)</a> ·
  <a href="README.md">README</a> ·
  <a href="docs/papers/ascendant-intelligence.md">Ascendant Intelligence Paper</a> ·
  <a href="docs/architecture/current.md">Current Architecture</a> ·
  <a href="native/ROADMAP.md">Native AppKit Roadmap</a>
</p>

This roadmap charts the completed engineering milestones of **HEAV3NS-MICRO (`v0.1.0`)** and defines the concrete aspirational horizons required to implement the foundational vision of **[`doc.md`](doc.md) — HEAV3NS: The Sovereign Intelligence Architecture (`THE ASCENDANT COMPUTATIONAL INTELLIGENCE`)** in full.

---

## Completed Foundation (`v0.1.0` — Verified & Sovereign)

- [x] **Pillar 1 (`HEAV3NS-MICRO` — Distilled Intelligence Runtime)**:
  - Self-contained Rust micro-runtime with 16-MiB `ArenaAllocator`, 3-stage self-healing `ToolCallArgParser`, multi-provider failover, `/btw` side-query isolation, and `FT-01..FT-05` Flight Test suite (`19/19` checks passing).
- [x] **Pillar 2 (`SPIDER` — Structural Perception & Environmental Understanding)**:
  - Functional viewport region classification (`HeaderNav`, `Sidebar`, `Toolbar`, `ContentArea`, `FooterStatusBar`, `ModalDialog`), UI element affordance classification (including disabled-control guards), relational graph extraction (`contains`, `labelled_by`, `controls`, `submits`, `adjacent`), and `spider.check` static AST/cycle verification.
- [x] **Pillar 3 (`REWIND` — Temporal Cognition, Continuity & Recoverability)**:
  - $O(1)$ `SlabBufferSnapshot` arena pointer rewind, pure-Rust SHA-256 `WorkspaceSnapshotCas` (`WouldBlock`-protected `/undo`, `/restore`, non-Git `/diff`), two-phase `SessionVfs` with FNV-1a `VfsDiskBaseline` parity verification, `TransactionJournal` atomic file rollback, 32-deep `VirtualDisplayState` undo/redo ring, and `BehavioralKnowledgeStore::reconcile_state` epoch drift detection.
- [x] **Pillar 4 (`BROCCOLI` — Persistent Knowledge, Memory & Historical Evolution)**:
  - `BehavioralKnowledgeStore` with Laplace-smoothed confidence calibration, automatic `LearnedWorkflowProcedure` synthesis from multi-step recoveries, hybrid TF-IDF + FNV-1a `MemoryStore` synchronization, and Kahn-verified `SkillTreeCatalog` evolutionary DAG with `SkillDoctor` linter and multi-format Soul Drop Vault ingestion.
- [x] **Pillar 5 (`SYNTHETIC SWARM` — Distributed Collective Intelligence)**:
  - 5-specialist parallel cognitive consultation (`VisualPerceiver`, `StructuralAnalyst`, `WorkflowNavigator`, `RiskAssessor`, `RecoverySpecialist`) with `ConflictResolver` grounding sovereignty and memoization cache, plus 7-specialist `Legion` topological task classification (`classify_task_for_legion`).
- [x] **Pillar 6 (`COMPUTER USE` — Embodied Agency & Double-Wrapped Spatial Mediation)**:
  - 9-stage closed-loop `ComputerUseOrchestrator` (`Observe -> Orient -> Interpret -> Reason -> Plan -> Act -> Observe -> Verify -> Learn`), `SpatialActionMediator` Double-Wrapped Action Mediation (`Wrapper A` Grounding in $\le 0.40\ \mu\text{s}$ + `Wrapper B` Execution Assurance), automatic off-screen viewport pre-scrolling, multi-step `execute_plan`, modal trap auto-interception (`key: Escape`), disabled-element protection, checkbox/radio state toggling, and SSRF-hardened `browser_use`.
- [x] **Pillar 7 (`THE HARNESS` — Sovereign Computational Substrate & Native Control Station)**:
  - 5-Zone **HEAV3NS-JOY** architecture across `80` Rust modules and `307` workspace files (`100%` Stability Index, `Grade: A+ — SOVEREIGN`), `41` formal `I<Name>` traits, `31` native tools (`tool_search` 8-slot LRU + `execute_tools` batch + DAG wave scheduling), 5-stop `ExecutionPosture` (`Shift+Tab`), macOS Seatbelt `sandbox-exec`, `TurnReceipt` (`heav3ns.receipt/v1`), multiplexed `"v": 1` stdio JSON-RPC bridge, and modular AppKit `HEAV3NS-CODE` desktop client (`321/321` Swift tests passing, `77/77` Rust tests passing).

---

## Horizon I (`v0.2.0`): Native OS Kernel-Bridge & 60 Hz Hardware Embodiment

**Objective**: Bind the verified `SpatialActionMediator` and `VirtualDisplayState` contracts directly to live macOS hardware framebuffers and Accessibility APIs with zero-copy throughput.

- [ ] **H1.1 — Zero-Copy `ScreenCaptureKit` & `AXUIElement` Adapter (`src/infrastructure/`)**:
  - Implement a live macOS `LiveMacOsDisplayAdapter` implementing `IVirtualDisplay` that ingests `AXUIElement` hierarchies and `CGWindowList` z-order bounds directly into `SpatialScene` while preserving deterministic `VirtualDisplayState` simulation for offline flight tests.
- [ ] **H1.2 — Hardware-Accelerated Set-of-Marks & Optical Glyph Grounding**:
  - Fuse CoreText / Vision framework bounding-box detection with DOM and Accessibility trees under `TargetProvenance::Reconciled`, achieving `< 16.6 ms` (60 Hz) full-screen perception-to-grounding latency.
- [ ] **H1.3 — Native MCP Server & Federation Mode (`--mcp`)**:
  - Expose all 29 native tools (`spatial_interact`, `computer_use`, `browser_use`, `vision_inspect`, `skill_forge`, `engage_swarm`, VFS tools) over standard Model Context Protocol JSON-RPC 2.0 streams.

---

## Horizon II (`v0.3.0`): Causal `SPIDER` Code-to-Pixel Tracing & Universal `REWIND`

**Objective**: Unify structural code perception with visual UI perception and enable instant branching time-travel across entire execution universes.

- [ ] **H2.1 — Bi-Directional Code-to-Pixel `SPIDER` Graph**:
  - Link every interactive `SpatialTarget` rendered in a running application preview (`ProjectRunView`) back to the exact AST declaration and source file line (`Rust`, `Swift`, `TypeScript`) that produced it.
- [ ] **H2.2 — Universal Branching Time-Travel (`REWIND` Universe Forking)**:
  - Combine `SlabBufferSnapshot` (Arena), `VfsFileOverlay` (SessionVfs), `MutationTransaction` (TransactionJournal), `.broccolidb` Git checkpoints, and `VirtualDisplayState` snapshots into a single `CausalTimelineSnapshot` supporting $O(1)$ speculative execution forks.
- [ ] **H2.3 — Parallel Speculative Swarm Execution**:
  - Allow `SYNTHETIC SWARM` specialists to fork 3 parallel `REWIND` universes, test competing code patches or UI workflows concurrently, verify postconditions via Wrapper B, and commit only the highest-scoring branch.

---

## Horizon III (`v0.4.0`): Federated `BROCCOLI` Vault & Autonomous Skill Distillation

**Objective**: Enable continuous self-evolution where every resolved obstacle permanently elevates the collective intelligence of the workspace.

- [ ] **H3.1 — Cross-Pod Federated `BROCCOLI` Knowledge Graph**:
  - Synchronize high-confidence `LearnedInteractionPattern` and `LearnedWorkflowProcedure` records across all loaded project pods in `HEAV3NS-CODE` (`PodsWindow`) with cryptographic provenance.
- [ ] **H3.2 — Closed-Loop Autonomous Skill Forging**:
  - When `ComputerUseOrchestrator` resolves a failure via `RecoverySpecialist` escalation, automatically synthesize a candidate `SkillNode`, pass it through `SkillDoctor` security/threat linting, verify it against a replay test, and register it in `SkillTreeCatalog`.
- [ ] **H3.3 — Red-Blue Adversarial Verification Waves**:
  - Automatically pair every mutating `legion_implementer` wave with an adversarial `legion_hardener` specialist that generates boundary/fuzz tests before `SessionVfs::commit_staged` is authorized.

---

## Horizon IV (`v1.0.0`): The Ascendant Computational Civilization

**Objective**: Full realization of the `HEAV3NS` charter (`BILLIONS MUST COMPUTE`).

- [ ] **H4.1 — Hybrid Neural-Engine + Frontier Swarm Routing**:
  - Route sub-millisecond `SPIDER` perception, `Wrapper A/B` spatial mediation, and `SYNTHETIC SWARM` specialist consultations to local Apple Silicon (`OnDevicePlaygroundService`) while reserving frontier models for deep architectural synthesis.
- [ ] **H4.2 — Self-Healing Architectural Homeostasis**:
  - Continuous background `StabilityDoctor` monitoring that detects structural entropy increases ($E_S > 0.0050$) and autonomously dispatches Joyzoning refactoring waves to maintain perpetual `100% SOVEREIGN` equilibrium.
