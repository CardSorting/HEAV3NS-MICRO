<!-- [LAYER: UI] -->
# ADR-0001: Five Canonical Zones & Principle of Geographic Alignment (PGA)

- **Status**: Accepted
- **Date**: 2026-02-01
- **Deciders**: HEAV3NS Core Architecture Team
- **Technical Story**: [`docs/CONSTITUTION.md`](../CONSTITUTION.md) · [`stability.config.json`](../../stability.config.json)

---

## Status

Accepted — Constitutional Law #1 and #2 of HEAV3NS-MICRO.

## Context

In complex software repositories—especially those authored and maintained by autonomous coding agents that only inspect small slices of context at a time—architectural boundaries degrade quickly:
- I/O operations (network, filesystem, shell) bleed into pure domain logic.
- Utility libraries accumulate upward dependencies into application controllers.
- UI components bypass core services to call concrete adapters directly.

### Decision drivers

- **Instant Agent Comprehension**: Any human engineer or AI coding agent must know the exact role and permitted imports of a file from its first line.
- **Deterministic Static Verification**: Boundaries must be checkable via zero-dependency static analysis.

### Considered options

1. **Unenforced Conventions**: Directory naming without automated enforcement.
2. **Layered Hexagonal Architecture (6–8 layers)**: Too many nuanced rings leading to frequent agent misclassifications.
3. **Five Canonical Joyzoning Zones with Header Tags**: `DOMAIN`, `CORE`, `INFRASTRUCTURE`, `PLUMBING`, `UI` with mandatory top-of-file comments.

## Decision

We chose **Option 3: Five Canonical Zones governed by the Principle of Geographic Alignment (PGA)**:
- `DOMAIN` (`src/domain/`): Pure business logic, state machines, and `I<Name>` port traits (0% I/O).
- `CORE` (`src/core/`): Use-case orchestration and tool governor.
- `INFRASTRUCTURE` (`src/infrastructure/`): Concrete I/O adapters implementing `domain/interfaces/I*`.
- `PLUMBING` (`src/plumbing/`): Stateless leaf primitives (0% state, 0% upward imports).
- `UI` (`src/ui/`, `src/lib.rs`, `src/main.rs`): Presenters, stdio worker, and Composition Roots.

Every file carries `// [LAYER: <ZONE>]` on line 1, matching its directory path.

## Consequences

### Positive

- Zero architectural ambiguity for both LLMs and humans.
- Continuous verification via `joy-check --theme strict` guaranteeing 100% Stability Index.

### Negative / trade-offs

- Every new file requires a header comment on line 1.

## Verification

```bash
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
```
