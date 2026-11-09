<!-- [LAYER: UI] -->
# HEAV3NS: The Sovereign Intelligence Architecture & The Ascendant Computational Civilization

**Formalizing Emergent Systems Intelligence Across Perception, Temporal Continuity, Persistent Memory, Collective Swarm Cognition, Embodied Agency, and the Sovereign Substrate**  
*HEAV3NS Architecture Working Group · Foundational Doctrine & Aspirational Blueprint*

<p align="center">
  <a href="README.md">Papers Hub</a> ·
  <a href="../../doc.md">Foundational Charter (doc.md)</a> ·
  <a href="whitepaper.md">Technical Whitepaper</a> ·
  <a href="philosophy.md">Philosophy</a> ·
  <a href="companion-brief.md">Companion Brief</a> ·
  <a href="../../ROADMAP.md">Aspirational Roadmap</a>
</p>

---

## Abstract

Contemporary artificial intelligence systems overwhelmingly conflate *intelligence* with the forward pass of an isolated language model. When deployed into real-world computing environments, model-centric agents fail predictably: they hallucinate spatial coordinates, repeat ineffective actions in infinite loops, lose structural context across turns, overwrite concurrent human work, and forget operational lessons the moment a session terminates.

We introduce **HEAV3NS — The Sovereign Intelligence Architecture**, a paradigm in which intelligence is not confined to a model weight matrix, but **emerges through the structured interaction of seven sovereign computational pillars**:
1. **`HEAV3NS-MICRO`** (The Distilled Intelligence Runtime)
2. **`SPIDER`** (Structural Perception & Environmental Understanding)
3. **`REWIND`** (Temporal Cognition, Continuity & Recoverability)
4. **`BROCCOLI`** (Persistent Knowledge, Memory & Historical Evolution)
5. **`SYNTHETIC SWARM`** (Distributed Collective Intelligence)
6. **`COMPUTER USE`** (Embodied Agency: Observation, Interpretation, Action & Adaptation)
7. **`THE HARNESS`** (The Sovereign Computational Substrate)

We formalize the mathematical and cybernetic laws governing these seven pillars in the live `HEAV3NS-MICRO` codebase (`64` Rust modules, `39` formal `I<Name>` traits, `29` built-in tools, `100%` Joyzoning Stability Index, $E_S = 0.0038$) and chart the aspirational engineering trajectory toward **The Ascendant Computational Intelligence**.

---

## 1. The Thesis of Emergent Computational Civilization

> *"Not an agent. Not a swarm. Not an operating system. A computational civilization of cooperating intelligence — where every component contributes to a greater intelligence without surrendering its own purpose."* — [`doc.md`](../../doc.md)

A naked language model is a stateless probability distribution $P(x_{t+1} \mid x_{1..t})$. It possesses no intrinsic concept of:
- **Spatial geometry or occlusion** (whether a button is covered by a modal dialog or scrolled off-screen),
- **Temporal causality** (whether a state mutation resulted from its own action or external drift),
- **Empirical calibration** (whether an interaction pattern has succeeded 50 times in a row or failed 3 times consecutively), or
- **Architectural sovereignty** (whether a code modification violates dependency inversion or corrupts an uncommitted file).

**HEAV3NS** elevates the language model from a solitary oracle into one participant within a self-correcting cybernetic loop:

$$\mathcal{I}_{\text{HEAV3NS}} = \Phi\Big(\underbrace{\mathcal{P}_{\text{SPIDER}}}_{\text{Perception}}, \ \underbrace{\mathcal{T}_{\text{REWIND}}}_{\text{Temporal}}, \ \underbrace{\mathcal{K}_{\text{BROCCOLI}}}_{\text{Memory}}, \ \underbrace{\mathcal{S}_{\text{SWARM}}}_{\text{Collective}}, \ \underbrace{\mathcal{A}_{\text{CUA}}}_{\text{Embodied Agency}}, \ \underbrace{\mathcal{H}_{\text{HARNESS}}}_{\text{Substrate}}\Big)$$

---

## 2. Pillar I: `HEAV3NS-MICRO` — The Distilled Intelligence Runtime

