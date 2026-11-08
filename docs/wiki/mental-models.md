<!-- [LAYER: UI] -->
# Mental Models & Visual Analogies

<p align="center">
  <a href="README.md">Wiki Hub</a> ·
  <a href="../README.md">Docs Hub</a> ·
  <a href="../CONSTITUTION.md">Constitution</a> ·
  <a href="rust-substrate-architecture.md">Substrate Architecture</a>
</p>

To reason accurately about **HEAV3NS-MICRO**, humans and AI coding agents should internalize these five core mental models:

---

## 1. The Arena Slab & Watermark Rewind

Think of the runtime's memory not as a chaotic ocean of scattered heap allocations, but as a clean, pre-carved **16-MiB notebook**.
- When an agent tick begins, the runtime places a bookmark (watermark pointer) on the current page.
- As the model streams tokens, parses ASTs, and manipulates JSON fragments, it writes linearly down the page (bump-pointer allocation).
- When the turn concludes and permanent state is saved, the runtime simply snaps the bookmark back to the turn boundary ($O(1)$ rewind).
- In a single clock cycle, thousands of transient allocations evaporate without triggering garbage collection, memory leaks, or slab fragmentation.

---

## 2. The Two-Phase VFS Reality Shield

When an agent decides to modify code on disk, it never writes directly to the underlying filesystem. Instead, it enters an **airlock** ([`SessionVfs`](../../src/infrastructure/vfs_store.rs)):
- **Phase 1 (Staging)**: The agent creates an in-memory overlay. Crucially, the airlock takes a cryptographic fingerprint (FNV-1a 32-bit hash and byte count) of the disk file *before* modification.
- **Phase 2 (Commit Verification)**: Before flushing the overlay to disk, the runtime checks the file's current disk fingerprint. If a human developer edited the file in their editor while the agent was thinking, the airlock shuts and halts the commit.
- Disk state is never clobbered blindly.

---

## 3. The Wavefront (Concurrent Reads, Serialized Writes)

When an agent emits multiple tool calls in one turn, it creates an execution wavefront:
- **Calm Waters (Parallel Reads)**: Inspecting the workspace (`fs_read`, `search_grep`, `git_status`) produces zero system side-effects. All read tools are dispatched simultaneously across asynchronous worker threads.
- **The Floodgate (Mutating Barrier)**: Once all read operations finish and return their outputs, the wavefront closes. Mutating operations (`fs_write`, `patch_apply`, `shell_execute`) execute one by one in deterministic sequential order.

---

## 4. The Self-Healing Bracket Balancer

LLMs are probabilistic token generators, not strict JSON serializers. Under token limits or network interruptions, models frequently omit closing braces (`}`) or wrap output in Markdown fences (` ```json `).
- Instead of crashing and failing the turn, the parser acts as an **orthopedic splint**:
- It strips code fences, counts open vs. closed bracket depths, escapes unescaped interior quotes, and repairs the payload into valid JSON before passing it to deserialization.

---

## 5. The Five Sovereign Rooms (Joyzoning)

The repository is partitioned into five distinct rooms with strict one-way access doors:
1. **`PLUMBING`**: The tool shed. Contains stateless leaf tools (hashers, redactors, parsers). Reaches into nothing.
2. **`DOMAIN`**: The inner sanctum. Contains pure business logic, traits, and memory models. Never makes I/O calls.
3. **`INFRASTRUCTURE`**: The loading dock. Contains concrete HTTP clients, disk adapters, and shell runners.
4. **`CORE`**: The control center. Orchestrates use cases and tool scheduling. Depends only on domain contracts.
5. **`UI`**: The reception desk. Interacts with the user via CLI and stdio.
