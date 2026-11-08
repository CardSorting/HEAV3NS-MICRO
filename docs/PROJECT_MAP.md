<!-- [LAYER: UI] -->
# Project Map & Architecture (`HEAV3NS-MICRO`)

This map reflects the actual layout of the **HEAV3NS-MICRO** workspace (`/Users/bozoegg/Desktop/HEAV3NS-MICRO`).

## Workspace Overview

| Directory / File | Architectural Zone | Role |
| :--- | :--- | :--- |
| **`src/plumbing/`** | `PLUMBING` | Stateless leaf primitives: `hash_and_math.rs` (`now_ms`, `fnv1a_32`, `levenshtein`), `text_tokens.rs` (`estimate_text_tokens`, `truncate_text_to_token_budget`, `render_unified_diff`, `canonical_tool_name`), `json_repair.rs` (`extract_json_roots`, `repair_json_string`, streaming tool-call deduplicators), `secret_redactor.rs` (`DeterministicSecretRedactor`, `SafetyPolicy`), `fuzzy_patch.rs` (`heal_fuzzy_patch`, `DeterministicPatchEngine`) |
| **`src/domain/`** | `DOMAIN` | Pure domain models and state machines (`0%` I/O, `<= 3` structs/file): `arena.rs` (`ArenaAllocator`, `SlabBufferSnapshot`), `memory.rs` (`MemoryStore`, `MemoryEntry`, `GameStateSnapshot`), `compaction.rs` (`CompactionPolicy`, `CompactionReport`, `ContextBudgetInfo`), `swarm.rs` (`SwarmAssignment`, `SwarmItemReport`, `SwarmFormationResult`), `tool_parser.rs` (`ToolCallArgParser`) |
| **`src/domain/interfaces/`** | `DOMAIN` | Formal Dependency Inversion traits (`mod.rs`): `ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal` |
| **`src/domain/types/`** | `DOMAIN` | Pure domain value types (`<= 3` structs/file): `messages.rs`, `runtime.rs`, `progress.rs`, `tick_result.rs`, `vfs_types.rs`, `tool_types.rs`, `tool_schedule.rs`, `skill_types.rs` |
| **`src/infrastructure/`** | `INFRASTRUCTURE` | Concrete I/O adapters implementing `domain/interfaces`: `http_provider.rs` (`HttpProvider`, `parse_sse_stream`), `scripted_provider.rs` (`ScriptedProvider`), `vfs_store.rs` (`SessionVfs`, `SkillsIngestor`, `FileSessionPersistence`, `workspace_revision`), `transaction_journal.rs` (`TransactionJournal`), `shell_runner.rs` (`execute_shell_command`), `fs_tools.rs` (`dispatch_fs_tool`), `search_tools.rs` (`resolve_workspace_path`, `suggest_nearby_files`, `execute_grep_search`, `extract_workspace_symbols`) |
| **`src/core/`** | `CORE` | Use-case orchestration: `engine.rs` (`AgentEngine` ReAct loop), `slash_and_swarm.rs` (`run_swarm` & 10 slash commands), `session_store.rs` (`SessionStore`, `SessionCompactor`), `tool_governor.rs` (`ToolOutputGovernor`, `ToolLoopBreaker`, `ToolExecutionCache`), `tool_specs.rs` (23 built-in `ToolSpec` declarations), `tool_registry.rs` (`ToolRegistry` & wave scheduler), `request_queue.rs` (`RuntimeRequestQueue`, `PromptComposer`), `flight_runner.rs` (`FT-01..FT-05` Flight Test suite & micro-benchmarks), `substrate_tests.rs` (unit/integration test suite) |
| **`src/ui/`** | `UI` | CLI argument parser, interactive REPL, and JSONL stdio worker (`cli_presenter.rs`, `mod.rs`) |
| **`src/lib.rs`** | `UI` | Library Composition Root & backward-compatible public module facades (`engine`, `flight`, `parser`, `provider`, `session`, `tools`) |
| **`src/main.rs`** | `UI` | Binary Composition Root dispatching `--flight`, `--bench`, `--smoke`, `--stdio`, `-p`, and interactive REPL modes |
| **`docs/`** | `UI` / `DOMAIN` | Documentation hub (`README.md`, `PROJECT_MAP.md`, `CODE_TO_DOC_MAP.md`, `CONSTITUTION.md`, `CODEBASE_STANDARDS.md`, `architecture/current.md`) |

---

## Core Execution Flow

```
src/main.rs (Binary Composition Root)
  └─ src/ui/cli_presenter.rs (CLI & JSONL Stdio Worker)
       └─ src/core/engine.rs (AgentEngine::tick) & src/core/slash_and_swarm.rs (AgentEngine::run_swarm)
            ├─ RuntimeRequestQueue & PromptComposer (src/core/request_queue.rs)
            ├─ SessionStore & SessionCompactor (src/core/session_store.rs)
            │    ├─ ArenaAllocator (src/domain/arena.rs)
            │    ├─ MemoryStore (src/domain/memory.rs)
            │    └─ SessionVfs & FileSessionPersistence (src/infrastructure/vfs_store.rs)
            ├─ ILlmProvider -> HttpProvider / ScriptedProvider (src/infrastructure/http_provider.rs)
            └─ ToolRegistry (src/core/tool_registry.rs)
                 ├─ ToolCallArgParser (src/domain/tool_parser.rs -> src/plumbing/json_repair.rs)
                 ├─ ToolOutputGovernor, ToolLoopBreaker, ToolExecutionCache (src/core/tool_governor.rs)
                 ├─ TransactionJournal (src/infrastructure/transaction_journal.rs)
                 └─ dispatch_fs_tool / search_tools / shell_runner (src/infrastructure/*)
```

