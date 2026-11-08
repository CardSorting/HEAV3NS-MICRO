<!-- [LAYER: UI] -->
# Technical FAQ & Edge Cases

<p align="center">
  <a href="README.md">KB Hub</a> ·
  <a href="symptom-diagnosis-matrix.md">Symptom Matrix</a> ·
  <a href="../wiki/rust-substrate-architecture.md">Substrate Wiki</a> ·
  <a href="../CONSTITUTION.md">Constitution</a>
</p>

Answers to complex questions, performance edge cases, and runtime behaviors in **HEAV3NS-MICRO**.

---

## 1. Memory & Arena Allocation

### Q: Why not use a dynamic memory pool or jemalloc instead of a fixed 16-MiB slab?
**A**: Dynamic allocators introduce variable heap fragmentation and non-deterministic latency spikes during high-throughput token streaming. By fixing the slab to 16 MiB, HEAV3NS-MICRO guarantees that the process can never exhaust the host machine's memory, while providing constant-time $O(1)$ scratch memory unwinding at turn boundaries ([ADR-0002](../adr/0002-arena-allocator-and-o1-tick-rewind.md)).

### Q: What if an agent task legitimately requires reading a 50-MiB file?
**A**: Agent workflows should never load entire multi-megabyte files into dialogue context. The `fs_read` tool accepts byte offset and limit parameters, enabling targeted chunk inspection. For text analysis across large files, use `search_grep` to stream line matches without loading whole files.

---

## 2. Virtual File System & Rollback

### Q: Can multiple agents or threads stage changes to the same file simultaneously?
**A**: Overlays are staged within an isolated session context. If two parallel operations attempt to modify the same file path within a single turn, the [`WaveScheduler`](../../src/core/tool_governor.rs) serializes them into separate barrier waves. If an external process modifies the file on physical disk before commit, the FNV-1a baseline check aborts the commit ([PB-04](../playbooks/vfs-commit-conflict-and-journal-rollback.md)).

### Q: How far back can `/rewind` roll back changes?
**A**: The [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) maintains an append-only log of all commits performed during the active session. Consecutive invocations of `/rewind` walk backwards through the journal, inverting each change until the start of the session.

---

## 3. Tool Execution & Security

### Q: How do I add a new custom agent tool?
**A**:
1. Define the tool specification in [`src/core/tool_specs.rs`](../../src/core/tool_specs.rs).
2. Implement the execution logic in [`src/core/tool_registry.rs`](../../src/core/tool_registry.rs).
3. Specify its side-effect type (`ReadOnly` vs. `Mutating`) for the wave scheduler.
4. If the tool performs concrete I/O, declare a port trait in `src/domain/interfaces/mod.rs` and place the implementation in `src/infrastructure/`.

### Q: Is shell execution sandboxed?
**A**: Shell commands dispatched via `shell_execute` run under timeout supervision and require the session authority level to be set to `ShellExecute` or `FullSovereign`. Sensitive paths are guarded by `DeterministicSecretRedactor` ([ADR-0008](../adr/0008-secret-redaction-and-security-policy.md)).
