<!-- [LAYER: UI] -->
# HEAV3NS-MICRO (`v0.1.0`)

**Hyper-Compressed, Architecturally Sovereign Rust Micro Agent Runtime** — distilled from `LUMI-NEW` and structured under the **HEAV3NS-JOY** 5-Zone Joyzoning architecture (`100%` Stability Index, `100%` Build Health `A+`, `0` violations).

<p align="center">
  <a href="AGENTS.md">AGENTS.md</a> ·
  <a href="SOUL.md">SOUL.md</a> ·
  <a href="docs/README.md">Documentation Hub</a> ·
  <a href="docs/PROJECT_MAP.md">Project Map</a> ·
  <a href="docs/CONSTITUTION.md">Constitution</a> ·
  <a href="VALIDATION_REPORT.md">Validation Report</a>
</p>

---

## 5-Zone Joyzoning Architecture

Every source file in `src/` belongs to one of five strictly enforced architectural zones, verified by `joy-check --theme strict` (`0` violations, all modules `< 800` lines Tier 1 Optimal):

| Zone | Path | Files | Core Responsibility & Invariants |
| :--- | :--- | :---: | :--- |
| **`PLUMBING`** | [`src/plumbing/`](src/plumbing/mod.rs) | 6 | Stateless leaf utilities (`fnv1a_32`, `levenshtein`, `estimate_text_tokens`, `extract_json_roots`, `repair_json_string`, `DeterministicSecretRedactor`, `SafetyPolicy`, `DeterministicPatchEngine`). **0 upward imports, 0 mutable state (`SPI-205`).** |
| **`DOMAIN`** | [`src/domain/`](src/domain/mod.rs) | 16 | Pure domain entities, formal `I<Name>` traits ([`src/domain/interfaces/mod.rs`](src/domain/interfaces/mod.rs)), value types ([`src/domain/types/`](src/domain/types/mod.rs)), 16-MiB [`ArenaAllocator`](src/domain/arena.rs), [`MemoryStore`](src/domain/memory.rs), [`CompactionPolicy`](src/domain/compaction.rs), [`SwarmAssignment`](src/domain/swarm.rs), and [`ToolCallArgParser`](src/domain/tool_parser.rs). **0% I/O (`SPI-206`), `<= 3` structs/file (`SPI-203`).** |
| **`INFRASTRUCTURE`** | [`src/infrastructure/`](src/infrastructure/mod.rs) | 8 | Concrete I/O adapters implementing `domain/interfaces`: [`HttpProvider`](src/infrastructure/http_provider.rs), [`ScriptedProvider`](src/infrastructure/scripted_provider.rs), [`SessionVfs`](src/infrastructure/vfs_store.rs), [`SkillsIngestor`](src/infrastructure/vfs_store.rs), [`FileSessionPersistence`](src/infrastructure/vfs_store.rs), [`TransactionJournal`](src/infrastructure/transaction_journal.rs), [`execute_shell_command`](src/infrastructure/shell_runner.rs), [`dispatch_fs_tool`](src/infrastructure/fs_tools.rs), and [`search_tools`](src/infrastructure/search_tools.rs). |
| **`CORE`** | [`src/core/`](src/core/mod.rs) | 10 | Use-case orchestration: [`AgentEngine`](src/core/engine.rs), [`slash_and_swarm`](src/core/slash_and_swarm.rs), [`SessionStore`](src/core/session_store.rs), [`SessionCompactor`](src/core/session_store.rs), [`ToolRegistry`](src/core/tool_registry.rs), [`tool_specs`](src/core/tool_specs.rs), [`ToolOutputGovernor`](src/core/tool_governor.rs), [`RuntimeRequestQueue`](src/core/request_queue.rs), [`flight_runner`](src/core/flight_runner.rs), and [`substrate_tests`](src/core/substrate_tests.rs). |
| **`UI`** | [`src/ui/`](src/ui/mod.rs), [`src/lib.rs`](src/lib.rs), [`src/main.rs`](src/main.rs) | 4 | CLI argument parser, interactive REPL, headless JSONL stdio worker ([`cli_presenter.rs`](src/ui/cli_presenter.rs)), and Composition Roots ([`lib.rs`](src/lib.rs), [`main.rs`](src/main.rs)). |

---

## Core Capabilities

1. **Arena Slab Allocator (`src/domain/arena.rs`)**: 16-MiB 4-byte aligned bump-pointer slab with $O(1)$ snapshot rewind (`SlabBufferSnapshot`), zero GC pause, and deterministic JSONL session persistence (`src/core/session_store.rs`).
2. **Transactional Virtual File System (`src/infrastructure/vfs_store.rs`)**: FNV-1a + byte-length disk baseline capture (`VfsDiskBaseline`), conflict detection against external edits before commit, and atomic multi-state snapshot rewind across transcript, VFS, memory, and Arena slab.
3. **Self-Healing Tool Argument Parser (`src/domain/tool_parser.rs` & `src/plumbing/json_repair.rs`)**: 3-stage recovery (Fast-Path JSON -> Heuristic JSON repair of markdown fences, comments, Python literals, single quotes, trailing commas, unclosed brackets -> Key-Value regex fallback), 60+ parameter alias canonicalization, and Levenshtein fuzzy matching.
4. **23 Native Built-In Tools & Dependency Wave Scheduler (`src/core/tool_registry.rs` & `src/core/tool_specs.rs`)**: Parallel read waves (`tokio::JoinSet`), isolated write barriers, V4A multi-file patches (`*** Begin Patch`), whitespace-tolerant fuzzy healing (`95%` confidence), atomic rollback journal (`TransactionJournal`), and `ToolLoopBreaker`.
5. **Multi-Turn ReAct Engine, Topological Swarm, & 10 Slash Commands (`src/core/engine.rs` & `src/core/slash_and_swarm.rs`)**: Full ReAct loop, `/help`, `/status`, `/compact`, `/clear`, `/memory`, `/vfs`, `/commit`, `/skills`, `/rewind`, `/doctor`, and topological multi-agent swarm execution (`AgentEngine::run_swarm`).

---

## Quick Start & Verification

```bash
# 1. Run all unit & integration tests
cargo test

# 2. Build optimized release binary (~2.3 MiB stripped)
cargo build --release

# 3. Run the 5-scenario Agent Flight Test suite (FT-01..FT-05)
./target/release/heav3ns-micro --flight

# 4. Run substrate micro-benchmarks + Flight Test suite
./target/release/heav3ns-micro --bench

# 5. Run deterministic runtime smoke self-check
./target/release/heav3ns-micro --smoke

# 6. Run strict Joyzoning architectural audit
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
```
