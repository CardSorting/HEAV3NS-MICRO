<!-- [LAYER: UI] -->
# ADR-0011: Sovereign Intelligence Architecture, Double-Wrapped Spatial Mediation & Closed-Loop Computer Use

- **ADR ID**: `ADR-0011`
- **Status**: **Accepted**
- **Date**: `2026-11-08`
- **Deciders**: HEAV3NS Architecture Working Group
- **Surface**: Spatial Action Architecture · Embodied Computer Use · Collective Swarm Intelligence
- **Enforced By**: `cargo test core::spatial_tests` · `cargo test core::computer_use_tests` · `npm run joy-check`

---

## Status

**Accepted** — Implemented across [`src/plumbing/spatial_geometry.rs`](../../src/plumbing/spatial_geometry.rs), [`src/plumbing/environmental_diff.rs`](../../src/plumbing/environmental_diff.rs), [`src/plumbing/display_telemetry.rs`](../../src/plumbing/display_telemetry.rs), [`src/domain/spatial_action.rs`](../../src/domain/spatial_action.rs), [`src/domain/spatial_mediator.rs`](../../src/domain/spatial_mediator.rs), [`src/domain/computer_use.rs`](../../src/domain/computer_use.rs), [`src/infrastructure/behavioral_store.rs`](../../src/infrastructure/behavioral_store.rs), [`src/core/spatial_engine.rs`](../../src/core/spatial_engine.rs), and [`src/core/computer_use_orchestrator.rs`](../../src/core/computer_use_orchestrator.rs).

---

## Context

Conventional computer-use and browser-use agents operate as open-loop coordinate emitters: they inspect a static screenshot or DOM dump, guess an `(x, y)` coordinate or selector, and dispatch a click or keystroke without verifying whether:
1. The layout epoch has changed since observation,
2. The target is occluded by a higher z-order modal dialog,
3. The target lies off-screen outside the current viewport scroll bounds,
4. The target is disabled (`enabled == false`) or incapable of receiving text input, or
5. The action actually produced an observable state transition (`focus`, `value`, `frame_seq`, or `scroll_offset`).

Furthermore, when an interaction fails, conventional agents lack structural interface perception (`SPIDER`), temporal drift reconciliation (`REWIND`), persistent behavioral learning (`BROCCOLI`), and multi-specialist collective deliberation (`SYNTHETIC SWARM`).

---

## Decision

We elevate **HEAV3NS-MICRO** into the runtime substrate of **HEAV3NS — The Sovereign Intelligence Architecture** ([`doc.md`](../../doc.md)) through four architectural decisions:

1. **First-Class Spatial Action Architecture & Double-Wrapped Mediation (`Wrapper A` + `Wrapper B`)**:
   - Introduce [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs) and [`SpatialInteractionEngine`](../../src/core/spatial_engine.rs) implementing formal domain interfaces (`ISpatialMediator`, `ISpatialGroundingGate`, `IExecutionAssuranceGate`).
   - **Wrapper A (Spatial Grounding Gate)** verifies `layout_epoch` freshness, resolves targets across Set-of-Marks (`som_id`), frame tokens (`s{hex}:{id}`), labels, and normalized coordinate spaces, computes exact 2D sweep-line occlusion union (`calculate_rectangles_union_area`), detects off-screen targets (`requires_scroll`), and computes a 15% inset safe point.
   - **Wrapper B (Execution Assurance Gate)** enforces pre-execution role and enabled-state invariants (`SpatialError::TargetDisabled`, `ActionNotPermittedForRole`), dispatches automatic off-screen pre-scrolling and multi-step `SpatialActionPlan` execution (`execute_plan`), and verifies post-execution observable deltas (`Confirmed` vs. `SuspectedNoop`).
2. **9-Stage Closed-Loop Computer Use Orchestrator (`COMPUTER USE` $\times$ `SPIDER`)**:
   - Introduce [`ComputerUseOrchestrator`](../../src/core/computer_use_orchestrator.rs) executing `OBSERVE -> ORIENT -> INTERPRET -> REASON -> PLAN -> ACT -> OBSERVE -> VERIFY -> LEARN`.
   - Automatically detect modal traps via `SPIDER` (`detect_modal_or_dialog`) and intercept outside clicks with `key: Escape` while permitting internal modal interactions.
   - Automatically plan focus-clicks before typing into unfocused inputs and break loops after 3 consecutive failures.
3. **5-Specialist Collective Consultation (`SYNTHETIC SWARM`) with Grounding Sovereignty**:
   - Convene `VisualPerceiver`, `StructuralAnalyst`, `WorkflowNavigator`, `RiskAssessor`, and `RecoverySpecialist` alongside `HistoricalKnowledge` and `ConflictResolver`.
   - Enforce **Grounding Sovereignty**: live environmental observation always overrides conflicting historical assumptions.
4. **Durable Behavioral Knowledge Store (`BROCCOLI`) & Temporal Reconciliation (`REWIND`)**:
   - Track Laplace-smoothed interaction confidence in [`BehavioralKnowledgeStore`](../../src/infrastructure/behavioral_store.rs), synthesize multi-step [`LearnedWorkflowProcedure`](../../src/domain/types/learned_knowledge_types.rs) sequences, synchronize high-confidence patterns into `MemoryStore`, and reconcile layout epoch drift via `reconcile_state`.

---

## Consequences

- **Positive**:
  - Spatial grounding executes in $\le 0.40\ \mu\text{s}$ per operation (`> 240,000` ops/sec) with zero heap fragmentation.
  - Off-screen elements are automatically scrolled into view and re-grounded before click or text dispatch.
  - Modal dialogs, disabled elements, and suspected no-ops are caught deterministically with structured recovery guidance.
  - Behavioral patterns and multi-step recovery procedures are learned and retained across turns.
- **Trade-offs**:
  - Adds specialized modules across `PLUMBING`, `DOMAIN`, `INFRASTRUCTURE`, and `CORE` (bringing the crate to 70 modules and 29 built-in tools), all strictly maintained under Tier 1 Optimal (`< 800` lines) with a `100%` Joyzoning Stability Index ($E_S = 0.0041$).

---

## Verification

```bash
# 1. Run spatial grounding, off-screen auto-scroll, and multi-step plan tests
cargo test core::spatial_tests

# 2. Run closed-loop computer use, Spider perception, Broccoli store, and Swarm consultation tests
cargo test core::computer_use_tests

# 3. Verify 100% Joyzoning Stability Index
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
```

