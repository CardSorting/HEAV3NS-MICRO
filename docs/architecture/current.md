<!-- [LAYER: UI] -->
# Architecture (Current) — `HEAV3NS-MICRO`

This document describes the **HEAV3NS-MICRO** (`v0.1.0`) architecture across its five Joyzoning zones (`PLUMBING`, `DOMAIN`, `INFRASTRUCTURE`, `CORE`, `UI`).

## Module Inventory (`src/` — 44 Modules)

| Module | Zone | Responsibility |
| :--- | :--- | :--- |
| `src/plumbing/hash_and_math.rs` | `PLUMBING` | `now_ms`, `fnv1a_32`, `levenshtein` |
| `src/plumbing/text_tokens.rs` | `PLUMBING` | `estimate_text_tokens`, `truncate_text_to_token_budget`, `render_unified_diff`, `canonical_tool_name` |
| `src/plumbing/json_repair.rs` | `PLUMBING` | `extract_json_roots`, `repair_json_string`, streaming tool-call argument/name deduplication |
| `src/plumbing/secret_redactor.rs` | `PLUMBING` | `DeterministicSecretRedactor` (8 secret regex families + credential path guard) & `SafetyPolicy` |
| `src/plumbing/fuzzy_patch.rs` | `PLUMBING` | `heal_fuzzy_patch`, `DeterministicPatchEngine` (V4A `*** Begin Patch` parser & applier) |
| `src/domain/interfaces/mod.rs` | `DOMAIN` | `ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal` |
| `src/domain/types/*.rs` | `DOMAIN` | Pure value types (`messages.rs`, `runtime.rs`, `progress.rs`, `tick_result.rs`, `vfs_types.rs`, `tool_types.rs`, `tool_schedule.rs`, `skill_types.rs`) |
| `src/domain/arena.rs` | `DOMAIN` | 16-MiB 4-byte aligned bump-pointer `ArenaAllocator` & `SlabBufferSnapshot` |
| `src/domain/memory.rs` | `DOMAIN` | `MemoryStore`, `MemoryEntry`, `GameStateSnapshot`, `MEMORY_ENVELOPE_PREFIX` |
| `src/domain/compaction.rs` | `DOMAIN` | `CompactionPolicy`, `CompactionReport`, `ContextBudgetInfo` |
| `src/domain/swarm.rs` | `DOMAIN` | `SwarmAssignment`, `SwarmItemReport`, `SwarmFormationResult` |
| `src/domain/tool_parser.rs` | `DOMAIN` | 3-stage self-healing `ToolCallArgParser` with 60+ parameter aliases & schema coercion |
| `src/infrastructure/http_provider.rs` | `INFRASTRUCTURE` | Multi-protocol `HttpProvider` (OpenAI, Anthropic, Gemini) + SSE stream accumulator (`parse_sse_stream`) |
| `src/infrastructure/scripted_provider.rs` | `INFRASTRUCTURE` | Deterministic `ScriptedProvider` & `ProviderStep` for Flight Tests |
| `src/infrastructure/vfs_store.rs` | `INFRASTRUCTURE` | `SessionVfs`, `SkillsIngestor`, `FileSessionPersistence`, `workspace_revision` |
| `src/infrastructure/transaction_journal.rs` | `INFRASTRUCTURE` | `TransactionJournal` atomic pre-mutation backup and rollback |
| `src/infrastructure/shell_runner.rs` | `INFRASTRUCTURE` | `execute_shell_command` with timeout and `CancellationToken` enforcement |
| `src/infrastructure/fs_tools.rs` | `INFRASTRUCTURE` | `dispatch_fs_tool` (filesystem read/write/patch/rollback tool handlers) |
| `src/infrastructure/search_tools.rs` | `INFRASTRUCTURE` | `resolve_workspace_path`, `suggest_nearby_files`, `execute_grep_search`, `extract_workspace_symbols` |
| `src/core/engine.rs` | `CORE` | `AgentEngine` multi-turn ReAct state machine, context compaction, and model failover |
| `src/core/slash_and_swarm.rs` | `CORE` | `AgentEngine::run_swarm` topological wave execution & 10 interactive slash commands |
| `src/core/session_store.rs` | `CORE` | `SessionStore` (Arena + JSONL persistence) & `SessionCompactor` |
| `src/core/tool_governor.rs` | `CORE` | `ToolOutputGovernor` (65%/35% head/tail truncation), `ToolLoopBreaker`, `ToolExecutionCache` |
| `src/core/tool_specs.rs` | `CORE` | `builtin_tool_specs` (23 built-in tool schemas) |
| `src/core/tool_registry.rs` | `CORE` | `ToolRegistry` & parallel read / exclusive write wave scheduler (`partition_waves`) |
| `src/core/request_queue.rs` | `CORE` | `RuntimeRequestQueue` & `PromptComposer` (`LUMI-MEMORY/1`, `SKILL.md`, `AGENTS.md` injection) |
| `src/core/flight_runner.rs` | `CORE` | `run_agent_flight_test` (`FT-01..FT-05`) & `run_micro_benchmarks` |
| `src/core/substrate_tests.rs` | `CORE` | 8 unit and integration test suites |
| `src/ui/cli_presenter.rs` | `UI` | CLI argument parser, help renderer, and headless JSONL stdio worker (`run_stdio_worker`) |
| `src/lib.rs` & `src/main.rs` | `UI` | Library and binary Composition Roots |