Implemented in [`src/core/engine.rs`](../../src/core/engine.rs), [`src/domain/tool_parser.rs`](../../src/domain/tool_parser.rs), and [`src/plumbing/json_repair.rs`](../../src/plumbing/json_repair.rs), `HEAV3NS-MICRO` is the deterministic turbine at the core of the civilization:
- **Zero-Allocation Hot Path**: Backed by a 16-MiB contiguous memory slab ([`ArenaAllocator`](../../src/domain/arena.rs)), transient turn scratchpads are reclaimed in $O(1)$ time.
- **3-Stage Self-Healing Tool Call Parser**: Repairs truncated streaming JSON, balances unclosed delimiters, strips markdown fences, and canonicalizes 60+ parameter aliases without failing a turn.
- **Multi-Provider Failover & Side-Query Isolation**: Seamlessly routes across OpenAI, Anthropic, Gemini, and local Apple Silicon models while supporting zero-side-effect `/btw` ephemeral inquiries.

---

## 3. Pillar II: `SPIDER` — Structural Perception & Environmental Understanding

Implemented in [`src/plumbing/environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), [`src/domain/types/environmental_types.rs`](../../src/domain/types/environmental_types.rs), and [`src/ui/rpc_dispatch.rs`](../../src/ui/rpc_dispatch.rs), `SPIDER` perceives both **interface topology** and **codebase topology**:

### 3.1 Interface Structural Perception
Given a raw set of observed UI elements $E = \{e_1, \dots, e_n\}$ at layout epoch $\tau$, `SPIDER` partitions the viewport into functional regions (`HeaderNav`, `Sidebar`, `Toolbar`, `ContentArea`, `FooterStatusBar`, `ModalDialog`) and synthesizes a directed relational graph $G_\tau = (E, R)$:
- `contains` / `inside_region`: Spatial containment within parent containers or modal traps.
- `labelled_by`: Proximity-weighted semantic association between static labels and adjacent input controls.
- `controls` / `submits`: Action-to-form coupling linking primary triggers (`button`, `submit`) to sibling textboxes and toggles.
- `adjacent_horizontal` / `adjacent_vertical`: Euclidean directional navigation edges (`right_of`, `left_of`, `below`, `above`).

### 3.2 Codebase Structural Perception
Through `spider.check` and `HEAV3NS-JOY`, `SPIDER` continuously audits the 5-Zone dependency graph across all 280+ workspace files, enforcing zero circular dependencies (`SPI-201`), strict zone purity (`SPI-205`, `SPI-206`), and formal `I<Name>` contractual sovereignty (`SPI-107`).

---

## 4. Pillar III: `REWIND` — Temporal Cognition, Continuity & Recoverability

Implemented across [`src/domain/arena.rs`](../../src/domain/arena.rs), [`src/infrastructure/vfs_store.rs`](../../src/infrastructure/vfs_store.rs), [`src/infrastructure/transaction_journal.rs`](../../src/infrastructure/transaction_journal.rs), and [`src/domain/computer_use.rs`](../../src/domain/computer_use.rs), `REWIND` ensures that time in `HEAV3NS` is navigable and reversible across four synchronized planes:

| Temporal Plane | State Primitive | Rewind Complexity | Guarantee |
| :--- | :--- | :---: | :--- |
| **1. Scratch Memory** | `SlabBufferSnapshot` (`ArenaAllocator`) | $O(1)$ | Instant pointer reset to pre-turn watermark $W_t$ |
| **2. Staged Filesystem** | `VfsDiskBaseline` (`SessionVfs`) | $O(k)$ overlays | FNV-1a 32-bit hash & byte-length parity check before disk commit |
| **3. Physical Disk** | `MutationTransaction` (`TransactionJournal`) | $O(1)$ per file | Exact pre-mutation byte restoration on `/rewind` or `rollback_last_mutation` |
| **4. Visual Display** | `DisplaySnapshot` (`VirtualDisplayState`) | $O(1)$ (32-deep ring) | Full window, focus, element value, and cursor restoration (`undo`/`redo`) |

Furthermore, [`BehavioralKnowledgeStore::reconcile_state`](../../src/infrastructure/behavioral_store.rs) detects external environmental drift ($\tau_{\text{after}} > \tau_{\text{checkpoint}} + 1$ or unexpected frame hash divergence) so the agent never acts on stale temporal assumptions.

---

## 5. Pillar IV: `BROCCOLI` — Persistent Knowledge, Memory & Historical Evolution

Implemented in [`src/infrastructure/behavioral_store.rs`](../../src/infrastructure/behavioral_store.rs), [`src/domain/types/learned_knowledge_types.rs`](../../src/domain/types/learned_knowledge_types.rs), [`src/domain/memory.rs`](../../src/domain/memory.rs), and [`src/domain/skill_tree.rs`](../../src/domain/skill_tree.rs), `BROCCOLI` transforms transient operational experience into durable institutional wisdom:

### 5.1 Empirical Confidence Calibration
Every interaction pattern $p \in \mathcal{K}_{\text{BROCCOLI}}$ tracks empirical successes $S_p$ and failures $F_p$ across layout epochs, calibrating confidence via Laplace-smoothed Bayesian updating:

$$C(p) = \text{clamp}\left(\frac{S_p + 1}{S_p + F_p + 2}, \ 0.10, \ 0.99\right)$$

### 5.2 Procedural Synthesis & Evolutionary Skill Tree
When the closed-loop orchestrator discovers a multi-step recovery sequence (such as auto-focusing an unfocused input before typing, or dismissing a blocking modal dialog with `Escape` before clicking an underlying control), `BROCCOLI` synthesizes a reusable [`LearnedWorkflowProcedure`](../../src/domain/types/learned_knowledge_types.rs) and synchronizes high-confidence patterns directly into the hybrid TF-IDF [`MemoryStore`](../../src/domain/memory.rs). Simultaneously, [`SkillTreeCatalog`](../../src/domain/skill_tree.rs) maintains a Kahn-verified topological DAG of reusable agent skills and `SoulProfile` personas.

---

## 6. Pillar V: `SYNTHETIC SWARM` — Distributed Collective Intelligence

Implemented in [`src/core/computer_use_orchestrator.rs`](../../src/core/computer_use_orchestrator.rs) and [`src/domain/swarm.rs`](../../src/domain/swarm.rs), `SYNTHETIC SWARM` operates at two complementary scales:

1. **Micro-Scale Cognitive Consultation (`ISwarmIntelligenceConsultant`)**:
   When the embodied agent encounters ambiguity, modal traps, or prior action failure, it convenes five specialized cognitive lenses in parallel:
   - `VisualPerceiver`: Evaluates target density and Set-of-Marks visual salience.
   - `StructuralAnalyst`: Inspects `SPIDER` hierarchy and modal focus containment.
   - `WorkflowNavigator`: Synthesizes next-step modality from `BROCCOLI` procedures and task intent.
   - `RiskAssessor`: Enforces safety fences against destructive operations.
   - `RecoverySpecialist`: Formulates delivery-mode escalation (`background` $\to$ `foreground`) and keyboard fallbacks.
   - **Grounding Sovereignty (`ConflictResolver`)**: If historical patterns in `BROCCOLI` contradict live environmental observations (e.g., a newly appeared modal dialog), live reality takes strict precedence over historical memory.
2. **Macro-Scale Task Formation (`The Legion`)**:
   Complex engineering objectives are decomposed by [`classify_task_for_legion`](../../src/domain/swarm.rs) across 7 specialist personas (`legion_architect`, `legion_researcher`, `legion_implementer`, `legion_ux_ergonomics`, `legion_hardener`, `legion_data_steward`, `legion_verifier`) executing in topological dependency waves.

---

## 7. Pillar VI: `COMPUTER USE` — Embodied Agency & Double-Wrapped Spatial Mediation

Implemented in [`src/domain/spatial_mediator.rs`](../../src/domain/spatial_mediator.rs), [`src/core/spatial_engine.rs`](../../src/core/spatial_engine.rs), and [`src/core/computer_use_orchestrator.rs`](../../src/core/computer_use_orchestrator.rs), `COMPUTER USE` replaces blind coordinate guessing with **Double-Wrapped Spatial Action Mediation**:

### 7.1 Wrapper A — The Spatial Grounding Gate ($\le 0.40\ \mu\text{s}$)
Before any pointer or keyboard event is authorized:
1. **Stale-Epoch Fence**: Rejects any contract whose `expected_epoch` lags the mediator's `layout_epoch`.
2. **Multi-Modal Target Resolution**: Resolves targets by `som_id`, frame-bound token (`s{hex}:{id}`), semantic label, or normalized coordinates (`screen`, `normalized_1000`, percentage), grounding targetless keyboard and scroll actions to the active focus or window plane.
3. **Exact 2D Sweep-Line Occlusion Union**: Computes exact union area $\bigcup_i (R_{\text{target}} \cap R_{\text{occluder}, i})$ across higher z-order windows and dialogs (ignoring non-blocking transparent overlays), rejecting targets whose occlusion exceeds `max_occlusion`.
4. **Viewport Visibility & Automatic Off-Screen Pre-Scroll**: Computes viewport visibility ratio; if an element lies off-screen ($\text{visibility} \le 0.05$), [`SpatialInteractionEngine::execute_spatially_grounded_action`](../../src/core/spatial_engine.rs) automatically dispatches a centering scroll delta $(\Delta x, \Delta y)$, refreshes the scene, and re-grounds the target in-viewport.
5. **15% Inset Safe Point**: Computes a deterministic interior click point $P_{\text{safe}}$ inset by $15\%$ from all bounding-box edges.

### 7.2 Wrapper B — The Execution Assurance Gate
1. **Pre-Execution Role & Enabled Invariants**: Blocks interaction with disabled controls (`SpatialError::TargetDisabled`), prevents typing into non-input roles (`ActionNotPermittedForRole`), and verifies $P_{\text{safe}} \in R_{\text{target}}$.
2. **Post-Execution Observable Verification**: Compares pre- and post-action `SpatialScene` snapshots (`focus` shifts, `value` mutations, `frame_seq` transitions, and `scroll_offset` deltas) to emit a cryptographic [`SpatialExecutionReceipt`](../../src/domain/spatial_action.rs) (`Confirmed` vs. `SuspectedNoop`).

---

## 8. Pillar VII: `THE HARNESS` — The Sovereign Computational Substrate

`THE HARNESS` binds all six cognitive pillars into a single, zero-entropy execution fabric:
- **DAG Wave Scheduler ([`ToolRegistry`](../../src/core/tool_registry.rs))**: Executes read-only tools in parallel waves and mutating tools behind exclusive barriers across 29 built-in tools.
- **Token-Budget Governor & Loop Breaker ([`tool_governor.rs`](../../src/core/tool_governor.rs))**: Enforces 65%/35% head/tail output budgeting with disk spill recovery (`retrieve_spill_content`) and breaks repetitive tool loops.
- **Zero-Trust Security & Ergonomic Defenses ([`secret_redactor.rs`](../../src/plumbing/secret_redactor.rs), [`vision_and_dom.rs`](../../src/plumbing/vision_and_dom.rs))**: Blocks SSRF/IMDSv2/IPv4-mapped-IPv6 navigation, sanitizes Trojan Source bidirectional Unicode, redacts 8 families of credentials, and enforces `ADR-0015`/`ADR-0016` VFS tool ergonomics.
- **Native macOS AppKit Command Station ([`native/`](../../native/))**: Pure AppKit desktop IDE (`HEAV3NS-CODE`) with turn scrubbers, docked non-blocking approvals, Unicode-safe intra-line diffing, Swarm Radar, and Multi-Project Pods.

---

## 9. Aspirational Grand Challenges

To carry **HEAV3NS** from its current verified state (`v0.1.0`) to the ultimate realization of **The Ascendant Computational Civilization (`v1.0.0`)**, we define five Grand Challenges:

1. **GC-1: Zero-Copy Kernel & GPU Perception (`SPIDER` $\times$ `COMPUTER USE`)**: Direct `ScreenCaptureKit` + `AXUIElement` + Metal shader Set-of-Marks synthesis operating at 60 Hz (`< 16.6 ms` full-screen perception-to-grounding latency).
2. **GC-2: Causal Code-to-Pixel Bi-Directional Tracing (`SPIDER`)**: Map every interactive `SpatialTarget` on screen to the exact AST node and source file in the workspace that renders it, enabling one-click visual-to-code refactoring.
3. **GC-3: Universal Branching Time-Travel (`REWIND`)**: Instantaneous $O(1)$ fork-and-compare of entire agent execution universes (memory slab + VFS overlays + virtual display state) so `SYNTHETIC SWARM` specialists can speculatively execute competing hypotheses in parallel and commit only the verified winner.
4. **GC-4: Autonomous Skill Auto-Distillation (`BROCCOLI`)**: Zero-human-intervention distillation of successful multi-step error recoveries into permanent, regression-tested `SkillNode` entries in the evolutionary DAG.
5. **GC-5: Self-Sovereign Architectural Homeostasis (`THE HARNESS`)**: Autonomous background refactoring waves triggered whenever workspace structural entropy $E_S$ rises above `0.0050`, guaranteeing perpetual Tier 1 Optimal simplicity at arbitrary repository scale.

---

## 10. Empirical Verification

```bash
# 1. Verify all 49 Rust unit, spatial, computer-use, and substrate tests
cargo test

# 2. Verify all 311 Native macOS AppKit tests
cd native && swift test && cd ..

# 3. Verify 100% Joyzoning Stability Index & A+ Build Health
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --doctor

# 4. Verify documentation integrity and link parity
node bin/docs-check.mjs
```

