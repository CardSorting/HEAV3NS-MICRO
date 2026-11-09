<!-- [LAYER: UI] -->
# Glossary

<p align="center">
  <a href="README.md">Docs Hub</a> ·
  <a href="../doc.md">Sovereign Charter (doc.md)</a> ·
  <a href="CONSTITUTION.md">Constitution</a> ·
  <a href="CODEBASE_STANDARDS.md">Codebase Standards</a> ·
  <a href="api/public-api.md">Public API</a>
</p>

Definitions of core terms, sovereign pillars, architectural concepts, and code primitives used across **HEAV3NS — The Sovereign Intelligence Architecture** and **HEAV3NS-MICRO**.

---

| Term | Definition |
| :--- | :--- |
| **HEAV3NS** | **The Sovereign Intelligence Architecture** ([`doc.md`](../doc.md)) — a computational civilization of seven cooperating pillars (`HEAV3NS-MICRO`, `SPIDER`, `REWIND`, `BROCCOLI`, `SYNTHETIC SWARM`, `COMPUTER USE`, `THE HARNESS`) pursuing **The Ascendant Computational Intelligence**. |
| **`HEAV3NS-MICRO`** | The distilled Rust intelligence runtime (`src/`) providing a 16-MiB memory arena, self-healing tool parser, transactional VFS, wave scheduler, and embodied spatial action engine. |
| **`SPIDER`** | The structural perception pillar (`environmental_diff.rs`, `spider.check`) that analyzes both UI viewport regions/affordances/relationships and static codebase AST dependency graphs. |
| **`REWIND`** | The temporal cognition and continuity pillar providing $O(1)$ `ArenaAllocator` pointer rewind, `SessionVfs` baseline checks, `TransactionJournal` file rollback, 32-deep display undo/redo, and epoch drift reconciliation. |
| **`BROCCOLI`** | The persistent knowledge and evolutionary memory pillar (`BehavioralKnowledgeStore`, `MemoryStore`, `SkillTreeCatalog`) calibrating interaction confidence and synthesizing reusable workflow procedures. |
| **`SYNTHETIC SWARM`** | The distributed collective intelligence pillar combining 5-specialist cognitive consultation (`VisualPerceiver`, `StructuralAnalyst`, `WorkflowNavigator`, `RiskAssessor`, `RecoverySpecialist` + `ConflictResolver`) and 7-specialist `Legion` task formations. |
| **`COMPUTER USE`** | The embodied agency pillar executing the 9-stage closed loop (`Observe -> Orient -> Interpret -> Reason -> Plan -> Act -> Observe -> Verify -> Learn`) over Double-Wrapped Spatial Action Mediation. |
| **`THE HARNESS`** | The computational substrate binding the pillars together (`ToolRegistry` wave scheduler, `ToolOutputGovernor`, `DeterministicSecretRedactor`, `"v": 1` stdio JSON-RPC bridge, and `HEAV3NS-CODE` AppKit IDE). |
| **Double-Wrapped Action Mediation** | Two-stage spatial action gate in [`SpatialActionMediator`](../src/domain/spatial_mediator.rs): **Wrapper A (Spatial Grounding Gate)** verifies epoch freshness, 2D occlusion union, viewport visibility (with off-screen auto-scroll), and 15% inset safe points; **Wrapper B (Execution Assurance Gate)** enforces role/enabled preconditions and verifies post-action focus/value/scroll deltas. |
| **Grounding Sovereignty** | The law enforced by `ConflictResolver` requiring live environmental observations (`SPIDER`) to take strict precedence over conflicting historical patterns in `BROCCOLI`. |
| **Joyzoning** | The architectural discipline dividing code into five sovereign zones (`DOMAIN`, `CORE`, `INFRASTRUCTURE`, `PLUMBING`, `UI`) tagged with top-of-file headers. |
| **PGA** (Principle of Geographic Alignment) | The rule requiring a file's physical directory path (`src/<zone>/`) to match its declared `[LAYER: <ZONE>]` header tag (`SPI-208`). |
| **Contractual Sovereignty** | The rule requiring `CORE` orchestrators to depend on formal `I<Name>` traits (`39` traits in `src/domain/`), never directly on concrete infrastructure structs (`SPI-107`). |
| **`ArenaAllocator`** | High-performance contiguous memory slab pre-allocating a 16-MiB buffer with linear bump-pointer allocation and $O(1)$ tick unwinding ([ADR-0002](adr/0002-arena-allocator-and-o1-tick-rewind.md)). |
| **`SessionVfs`** | In-memory Virtual File System layer that stages modifications and verifies FNV-1a disk baselines (`VfsDiskBaseline`) before applying mutations to physical disk ([ADR-0004](adr/0004-two-phase-vfs-staging-and-transaction-journal.md)). |
| **`TransactionJournal`** | Append-only ledger recording every committed filesystem modification with full reverse patch data for deterministic undo. |
| **`WaveScheduler`** | Concurrency engine in `ToolRegistry` that partitions emitted tool calls into parallel read wavefronts and serialized write barriers ([ADR-0005](adr/0005-dag-wave-scheduler-concurrency.md)). |
| **Set-of-Marks (SoM)** | Visual and DOM target indexing scheme assigning monotonic `som_id` badges and frame-fenced `element_token` identifiers (`s{hex}:{id}`) to interactive UI controls. |
| **Tier 1 Optimal** | Module complexity standard requiring all files to stay under 800 lines of code (`SPI-203`). |
