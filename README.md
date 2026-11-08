<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`)

**Hyper-Compressed, Architecturally Sovereign Rust Micro Agent Runtime & Native macOS Desktop IDE Client** — structured under the **HEAV3NS-JOY** 5-Zone Joyzoning architecture (`100%` Stability Index, `0` blocking violations, all files `< 800` lines Tier 1 Optimal).

<p align="center">
  <a href="AGENTS.md">AGENTS.md</a> ·
  <a href="SOUL.md">SOUL.md</a> ·
  <a href="docs/README.md">Documentation Hub</a> ·
  <a href="docs/PROJECT_MAP.md">Project Map</a> ·
  <a href="docs/CONSTITUTION.md">Constitution</a> ·
  <a href="VALIDATION_REPORT.md">Validation Report</a> ·
  <a href="native/PRODUCT.md">Native Product Spec</a>
</p>

---

## Architecture Overview

`HEAV3NS-MICRO` provides a complete, dual-layer autonomous coding system:
1. **Rust Micro-Substrate (`src/`)**: A hyper-compressed, deterministic agent execution core with zero GC overhead, transactional VFS, self-healing tool parser, wave scheduler, and stdio JSON-RPC worker.
2. **Native macOS Desktop IDE (`native/`)**: An AppKit-native developer control station featuring live multi-agent swarm oversight, dynamic file/diff inspection, conversation memory, turn scrubbers, and Git checkpoint recovery.

### 5-Zone Joyzoning Architecture

Every source file belongs to one of five strictly enforced architectural zones, verified by `joy-check --theme strict`:

| Zone | Path | Responsibility & Invariants |
| :--- | :--- | :--- |
| **`PLUMBING`** | [`src/plumbing/`](src/plumbing/mod.rs), `native/...` | Stateless leaf utilities (`fnv1a_32`, `levenshtein`, `estimate_text_tokens`, `extract_json_roots`, `repair_json_string`, `encode_base64`, `DeterministicSecretRedactor`, `SafetyPolicy`, `DeterministicPatchEngine`). **0 upward imports, 0 mutable state (`SPI-205`).** |
| **`DOMAIN`** | [`src/domain/`](src/domain/mod.rs) | Pure business entities and formal `I<Name>` traits ([`interfaces/`](src/domain/interfaces/mod.rs)), value types ([`types/`](src/domain/types/mod.rs)), 16-MiB [`ArenaAllocator`](src/domain/arena.rs), [`MemoryStore`](src/domain/memory.rs), [`CompactionPolicy`](src/domain/compaction.rs), [`SwarmAssignment`](src/domain/swarm.rs), and [`ToolCallArgParser`](src/domain/tool_parser.rs). **0% I/O (`SPI-206`), `<= 3` structs/file (`SPI-203`).** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](src/infrastructure/mod.rs) | Concrete I/O adapters: [`HttpProvider`](src/infrastructure/http_provider.rs), [`ScriptedProvider`](src/infrastructure/scripted_provider.rs), [`SessionVfs`](src/infrastructure/vfs_store.rs), [`SkillsIngestor`](src/infrastructure/vfs_store.rs), [`FileSessionPersistence`](src/infrastructure/vfs_store.rs), [`TransactionJournal`](src/infrastructure/transaction_journal.rs), [`execute_shell_command`](src/infrastructure/shell_runner.rs), [`dispatch_fs_tool`](src/infrastructure/fs_tools.rs), and [`search_tools`](src/infrastructure/search_tools.rs). |
| **`CORE`** | [`src/core/`](src/core/mod.rs) | Use-case orchestration: [`AgentEngine`](src/core/engine.rs), [`slash_and_swarm`](src/core/slash_and_swarm.rs), [`SessionStore`](src/core/session_store.rs), [`SessionCompactor`](src/core/session_store.rs), [`ToolRegistry`](src/core/tool_registry.rs), [`tool_specs`](src/core/tool_specs.rs), [`ToolOutputGovernor`](src/core/tool_governor.rs), [`RuntimeRequestQueue`](src/core/request_queue.rs), [`flight_runner`](src/core/flight_runner.rs), and [`substrate_tests`](src/core/substrate_tests.rs). |
| **`UI`** | [`src/ui/`](src/ui/mod.rs), [`native/`](native/), Composition Roots | CLI presenter, stdio JSONL worker, desktop RPC dispatcher (`rpc_dispatch.rs`), AppKit Native Desktop IDE client (`native/Sources/Heav3nsCode/`), and Composition Roots ([`lib.rs`](src/lib.rs), [`main.rs`](src/main.rs)). |

---

## Native macOS Desktop Integration (`native/`)

The native AppKit client communicates directly with the Rust substrate via a bidirectional newline-delimited JSON-RPC bridge over stdio:

```
┌────────────────────────────────────────────────────────┐
│           Native macOS Client (AppKit / Swift)         │
│  - WorkspaceWindow (Lifecycle, Stored Properties)      │
│  - WorkspaceWindow+Navigation (Chats, Files, Diffs)    │
│  - WorkspaceWindow+Inspection (State, Spider, Memory)  │
│  - WorkspaceWindow+Execution (Drafts, Queue, Approvals)│
│  - WorkspaceWindow+Swarm (Specialist Formations)       │
│  - WorkspaceWindow+Events (Event Loop & Pods)          │
│  - RuntimeClient (ControlFrameReader / JSON-RPC)       │
└───────────────────────────┬────────────────────────────┘
                            │ stdio JSON-RPC (v: 1)
                            ▼
