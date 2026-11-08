<!-- [LAYER: UI] -->
# ADR-0005: Topological Wave Scheduling for Concurrent Read/Write Tool Waves

- **Status**: Accepted
- **Date**: 2026-02-05
- **Deciders**: HEAV3NS Core Architecture Team
- **Technical Story**: [`src/core/tool_governor.rs`](../../src/core/tool_governor.rs) · [`docs/PORTABLE_TRINITY.md`](../PORTABLE_TRINITY.md)

---

## Status

Accepted.

## Context

When an LLM outputs multiple tool calls in a single turn (e.g., three grep searches, two file reads, and a file write), executing them sequentially wastes seconds of clock time. However, running all tools concurrently risks race conditions where reads observe partially written file overlays or two writes mutate the same file concurrently.

### Decision drivers

- Parallel execution of side-effect-free read operations.
- Deterministic, race-free serialization of mutating actions.
- Enforcement of session execution authority levels.

### Considered options

1. **Strictly Sequential Execution**: Safe but slow; does not leverage multi-core async parallelism.
2. **Unconstrained Concurrency**: High performance but introduces disk and memory race conditions.
3. **Topological Wave Partitioning (DAG Wave Scheduler)**: Partition tool calls into parallel read waves followed by serialized mutating waves.

## Decision

We chose **Option 3: Topological Wave Scheduling** implemented in [`src/core/tool_governor.rs`](../../src/core/tool_governor.rs):
- Built-in tools are statically categorized into `ReadOnly` or `Mutating`.
- Safe read tools (`fs_read`, `search_grep`, `git_status`) execute in parallel using asynchronous worker tasks.
- Mutating tools (`fs_write`, `patch_apply`, `shell_execute`) execute sequentially in barrier order after all read operations resolve.

## Consequences

### Positive

- Maximal read concurrency while guaranteeing total isolation for mutating writes.
- Deterministic tool execution ordering and audit trail.

### Negative / trade-offs

- Mutating tools cannot execute in parallel with other mutating tools.

## Verification

```bash
cargo test test_wave_scheduler_partitioning
```
