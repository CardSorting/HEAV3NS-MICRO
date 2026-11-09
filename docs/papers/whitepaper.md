<!-- [LAYER: UI] -->
# HEAV3NS-MICRO Technical Whitepaper

**A Zero-Bloat, High-Throughput Rust Intelligence Runtime, Double-Wrapped Spatial Action Architecture & Native macOS Control Station**  
*HEAV3NS Architecture Working Group*

<p align="center">
  <a href="README.md">Papers Hub</a> ·
  <a href="../../doc.md">Foundational Charter (doc.md)</a> ·
  <a href="ascendant-intelligence.md">Ascendant Intelligence</a> ·
  <a href="companion-brief.md">Companion Brief</a> ·
  <a href="philosophy.md">Philosophy</a> ·
  <a href="../architecture/current.md">Architecture</a> ·
  <a href="../adr/0011-sovereign-intelligence-and-spatial-computer-use.md">ADR-0011</a>
</p>

---

## Abstract

We present **HEAV3NS-MICRO** (`v0.1.0`), the distilled Rust intelligence runtime of **HEAV3NS — The Sovereign Intelligence Architecture**, paired with **HEAV3NS-CODE**, a native AppKit macOS developer control station. Organized across five mathematically enforced Joyzoning layers (`64` Rust modules, `39` formal `I<Name>` traits, `29` native tools, `280+` audited workspace files), the system unifies seven sovereign pillars: **`HEAV3NS-MICRO`** (runtime core), **`SPIDER`** (structural & interface perception), **`REWIND`** (multi-plane temporal continuity), **`BROCCOLI`** (durable behavioral knowledge & evolutionary skill DAG), **`SYNTHETIC SWARM`** (5-specialist cognitive consultation & 7-specialist Legion task orchestration), **`COMPUTER USE`** (9-stage closed-loop embodied agency with Double-Wrapped Spatial Action Mediation), and **`THE HARNESS`** (wave scheduler, token governor, zero-trust security, and `"v": 1` stdio JSON-RPC bridge). We demonstrate sub-microsecond spatial grounding ($\le 0.40\ \mu\text{s}$), 100% Joyzoning Stability Index ($E_S = 0.0038$), 100% Build Health (`Grade: A+ — SOVEREIGN`), and 100% pass rates across all 49 Rust test suites (including the `FT-01..FT-05` Flight benchmark) and 311 native AppKit tests.

---

## 1. Introduction & Architectural Motivation

Autonomous software engineering and computer-use agents must reason over codebases, execute filesystem mutations, and interact with live graphical and DOM interfaces under strict reliability guarantees. Conventional agent stacks exhibit six systemic failure modes:
1. **Dynamic Memory Sprawl**: Unbounded heap allocations during streaming token generation and AST traversal.
2. **Streaming Parser Fragility**: Crashes on truncated JSON tool calls, unescaped newlines, or markdown fences.
3. **Speculative Filesystem Corruption**: Direct disk writes without baseline verification or atomic rollback.
4. **Ungrounded Pointer & Keyboard Dispatch**: Blind `(x, y)` clicks that strike overlapping modals, off-screen elements, or disabled controls without postcondition verification.
5. **Episodic Amnesia**: Inability to retain verified UI interaction patterns or multi-step recovery workflows across turns.
6. **Web-Wrapper UI Latency**: Multi-gigabyte Electron frontends that block user input with modal approval sheets.

HEAV3NS-MICRO resolves all six failure modes through a unified, sovereign architecture.

---

## 2. Memory Arena & $O(1)$ `REWIND` Formalism (`src/domain/arena.rs`)

HEAV3NS-MICRO pre-allocates a contiguous memory slab of size $M = 16 \times 1024 \times 1024$ bytes (16 MiB):

$$\text{Arena} = [b_0, b_1, \dots, b_{M-1}]$$

Allocations advance linearly via a bump pointer $P_{\text{alloc}} \in [0, M)$ with alignment $A = 4$:

$$P_{\text{aligned}} = (P_{\text{alloc}} + A - 1) \ \& \ \sim(A - 1), \qquad P_{\text{next}} = P_{\text{aligned}} + S$$