┌────────────────────────────────────────────────────────┐
│             heav3ns-micro Rust Substrate               │
│  - src/ui/stdio_worker.rs (Stream Framing & Handshake) │
│  - src/ui/rpc_dispatch.rs (Method Router & Handlers)   │
│  - src/core/engine.rs (ReAct Loop & Swarm Execution)   │
│  - src/domain/arena.rs (16-MiB Slab & Snapshots)       │
│  - src/infrastructure/vfs_store.rs (Transactional VFS) │
└────────────────────────────────────────────────────────┘
```

### Supported Desktop RPC Methods
- `initialize` / `snapshot`: Session handshake, capability negotiation, and state recovery.
- `task.submit` / `task.stop`: Autonomous instruction submission and safe interruption.
- `files.list` / `files.read` / `files.readBinary`: Safe workspace exploration and Base64 binary asset streaming.
- `changes.read`: Unified diffs of staged and working tree modifications.
- `spider.check`: Static syntax, local imports, and cyclic dependency audits.
- `memory.list` / `memory.save` / `memory.remove`: Persistent conversation memory inspection and modification.
- `swarm.plan` / `swarm.next` / `swarm.report`: Multi-specialist topological swarm orchestration.
- `checkpoint.list` / `checkpoint.restore`: Local Git checkpoint examination and rollback.

---

## Core Capabilities

1. **Arena Slab Allocator (`src/domain/arena.rs`)**: 16-MiB 4-byte aligned bump-pointer slab with $O(1)$ snapshot rewind (`SlabBufferSnapshot`), zero GC pause, and deterministic JSONL session persistence.
2. **Transactional Virtual File System (`src/infrastructure/vfs_store.rs`)**: FNV-1a + byte-length disk baseline capture (`VfsDiskBaseline`), conflict detection against external edits before commit, and atomic multi-state snapshot rewind.
3. **Self-Healing Tool Argument Parser (`src/domain/tool_parser.rs` & `src/plumbing/json_repair.rs`)**: 3-stage recovery (Fast-Path JSON -> Heuristic repair -> Key-Value fallback), 60+ parameter alias canonicalizations, and fuzzy argument healing.
4. **23 Native Built-In Tools & Wave Scheduler (`src/core/tool_registry.rs`)**: Parallel read waves (`tokio::JoinSet`), exclusive write barriers, V4A multi-file patches (`*** Begin Patch`), and atomic transaction journal rollbacks.
5. **Topological Multi-Agent Swarm (`src/core/slash_and_swarm.rs`)**: Deterministic task decomposition into specialist formation graphs with non-blocking status reporting and auto-reconciliation.
6. **Native Desktop Windowing (`native/`)**: Modular AppKit window architecture with turn scrubbers, action palette (`Cmd+K`), quick open (`Cmd+P`), and multi-project Pod monitoring.

---

## Quick Start & Verification

```bash
# 1. Run Rust unit & integration test suites (8 suites)
cargo test

# 2. Run Native macOS AppKit test suite (308 tests)
cd native && swift test && cd ..

# 3. Build optimized release binary
cargo build --release

# 4. Run the 5-scenario Agent Flight Test suite (FT-01..FT-05)
./target/release/heav3ns-micro --flight

# 5. Run substrate micro-benchmarks + smoke self-check
./target/release/heav3ns-micro --bench
./target/release/heav3ns-micro --smoke

# 6. Run strict Joyzoning architectural audit
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
```
