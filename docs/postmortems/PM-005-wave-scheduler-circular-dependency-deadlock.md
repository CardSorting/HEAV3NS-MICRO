<!-- [LAYER: UI] -->
# PM-005: Wave Scheduler Circular Dependency Deadlock

- **Incident Date**: 2026-02-22
- **Severity**: SEV-2
- **Subsystem**: `WaveScheduler` (`src/core/tool_governor.rs`)
- **Related ADR**: [ADR-0005](../adr/0005-dag-wave-scheduler-concurrency.md)
- **Resolved By**: Strict topological partition between read waves and write barriers

---

## Summary

When an LLM emitted a composite tool call sequence where a read operation depended on the output of an uncommitted file write staged in the same turn, the scheduler stalled in an unresolved barrier wait.

---

## Root cause

1. **Why did the scheduler stall?** It awaited completion of a write tool while holding read channels open.
2. **Why was there a circular wait?** The scheduler attempted to dynamically order tools based on implicit file paths rather than a strict static read/write partition.
3. **Why wasn't the dependency resolved?** Staged VFS overlays are not visible to standard external processes until committed.

---

## Action items

1. **Enforce static wave partitioning**: Group all read tools into Wave 1 (parallel execution) and all mutating tools into Wave 2 (serialized execution).
2. **Add unit test**: Add `test_wave_scheduler_partitioning`.
3. **Document in Architecture**: Update [Tool System & Wavefront](../wiki/tool-system-and-execution-wavefront.md).

---

## Lessons learned

Dynamic runtime dependency resolution between agent tool calls introduces deadlock risks. Static two-wave partitioning (parallel reads, followed by serialized writes) provides determinism and safety.
