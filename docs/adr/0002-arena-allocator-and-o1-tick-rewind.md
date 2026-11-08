<!-- [LAYER: UI] -->
# ADR-0002: Contiguous 16-MiB Arena Slab & O(1) Tick Rewind

- **Status**: Accepted
- **Date**: 2026-02-02
- **Deciders**: HEAV3NS Core Architecture Team
- **Technical Story**: [`src/domain/arena.rs`](../../src/domain/arena.rs) · [`docs/PORTABLE_TRINITY.md`](../PORTABLE_TRINITY.md)

---

## Status

Accepted.

## Context

Streaming agent execution involves rapid generation and manipulation of thousands of small strings: intermediate token chunks, tool name buffers, JSON fragments, and AST symbols. Relying on default system allocators (`malloc`/`free`) causes heap fragmentation, cache thrashing, and unpredictable latency in resource-constrained environments.

### Decision drivers

- Predictable bounded memory usage.
- Sub-millisecond turn reset latency.
- Elimination of memory leaks across long-running sessions.

### Considered options

1. **Standard Rust Allocations (`Box`, `String`, `Vec`)**: Unbounded memory growth and heap fragmentation.
2. **Per-Turn Dynamic Memory Pools**: Overhead of creating and destroying pool descriptors on every turn.
3. **Contiguous 16-MiB Slab with Watermark Bump Pointer**: Pre-allocate a 16-MiB buffer once; allocate linearly; rewind pointer to checkpoint watermark at turn boundary in $O(1)$ time.

## Decision

We chose **Option 3: Contiguous 16-MiB Memory Arena with $O(1)$ Tick Rewinding** implemented in [`src/domain/arena.rs`](../../src/domain/arena.rs):
- Allocations are 8-byte aligned bump-pointer offsets.
- Scratch allocations record a turn start watermark and unwind in constant time via `rewind_to_checkpoint()`.
- Long-term memory is managed via explicit semantic compaction (`CompactionEngine`).

## Consequences

### Positive

- Zero heap fragmentation over millions of streaming tokens.
- Instant $O(1)$ memory reset between agent dialogue turns.
- Hard 16-MiB physical ceiling preventing memory exhaustion on host systems.

### Negative / trade-offs

- Turning sessions with massive raw context require proactive compaction before reaching the 16-MiB ceiling.

## Verification

```bash
cargo test test_arena_allocator_alignment_and_o1_rewind
```
