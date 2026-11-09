<!-- [LAYER: UI] -->
# Mental Models & Visual Analogies

<p align="center">
  <a href="README.md">Wiki Hub</a> ·
  <a href="../../doc.md">Sovereign Charter (doc.md)</a> ·
  <a href="../README.md">Docs Hub</a> ·
  <a href="../CONSTITUTION.md">Constitution</a> ·
  <a href="rust-substrate-architecture.md">Substrate Architecture</a>
</p>

To reason accurately about **HEAV3NS-MICRO** and **[`HEAV3NS — The Sovereign Intelligence Architecture`](../../doc.md)**, humans and AI coding agents should internalize these seven core mental models:

---

## 1. The Arena Slab & Watermark Rewind (`REWIND`)

Think of the runtime's memory not as a chaotic ocean of scattered heap allocations, but as a clean, pre-carved **16-MiB notebook** ([`ArenaAllocator`](../../src/domain/arena.rs)):
- When an agent tick begins, the runtime places a bookmark (`SlabBufferSnapshot`) on the current page.
- As the model streams tokens, parses ASTs, and manipulates JSON fragments, it writes linearly down the page (bump-pointer allocation).
- When the turn concludes and permanent state is saved, the runtime snaps the bookmark back to the turn boundary ($O(1)$ rewind).

---

## 2. The Two-Phase VFS Reality Shield (`REWIND`)

When an agent modifies code on disk, it never writes directly to the underlying filesystem. Instead, it enters an **airlock** ([`SessionVfs`](../../src/infrastructure/vfs_store.rs)):
- **Phase 1 (Staging)**: The agent creates an in-memory overlay and captures an FNV-1a 32-bit hash and byte length fingerprint (`VfsDiskBaseline`) of the disk file *before* modification.
- **Phase 2 (Commit Verification)**: Before flushing the overlay to disk, the runtime re-verifies the disk fingerprint. If a human developer edited the file in their IDE while the agent was thinking, the airlock aborts the commit safely.

---

## 3. The Wavefront (`THE HARNESS`)

When an agent emits multiple tool calls across our 29 native tools in one turn, [`ToolRegistry::partition_waves`](../../src/core/tool_registry.rs) creates an execution wavefront:
- **Calm Waters (Parallel Reads)**: Read-only tools (`view_file`, `grep_search`, `search_symbols`) execute concurrently across asynchronous worker futures.
- **The Floodgate (Mutating Barrier)**: Mutating operations (`write_file`, `run_command`, `computer_use`, `spatial_interact`) execute in deterministic serialized waves.

---

## 4. The Self-Healing Bracket Balancer (`HEAV3NS-MICRO`)

LLMs are probabilistic token generators, not strict JSON serializers. Under token limits, models frequently omit closing braces (`}`) or wrap output in Markdown fences (` ```json `).
- [`ToolCallArgParser`](../../src/domain/tool_parser.rs) and [`json_repair.rs`](../../src/plumbing/json_repair.rs) strip code fences, track delimiter depth outside string literals, repair unescaped newlines, and canonicalize 60+ parameter aliases.

---

## 5. The Double-Wrapped Spatial Gate (`COMPUTER USE`)

Embodied GUI and DOM interaction is never an unverified `(x, y)` leap of faith. Every action passes through two independent security checkpoints in [`SpatialActionMediator`](../../src/domain/spatial_mediator.rs):
- **Wrapper A (The Grounding Gate)**: Verifies that the layout epoch is fresh, computes the exact 2D sweep-line union area of any higher z-order occluding dialogs, automatically scrolls off-screen targets into the viewport (`requires_scroll`), and calculates a 15% inset interior safe point in $\le 0.40\ \mu\text{s}$.
- **Wrapper B (The Assurance Gate)**: Blocks clicks on disabled elements (`TargetDisabled`), prevents typing into non-input controls (`ActionNotPermittedForRole`), and compares before-and-after scene snapshots (`focus`, `value`, `frame_seq`, `scroll_offset`) to confirm the action actually took effect.

---

## 6. The `SPIDER` $\times$ `BROCCOLI` $\times$ `SWARM` Cybernetic Loop

Intelligence emerges from the interplay of perception, memory, and collective deliberation:
- **`SPIDER`** perceives the structural hierarchy of the screen (modal traps, regions, affordances) and codebase.
- **`BROCCOLI`** remembers which interaction patterns and multi-step recovery procedures (`LearnedWorkflowProcedure`) have succeeded historically.
- **`SYNTHETIC SWARM`** convenes 5 specialist lenses (`VisualPerceiver`, `StructuralAnalyst`, `WorkflowNavigator`, `RiskAssessor`, `RecoverySpecialist`) when obstacles arise, while **`ConflictResolver`** guarantees **Grounding Sovereignty**: live observation always overrides stale memory.

---

## 7. The Five Sovereign Rooms (Joyzoning)

The repository is partitioned into five rooms with strict one-way doors:
1. **`PLUMBING`** (`10` modules): Stateless leaf algorithms. Reaches into nothing.
2. **`DOMAIN`** (`25` modules): Pure entities, behavioral models (`UiElement`, `VirtualWindow`), and `39` formal `I<Name>` traits. Zero I/O.
3. **`INFRASTRUCTURE`** (`9` modules): Concrete HTTP, VFS, Journal, and Broccoli Store adapters.
4. **`CORE`** (`14` modules): Orchestrators (`AgentEngine`, `ComputerUseOrchestrator`, `SpatialInteractionEngine`, `ToolRegistry`).
5. **`UI`** (`6` Rust modules + `native/` AppKit IDE): CLI, `"v": 1` stdio JSON-RPC worker, and `HEAV3NS-CODE` desktop station.
