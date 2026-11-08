<!-- [LAYER: UI] -->
# PM-001: Arena Allocator OOM on Unbounded Streaming Tokens

- **Incident Date**: 2026-02-10
- **Severity**: SEV-1
- **Subsystem**: `ArenaAllocator` (`src/domain/arena.rs`)
- **Related ADR**: [ADR-0002](../adr/0002-arena-allocator-and-o1-tick-rewind.md)
- **Resolved By**: Dynamic turn compaction in [`src/domain/compaction.rs`](../../src/domain/compaction.rs)

---

## Summary

During a 45-turn long-running autonomous refactoring session, the agent runtime panicked with `ArenaOom`. The entire 16-MiB contiguous memory slab was consumed because dialogue history was accumulating scratch token vectors without turn-level summarization.

---

## Root cause

1. **Why did the runtime crash?** The arena allocator exceeded its 16-MiB capacity ceiling.
2. **Why was memory exhausted?** Over 40 dialogue turns containing thousands of source code lines were retained in raw uncompressed strings.
3. **Why weren't prior turns freed?** The engine only unwound scratch allocations made within a single turn, retaining prior turn payloads in persistent session storage.
4. **Why didn't compaction trigger?** Compaction was purely manual via `/compact` rather than automatically triggered by arena watermarks.

---

## Action items

1. **Implement dynamic watermark threshold**: If arena utilization exceeds 75%, trigger [`CompactionEngine::compact_session`](../../src/domain/compaction.rs).
2. **Add unit test**: Add `test_arena_allocator_alignment_and_o1_rewind` in `src/core/substrate_tests.rs`.
3. **Document in Playbook**: Create [PB-02: Arena Exhaustion & OOM Triage](../playbooks/arena-exhaustion-and-oom-triage.md).

---

## Lessons learned

A static bump allocator requires automatic semantic compaction when operating under long-running dialogue loops. Without an automatic compaction threshold, bounded memory guarantees become denial-of-service risks.
