<!-- [LAYER: UI] -->
# Architecture (Current) — `HEAV3NS-MICRO`

This document describes the unified architecture of **HEAV3NS-MICRO** (`v0.1.0`), encompassing the hyper-compressed Rust micro-runtime and the AppKit Native Desktop IDE client across all Joyzoning zones (`PLUMBING`, `DOMAIN`, `INFRASTRUCTURE`, `CORE`, `UI`).

---

## 1. Dual-Layer System Topology

The system operates across two distinct but tightly interlaced runtime layers:

```
┌─────────────────────────────────────────────────────────────────┐
│               AppKit Native Desktop IDE (native/)               │
│                                                                 │
│  ┌───────────────────────┐             ┌─────────────────────┐  │
│  │ Navigation Sidebar    │             │ Task Context Panel  │  │
│  │ - Chats (History)     │             │ - Live Walkthrough  │  │
│  │ - File Tree Explorer  │             │ - Changes (Diffs)   │  │
│  │ - Git Changes View    │             │ - Inspect (Evidence)│  │
│  └───────────┬───────────┘             │ - Project Run       │  │
│              │                         └──────────┬──────────┘  │
│              ▼                                    ▼             │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ WorkspaceWindow (Core Controller & Layout Coordinator)     │  │
│  │  - Attachment Token Bar & Multiline Composer              │  │
│  │  - Transcript Scroll View & Turn Scrubber Rail            │  │
│  │  - Non-blocking Docked Approval Bar                       │  │
│  │  - Action Palette (Cmd+K) & Quick Open (Cmd+P)            │  │
│  └─────────────────────────────┬─────────────────────────────┘  │
│                                │                                │
│                                ▼                                │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ RuntimeClient & ControlFrameReader (JSON-RPC Transport)   │  │
│  └─────────────────────────────┬─────────────────────────────┘  │
└────────────────────────────────┼────────────────────────────────┘
                                 │
                                 │ stdio JSON-RPC (v: 1 framing)
                                 │
┌────────────────────────────────▼────────────────────────────────┐
│               Rust Agent Substrate (heav3ns-micro)              │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ src/ui/stdio_worker.rs (Handshake, Routing & Event Loops) │  │
│  └─────────────────────────────┬─────────────────────────────┘  │
│                                │                                │
│                                ▼                                │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ src/ui/rpc_dispatch.rs (Desktop RPC Dispatcher)           │  │
│  │  - files.*, changes.read, spider.check, memory.*          │  │
│  │  - task.submit, task.stop, task.lookup, swarm.*           │  │
│  └─────────────────────────────┬─────────────────────────────┘  │
│                                │                                │
│                                ▼                                │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ src/core/engine.rs (AgentEngine Multi-Turn ReAct Loop)    │  │
│  │  - Parallel Read Waves & Write Barriers (ToolRegistry)    │  │
│  │  - 16-MiB Slab Arena & Snapshot Rewind (ArenaAllocator)   │  │
│  │  - Transactional VFS & Baseline Guard (SessionVfs)        │  │
│  │  - 3-Stage Self-Healing Tool Call Parser (json_repair)    │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. Module Inventory

### Rust Substrate Modules (`src/` — 44 Modules)

| Module | Zone | Architectural Responsibility |
| :--- | :--- | :--- |
| `src/plumbing/hash_and_math.rs` | `PLUMBING` | `now_ms`, `fnv1a_32`, `levenshtein` distance |
| `src/plumbing/text_tokens.rs` | `PLUMBING` | `estimate_text_tokens`, token budgeting, `render_unified_diff`, `encode_base64` |
| `src/plumbing/json_repair.rs` | `PLUMBING` | `extract_json_roots`, `repair_json_string`, streaming tool deduplication |
| `src/plumbing/secret_redactor.rs` | `PLUMBING` | `DeterministicSecretRedactor` (8 secret regex families) & `SafetyPolicy` |
| `src/plumbing/fuzzy_patch.rs` | `PLUMBING` | `heal_fuzzy_patch`, `DeterministicPatchEngine` (V4A patch parser & applier) |
| `src/domain/interfaces/mod.rs` | `DOMAIN` | `ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal` |
| `src/domain/types/*.rs` | `DOMAIN` | Pure value types (`messages.rs`, `runtime.rs`, `progress.rs`, `tick_result.rs`, `vfs_types.rs`, `tool_types.rs`, `tool_schedule.rs`, `skill_types.rs`) |
| `src/domain/arena.rs` | `DOMAIN` | 16-MiB 4-byte aligned bump-pointer `ArenaAllocator` & `SlabBufferSnapshot` |
| `src/domain/memory.rs` | `DOMAIN` | `MemoryStore`, `MemoryEntry`, `GameStateSnapshot`, memory mutability (`remove_memory`) |
| `src/domain/compaction.rs` | `DOMAIN` | `CompactionPolicy`, `CompactionReport`, `ContextBudgetInfo` |
| `src/domain/swarm.rs` | `DOMAIN` | `SwarmAssignment`, `SwarmItemReport`, `SwarmFormationResult` |
| `src/domain/tool_parser.rs` | `DOMAIN` | 3-stage self-healing `ToolCallArgParser` with 60+ parameter aliases |
| `src/infrastructure/http_provider.rs` | `INFRASTRUCTURE` | Multi-protocol `HttpProvider` (OpenAI, Anthropic, Gemini) + SSE stream accumulator |
| `src/infrastructure/scripted_provider.rs` | `INFRASTRUCTURE` | Deterministic `ScriptedProvider` & `ProviderStep` for Flight Tests |
| `src/infrastructure/vfs_store.rs` | `INFRASTRUCTURE` | `SessionVfs`, `SkillsIngestor`, `FileSessionPersistence`, `workspace_revision` |
| `src/infrastructure/transaction_journal.rs` | `INFRASTRUCTURE` | `TransactionJournal` atomic pre-mutation file backups and rollback |
| `src/infrastructure/shell_runner.rs` | `INFRASTRUCTURE` | `execute_shell_command` with timeout and cancellation enforcement |
| `src/infrastructure/fs_tools.rs` | `INFRASTRUCTURE` | `dispatch_fs_tool` (filesystem read/write/patch/rollback tool handlers) |
| `src/infrastructure/search_tools.rs` | `INFRASTRUCTURE` | `resolve_workspace_path`, `suggest_nearby_files`, `execute_grep_search`, `extract_workspace_symbols` |
| `src/core/engine.rs` | `CORE` | `AgentEngine` multi-turn ReAct state machine, context compaction, and model failover |
| `src/core/slash_and_swarm.rs` | `CORE` | `AgentEngine::run_swarm` topological wave execution & 10 interactive slash commands |
| `src/core/session_store.rs` | `CORE` | `SessionStore` (Arena + JSONL persistence) & `SessionCompactor` |
| `src/core/tool_governor.rs` | `CORE` | `ToolOutputGovernor` (65%/35% head/tail truncation), `ToolLoopBreaker`, `ToolExecutionCache` |
| `src/core/tool_specs.rs` | `CORE` | `builtin_tool_specs` (23 built-in tool schemas) |
| `src/core/tool_registry.rs` | `CORE` | `ToolRegistry` & parallel read / exclusive write wave scheduler (`partition_waves`) |
| `src/core/request_queue.rs` | `CORE` | `RuntimeRequestQueue` & `PromptComposer` |
| `src/core/flight_runner.rs` | `CORE` | `run_agent_flight_test` (`FT-01..FT-05`) & `run_micro_benchmarks` |
| `src/core/substrate_tests.rs` | `CORE` | 8 unit and integration test suites |
| `src/ui/cli_presenter.rs` | `UI` | CLI parser, terminal REPL presenter, `--desktop-worker` and `--desktop-account` routing |
| `src/ui/stdio_worker.rs` | `UI` | Stdio JSON-RPC worker protocol engine (`v: 1` framing, handshakes, token checks) |
| `src/ui/rpc_dispatch.rs` | `UI` | Desktop JSON-RPC method handlers (`files.*`, `changes.*`, `spider.*`, `memory.*`, `task.*`, `swarm.*`) |
| `src/lib.rs` & `src/main.rs` | `UI` | Library and binary Composition Roots |

### Native macOS Client Modules (`native/Sources/Heav3nsCode/`)

| File | Architectural Role | Line Cap Status (`SPI-203`) |
| :--- | :--- | :--- |
| `WorkspaceWindow.swift` | Primary window controller, view hierarchy assembly, window/table delegates, and `dispose()` | 791 lines (Tier 1 Optimal) |
| `WorkspaceAttachmentViews.swift` | `AttachmentTokenBar`, `ComposerView`, `TranscriptScrollView`, `TurnScrubberRail` | 222 lines (Tier 1 Optimal) |
| `WorkspaceWindow+Swarm.swift` | Swarm formation graphs, specialist reconciliation loops, and radar state | 603 lines (Tier 1 Optimal) |
| `WorkspaceWindow+SwarmSettings.swift` | Swarm configuration popovers, command recovery sheets, and action confirmations | 223 lines (Tier 1 Optimal) |
| `WorkspaceWindow+Navigation.swift` | Navigation sidebar, chat switcher, file tree browser, and Git change navigator | 308 lines (Tier 1 Optimal) |
| `WorkspaceWindow+Inspection.swift` | File, diff, saved-state, spider check, and context report viewports with scroll anchors | 557 lines (Tier 1 Optimal) |
| `WorkspaceWindow+Execution.swift` | Instruction submission, queue reordering, draft preservation, and checkpoint restoration | 515 lines (Tier 1 Optimal) |
| `WorkspaceWindow+Events.swift` | JSON-RPC event routing, action palette (`Cmd+K`), toolbar delegate, and Pod snapshots | 764 lines (Tier 1 Optimal) |
| `RuntimeClient.swift` | Child process lifecycle management, IPC stream transport, and promise resolution | 382 lines (Tier 1 Optimal) |
| `ControlFrameReader.swift` | Chunked framing parser enforcing `"v": 1` protocol contracts and bounds checking | 114 lines (Tier 1 Optimal) |
| `WalkthroughView.swift` & `WalkthroughState.swift` | Live turn-by-turn agent activity inspector with jump-to-source capabilities | 618 & 282 lines (Tier 1 Optimal) |
| `PodsWindow.swift` & `PodLiveView.swift` | Multi-project swarm command station with concurrent task health telemetry | 284 & 186 lines (Tier 1 Optimal) |

---

## 3. Desktop JSON-RPC Protocol Specification (`v: 1`)

Communication between the AppKit client and `heav3ns-micro` flows across standard I/O using single-line JSON objects tagged with version `"v": 1`:

### Handshake & Initialization
1. Host launches `heav3ns-micro --stdio --desktop-worker`.
2. Worker writes initial connection frame: `{"v": 1, "type": "connection", "state": "ready"}`.
3. Client issues `initialize` request negotiating capabilities: `["conversations", "spider-check-v1", "runtime-state-v1", "task-continuation-v1", "project-repair-v1", "checkpoints-v1", "run-details", "queue-edit", "queue-remove"]`.
4. Client requests `snapshot` for workspace and session state.

### Core RPC Methods
- **`files.list`**: Returns recursive workspace directory tree.
- **`files.read` / `files.readBinary`**: Reads UTF-8 source or Base64-encoded binary documents (PDF, images, CSV) with pagination.
- **`changes.read`**: Renders unified diffs for staged or working-tree file modifications.
- **`spider.check`**: Runs static AST syntax checks, local import verification, and circular dependency checks.
- **`context.inspect` / `context.history`**: Inspects active context window budget, rolling summaries, and retained history.
- **`memory.list` / `memory.save` / `memory.remove`**: Inspects, stores, or deletes long-term conversation memory facts.
- **`task.submit` / `task.stop` / `task.lookup`**: Dispatches instructions, initiates graceful cancellation, or polls unconfirmed submissions without replay.
- **`swarm.plan` / `swarm.next` / `swarm.report`**: Coordinates multi-agent specialist task graphs.

---

## 4. Architectural Invariants & Governance

1. **Rule 1 (`[LAYER: <ZONE>]` Headers)**: Every file across Rust and Swift begins with a layer declaration comment on line 1.
2. **Rule 2 (Geographic Alignment)**: Physical disk layout strictly matches architectural zone boundaries (`SPI-208`).
3. **Rule 3 (Import Discipline)**: Single-line `use` and `import` declarations; zero cyclic dependency loops (`SPI-201`).
4. **Rule 4 (Contractual Sovereignty)**: All concrete infrastructure adapters consumed by Core adhere to formal `I<Name>` traits (`SPI-107`).
5. **Rule 5 (Simplicity Caps)**: All modules remain in **Tier 1 Optimal (< 800 lines, `SPI-203`)**; domain files contain `<= 3` structs.
6. **Rule 6 (Resource Lifecycle Hygiene)**: Window controllers and background task managers implement explicit `dispose()` routines (`SPI-110`).
