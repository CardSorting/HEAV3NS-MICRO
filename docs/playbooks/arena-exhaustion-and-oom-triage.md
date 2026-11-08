<!-- [LAYER: UI] -->
# Playbook PB-02: Arena Exhaustion & OOM Triage

<p align="center">
  <a href="README.md">Playbooks Hub</a> ·
  <a href="../README.md">Docs Hub</a> ·
  <a href="../adr/0002-arena-allocator-and-o1-tick-rewind.md">ADR-0002</a> ·
  <a href="../PORTABLE_TRINITY.md">Substrate Primitives</a>
</p>

## Triage at a glance

| Field | Detail |
| :--- | :--- |
| **Playbook ID** | `PB-02` |
| **Severity** | `SEV-1` (Runtime Out-Of-Memory / turn abort) |
| **Trigger Codes** | `ArenaOom`, allocation failure, arena utilization > 90% |
| **Time to Mitigate** | `2–5 minutes` |
| **First Command** | `cargo run -- -p "/status"` |
| **Rollback Safety** | Invoking `/compact` or `/clear` preserves committed file state |

---

## Remediation steps

### Case 1: Session Dialogue History Exceeding Token Budget
1. **Inspect current memory allocation**:
   Run `/status` in the REPL or view telemetry in the log output.
2. **Trigger semantic compaction**:
   Run `/compact` to invoke [`CompactionEngine::compact_session`](../../src/domain/compaction.rs). Historical messages will be summarized into compact semantic memory records, freeing arena space.
3. **Verify memory reclamation**:
   Run `/status` again to ensure arena utilization has dropped below 50%.

### Case 2: Scratch Allocation Leak Across Turns
1. If the arena watermark does not reset after a turn completes:
   - Check that `rewind_to_checkpoint()` is properly called at the end of the turn in `src/core/engine.rs`.
   - Ensure scratch buffers are not accidentally stored in long-lived session vectors.

### Case 3: Workspace Massive File Ingestion
1. If an agent attempts to load an entire massive binary or multi-megabyte log file into memory:
   - Use `fs_read` with range offsets rather than loading the entire file.
   - For search operations, prefer `search_grep` over reading entire file trees into memory.

---

## Verification

```bash
cargo test test_arena_allocator_alignment_and_o1_rewind
cargo run -- --smoke
```
