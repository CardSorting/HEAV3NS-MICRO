# HEAV3NS-MICRO — Tactical Rust Micro Agent Runtime

**Source Distilled:** `/Users/bozoegg/Desktop/LUMI-NEW` (2,466 TypeScript files, 725,161 LOC, 834 npm packages)  
**Target Runtime:** `/Users/bozoegg/Desktop/HEAV3NS-MICRO` (6 Rust modules, ~3,370 LOC, 7 direct crates, ~4.6 MiB stripped Mach-O binary)  

HEAV3NS-MICRO is a hyper-compressed, zero-bloat Rust extraction of the `LUMI-NEW` agent runtime. It preserves and consolidates `LUMI-NEW`'s genuine operational mechanisms while eliminating ~99.5% of source code volume and architectural indirection.

---

## Phase 1 & 2 — Reconnaissance & Tactical Classification

| Subsystem | `LUMI-NEW` Source | Classification | `HEAV3NS-MICRO` Consolidation |
| :--- | :--- | :--- | :--- |
| **Agent Turn Loop & Lifecycle** | `src/agents/extensions/execution/agent-engine.ts` (1,617 LOC) | **Preserve & Compress** | [`src/engine.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-MICRO/src/engine.rs) (`AgentEngine`, `PromptComposer`, `RuntimeRequestQueue`) |
| **Topological Swarm Coordinator** | `src/agents/extensions/swarm/agent-swarm-dispatcher.ts` | **Compress** | [`src/engine.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-MICRO/src/engine.rs) (`AgentEngine::run_swarm`, `SwarmAssignment`, dependency wave solver) |
| **Arena Slab, VFS, Compactor, Memory & Skills** | `src/core/arena.ts`, `session-vfs.ts`, `session-compactor.ts`, `memory-manager.ts`, `skills-ingestor.ts` | **Preserve & Compress** | [`src/session.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-MICRO/src/session.rs) (`ArenaAllocator`, `SessionVfs`, `SessionCompactor`, `MemoryStore`, `SkillsIngestor`) |
| **Self-Healing Parser, Streaming Dedup & Redactor** | `tool-call-arg-parser.ts`, `tool-call-delta.ts`, `universal-tool-call-adapter.ts`, `deterministic-secret-redactor.ts` | **Preserve & Compress** | [`src/parser.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-MICRO/src/parser.rs) (`ToolCallArgParser`, `DeterministicSecretRedactor`, `extract_text_tool_calls`) |
| **Wave Scheduler, Patch Engine & 23 Native Tools** | `tool-registry.ts`, `deterministic-patch-engine.ts`, `v4a-patch-tool-suite.ts`, `tool-error-auto-healer.ts`, `codebase-symbol-indexer.ts` | **Preserve & Compress** | [`src/tools.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-MICRO/src/tools.rs) (`ToolRegistry`, `DeterministicPatchEngine`, `TransactionJournal`, `ToolOutputGovernor`, `ToolLoopBreaker`) |
| **Multi-Wire LLM Providers & SSE Streaming** | `provider-factory.ts`, `openai.ts`, `anthropic.ts`, `google.ts`, `scripted-provider.ts` | **Replace** | [`src/provider.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-MICRO/src/provider.rs) (`HttpProvider`, `parse_sse_stream`, `ScriptedProvider`) |
| **Agent Flight Test & Micro-Benchmarks** | `src/evals/flight-test/runner.ts` (`FT-01`..`FT-05`) | **Preserve** | [`src/flight.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-MICRO/src/flight.rs) (`run_agent_flight_test`, `run_micro_benchmarks`) |
| **566 Supervisor / BroccoliSubstrate / Toy Classes** | `src/core/substrate/broccoli-*`, `src/agents/supervisors/*`, 1,600+ no-op tool wrappers | **Discard** | Eliminated (zero operational value) |

---

## Phase 3 — Runtime Architecture & Capabilities

1. **Zero-GC Memory & Context Substrate ([`src/session.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-MICRO/src/session.rs))**:
   - `ArenaAllocator`: 16 MiB contiguous `Vec<u32>` slab with 4-byte word alignment and $O(1)$ pointer rewind on `/clear` or `/rewind`.
   - `SessionCompactor`: Deterministic `LUMI-CONTEXT/1` rolling checkpoints with SHA-256 digests and token-budget windowing.
   - `SessionVfs` & `VfsDiskBaseline`: Staged file overlay with SHA-256 disk baseline verification prior to commit.
   - `MemoryStore` (`LUMI-MEMORY/1`) & `SkillsIngestor` (auto-discovers `.agents/skills`, `.claude/skills`, and `skills/` `SKILL.md` manifests).

2. **Self-Healing Parser & Secret Redaction ([`src/parser.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-MICRO/src/parser.rs))**:
   - 4-pass malformed JSON repair (concatenated roots, Markdown fences, Python `True`/`False`/`None`, single quotes, unquoted keys, trailing commas, unclosed delimiters, and Levenshtein fuzzy key healing).
   - `ADR-0014` streaming tool argument and tool name delta deduplication + `<tool_call>{...}</tool_call>` plain-text fallback extraction.
   - `DeterministicSecretRedactor`: Automatically masks PEM private keys, Anthropic/OpenAI/Google/AWS/HuggingFace/GitHub/Slack credentials, and JWTs in tool outputs, and blocks sensitive credential paths (`.ssh/id_rsa`, `.aws/credentials`, `.git-credentials`, `.netrc`).

3. **Dependency-Aware Wave Scheduler & 23 Native Tools ([`src/tools.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-MICRO/src/tools.rs))**:
   - Partitions tool batches into parallel read waves and serialized mutation barriers.
   - **File Inspection & Search**: `view_file` (`read_file`), `list_dir`, `find_files`, `file_info`, `directory_tree`, `grep_search`, `search_symbols` (multi-language AST/regex symbol & outline indexer).
   - **File Mutation & Patching**: `write_file`, `replace_file_content` (with whitespace-tolerant `heal_fuzzy_patch` & Levenshtein `ENOENT` suggestions), `multi_replace_file_content`, `edit_file_anchored` (`line#hash` verification), `apply_unified_diff` (`apply_patch` / `v4a_apply_patch` supporting both standard Unified Diffs and V4A multi-file patches), `batch_replace_regex`, `delete_file`, `move_file`, `rollback_last_mutation`, `get_workspace_diff`.
   - **Execution, Memory, Spill & Skills**: `run_command` (bounded head/tail capture + safety policy), `save_memory`, `search_memory`, `retrieve_spill_content`, `list_skills`, `use_skill` (exempted from lossy governor truncation).

---

## Phase 4 — Validation & Usage

```bash
# Run all 8 unit tests + FT-01..FT-05 Agent Flight Test suite
cargo test

# Run the 5-scenario Agent Flight Test suite (FT-01..FT-05)
./target/release/heav3ns-micro --flight

# Run substrate micro-benchmarks + Flight Test suite (JSON)
./target/release/heav3ns-micro --bench

# Run deterministic smoke check
./target/release/heav3ns-micro --smoke

# Run headless newline-delimited JSONL stdio worker
./target/release/heav3ns-micro --stdio
```