At the start of turn $t$, the engine captures a [`SlabBufferSnapshot`](../../src/domain/arena.rs) recording watermark $W_t = P_{\text{alloc}}$. Upon turn completion or `/rewind`, transient scratch allocations are reclaimed in $O(1)$ constant time:

$$P_{\text{alloc}} \leftarrow W_t$$

---

## 3. Self-Healing Streaming Parser (`src/plumbing/json_repair.rs`, `src/domain/tool_parser.rs`)

Given a streaming string buffer $S$ emitted by an LLM, the 3-stage parser executes:
1. **Fast-Path Deserialization**: Direct `serde_json` parse when $S$ is well-formed.
2. **Heuristic Structural Repair**: Strips markdown fences (` ```json `), repairs unescaped newlines inside string literals, strips trailing commas, and appends missing closing delimiters `}` / `]` via stack depth tracking.
3. **Key-Value Fallback & Alias Canonicalization**: Extracts key-value pairs and maps 60+ parameter aliases (e.g., `file_path` $\to$ `path`, `target_id` / `selector` / `ref`) while validating against [`ParamSchema`](../../src/domain/types/tool_types.rs).

---

## 4. Two-Phase Staged VFS & Rollback Formalism (`src/infrastructure/vfs_store.rs`)

When a tool stages a write at path $F$ with content $C_{\text{new}}$:
1. **Baseline Capture**: Reads existing disk file $C_{\text{disk}}$ and records [`VfsDiskBaseline`](../../src/domain/types/vfs_types.rs):
   $$\text{Baseline}(F) = \big(\text{byte\_len}(C_{\text{disk}}), \ \text{FNV1a-32}(C_{\text{disk}})\big)$$
2. **Overlay Staging**: Stages $C_{\text{new}}$ in memory as a [`VfsFileOverlay`](../../src/domain/types/vfs_types.rs).
3. **Optimistic Concurrency Commit**: Before flushing to disk, verifies $\text{CurrentDisk}(F) = \text{Baseline}(F)$ and records an inverse patch in [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) for instant `/rewind` or `rollback_last_mutation`.

---

## 5. Double-Wrapped Spatial Action Architecture (`src/domain/spatial_mediator.rs`, `src/core/spatial_engine.rs`)

To transcend brittle screenshot-only or DOM-only automation, bounding boxes are elevated into first-class [`SpatialTarget`](../../src/domain/types/spatial_types.rs) primitives mediated by two independent gates:

### 5.1 Wrapper A — Spatial Grounding Gate
Given a [`SpatialActionContract`](../../src/domain/types/spatial_types.rs) at layout epoch $\tau$:
1. **Epoch Fence**: Verifies $\tau_{\text{expected}} = \tau_{\text{current}}$ (`SpatialError::StaleTargetEpoch`).
2. **Multi-Modal Target Resolution**: Resolves candidates via Set-of-Marks badge (`som_id`), frame-fenced token (`s{hex}:{id}`), semantic label, or normalized coordinates (`screen`, `normalized_1000`, percentage). Targetless keyboard and viewport `scroll` actions ground to the active focus or active window plane.
3. **Exact 2D Sweep-Line Occlusion Union (`src/plumbing/spatial_geometry.rs`)**: For all higher z-order blocking targets $\{O_1, \dots, O_m\}$, computes exact union area of intersections $I_k = R_{\text{target}} \cap R_{O_k}$ via 1D interval sweep over unique $X$-coordinates:
   $$\text{OcclusionRatio}(R_{\text{target}}) = \frac{\text{Area}\left(\bigcup_{k=1}^m I_k\right)}{\text{Area}(R_{\text{target}})}$$
   If $\text{OcclusionRatio} > \theta_{\text{max}}$, grounding rejects the action with `SpatialError::TargetOccluded`.
4. **Viewport Visibility & Auto-Scroll Remediation**: Using [`ViewportTransform`](../../src/plumbing/spatial_geometry.rs), computes viewport intersection ratio. When $\text{Visibility} \le 0.05$, [`SpatialInteractionEngine::execute_spatially_grounded_action`](../../src/core/spatial_engine.rs) automatically dispatches a pre-scroll by centering delta $(\Delta x, \Delta y)$, updates `VirtualDisplayState.scroll_offset`, refreshes the `SpatialScene`, and re-grounds the target in-viewport (`visibilityRatio == 1.0`).
5. **Safe Interior Point**: Computes $P_{\text{safe}}$ inset by margin $m = 0.15$ from all edges of $R_{\text{target}}$.

### 5.2 Wrapper B — Execution Assurance Gate
1. **Pre-Execution Gate**: Rejects disabled targets (`SpatialError::TargetDisabled`), validates role capabilities (`ActionNotPermittedForRole`), and proves $P_{\text{safe}} \in R_{\text{target}}$ (`ContainmentViolation`).
2. **Post-Execution Gate**: Evaluates observable scene transitions between $S_{\text{before}}$ and $S_{\text{after}}$ (`focus` acquisition, `value` mutation, `frame_seq` change, or `scroll_offset` delta) to produce a signed [`SpatialExecutionReceipt`](../../src/domain/spatial_action.rs) (`AssuranceVerdict::Confirmed` or `SuspectedNoop`).

---

## 6. Closed-Loop Computer Use, `SPIDER`, `BROCCOLI` & `SYNTHETIC SWARM` (`src/core/computer_use_orchestrator.rs`)

The [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs) executes a 9-stage cybernetic loop (`OBSERVE -> ORIENT -> INTERPRET -> REASON -> PLAN -> ACT -> OBSERVE -> VERIFY -> LEARN`):
- **Modal Trap Auto-Interception**: When `SPIDER` (`detect_modal_or_dialog`) identifies an active modal window or inline dialog and the requested target lies outside the modal region, the orchestrator automatically intercepts the action and dispatches `key: Escape` to dismiss the trap while allowing internal modal interactions to proceed unhindered.
- **Auto-Focus Planning**: Typing into an unfocused input automatically schedules a foreground focus-click prior to text entry, or executes a full 3-step [`SpatialActionPlan`](../../src/domain/spatial_mediator.rs) (`scroll -> click -> type`) via `SpatialInteractionEngine::execute_plan`.
- **5-Specialist `SYNTHETIC SWARM` Consultation**: Under uncertainty or repeated failure, `VisualPerceiver`, `StructuralAnalyst`, `WorkflowNavigator`, `RiskAssessor`, and `RecoverySpecialist` synthesize recovery advice alongside `BROCCOLI` (`BehavioralKnowledgeStore`), with `ConflictResolver` ensuring live environmental observations override stale historical assumptions.

---

## 7. Topological Wave Scheduler & Desktop JSON-RPC Bridge

- **DAG Wave Scheduler (`src/core/tool_registry.rs`)**: Partitions batches of [`ScheduledToolCall`](../../src/domain/types/tool_schedule.rs) across 29 built-in tools into concurrent read waves and serialized write barriers.
- **Desktop JSON-RPC Protocol (`src/ui/stdio_worker.rs`, `src/ui/rpc_dispatch.rs`, `native/`)**: Connects the Rust substrate to the modular 8-file `WorkspaceWindow` AppKit architecture over `"v": 1` newline-delimited JSON frames with zero-replay `task.lookup` reconciliation and deterministic `dispose()` lifecycle hygiene across all 30 Swift classes.

---

## 8. Empirical Verification Summary

| Verification Suite | Command | Measured Result |
| :--- | :--- | :--- |
| **Rust Unit, Spatial & CUA Suite** | `cargo test` | **49 / 49 passed** (`0` failures, `0.05s`) |
| **Agent Flight Benchmark (`FT-01..FT-05`)** | `cargo run -- --flight` | **5 / 5 scenarios (`19 / 19` checks, `100%`)** |
| **Spatial Grounding Micro-Benchmark** | `cargo test test_microbenchmark` | **$\le 0.40\ \mu\text{s}$ / op** (`> 240,000` groundings/sec) |
| **Native macOS AppKit Test Suite** | `cd native && swift test` | **311 / 311 passed** (`0` failures) |
| **Strict Joyzoning & Stability Doctor** | `npm run joy-check` | **100% Stability Index ($E_S = 0.0038$)**, **Grade A+ (`100% SOVEREIGN`)** |
| **Documentation Guardrails** | `npm run docs:check` | **100% header, section, and link parity** |
