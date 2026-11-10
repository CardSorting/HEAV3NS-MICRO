<!-- [LAYER: UI] -->
# Architecture (Current) — `HEAV3NS-MICRO` & The Seven Sovereign Pillars

This document describes the unified architecture of **HEAV3NS-MICRO** (`v0.1.0`), the distilled intelligence runtime of **[`HEAV3NS — The Sovereign Intelligence Architecture`](../../doc.md)**, encompassing the 82-module Rust substrate and the AppKit Native Desktop IDE client (`HEAV3NS-CODE`) across all five Joyzoning zones (`PLUMBING`, `DOMAIN`, `INFRASTRUCTURE`, `CORE`, `UI`).

---

## 1. Dual-Layer System Topology & The Seven Sovereign Pillars

The system operates across two tightly interlaced layers uniting the seven sovereign pillars (`HEAV3NS-MICRO`, `SPIDER`, `REWIND`, `BROCCOLI`, `SYNTHETIC SWARM`, `COMPUTER USE`, `THE HARNESS`):

```
┌─────────────────────────────────────────────────────────────────┐
│         AppKit Native Desktop IDE (native/ · HEAV3NS-CODE)      │
│                                                                 │
│  ┌───────────────────────┐             ┌─────────────────────┐  │
│  │ Unified Left Navigator│             │ Task Context Panel  │  │
│  │ - Chats (⌘1)          │             │ - Live Walkthrough  │  │
│  │ - Files (⌘2)          │             │ - Changes (Diffs)   │  │
│  │ - Changes (⌘3)        │             │ - Inspect & Spider  │  │
│  └───────────┬───────────┘             │ - Project Run & Web │  │
│              │                         └──────────┬──────────┘  │
│              ▼                                    ▼             │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ WorkspaceWindow (Modular Tier 1 Optimal Topology)         │  │
│  │  - Attachment Token Bar & Multiline Composer              │  │
│  │  - 5-Stop Posture Pill (Shift+Tab) & 3-Choice Approval Bar│  │
│  │  - Omnibar Shortcut Badges (F3, ⌥⌘Z, ⇧⌘I, ⇧⌘K)            │  │
│  │  - Live TurnReceiptSummaryBar & Scrubber Rail             │  │
│  │  - Swarm Radar & Multi-Project Pods Command Station       │  │
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
│          Rust Sovereign Substrate (heav3ns-micro · 82 Modules)  │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ UI: stdio_worker.rs · rpc_dispatch.rs · cli_presenter.rs  │  │
│  │  - Multiplexed async task execution (tokio::spawn)        │  │
│  │  - Immediate posture.set, approval.resolve & turn.undo    │  │
│  └─────────────────────────────┬─────────────────────────────┘  │
│                                │                                │
│                                ▼                                │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ CORE: AgentEngine · ComputerUseOrchestrator · SpatialEng  │  │
│  │  - 5-Stop Execution Posture & FileReadTracker Guard       │  │
│  │  - 31 Native Tools (tool_search 8-LRU + execute_tools)    │  │
│  │  - 9-Stage Closed-Loop CUA & Double-Wrapped Mediation     │  │
│  │  - 5-Specialist Swarm Consultant & 7-Specialist Legion    │  │
│  └─────────────────────────────┬─────────────────────────────┘  │
│                                │ (64 Formal I<Name> Traits)     │
│                                ▼                                │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ INFRASTRUCTURE & DOMAIN & PLUMBING                        │  │
│  │  - REWIND: 16-MiB Arena · SHA-256 CAS · SessionVfs        │  │
│  │  - Auto-Refreshing 2026 ModelPricingCatalog & Registry    │  │
│  │  - Parameterized macOS Seatbelt Sandbox & Pipe Draining   │  │
│  │  - BROCCOLI: BehavioralKnowledgeStore · Memory · SkillDAG │  │
│  │  - SPIDER: environmental_diff · spatial_geometry          │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. Complete Module Inventory

### Rust Substrate Modules (`src/` — 82 Modules, All `< 800` Lines Tier 1 Optimal)

| Module | Zone | Sovereign Pillar & Architectural Responsibility |
| :--- | :--- | :--- |
| `src/plumbing/hash_and_math.rs` | `PLUMBING` | `THE HARNESS`: `now_ms`, `fnv1a_32`, `levenshtein` distance |
| `src/plumbing/text_tokens.rs` | `PLUMBING` | `THE HARNESS`: `safe_utf8_prefix`, `safe_utf8_slice`, `mask_secret_preview`, token budgeting, positional LCS `render_unified_diff`, `encode_base64`, `canonical_tool_name` |
| `src/plumbing/json_repair.rs` | `PLUMBING` | `HEAV3NS-MICRO`: `extract_json_roots`, `repair_json_string`, streaming tool deduplication |
| `src/plumbing/secret_redactor.rs` | `PLUMBING` | `THE HARNESS`: `DeterministicSecretRedactor` (8 secret families), `CredentialDenyScope` (`Process` vs `ModelContext`, allowing `.env.example`), & `SafetyPolicy` |
| `src/plumbing/seatbelt_profile.rs` | `PLUMBING` | `THE HARNESS`: `SeatbeltSandboxMode`, `build_macos_seatbelt_invocation` (parameterized SBPL protecting `.git/hooks`, `~/.ssh`, `~/.aws`), `inspect_command_security` |
| `src/plumbing/shell_guard.rs` | `PLUMBING` | `THE HARNESS`: `pub trait IShellCommandGuard`, `ShellCommandGuard` (quote/compound heredoc file-write interdiction & pipeline terminal file-read guard) |
| `src/plumbing/fuzzy_patch.rs` | `PLUMBING` | `THE HARNESS`: `heal_fuzzy_patch`, `DeterministicPatchEngine` (V4A multi-file patch parser & applier) |
| `src/plumbing/vision_and_dom.rs` | `PLUMBING` | `COMPUTER USE`: `decode_image_metadata` (PNG/JPEG/GIF/WebP/BMP/SVG), `snapshot_html_dom`, `validate_navigation_url` (SSRF/IMDSv2 guard), synthetic SVG/BMP generators |
| `src/plumbing/spatial_geometry.rs` | `PLUMBING` | `COMPUTER USE`: `SpatialPoint`, `SpatialRect` (15% safe center, IoU), `ViewportTransform` (scroll & DPR), `calculate_rectangles_union_area` (exact 2D sweep-line union), `validate_trajectory` |
| `src/plumbing/environmental_diff.rs` | `PLUMBING` | `SPIDER`: `compute_element_deltas`, `classify_element_affordance`, `detect_modal_or_dialog`, `analyze_spider_structural_relationships`, `diagnose_failure_cause` |
| `src/plumbing/display_telemetry.rs` | `PLUMBING` | `COMPUTER USE` & `REWIND`: `compute_preview_delta`, `apply_annotation_mutation`, `query_action_ledger_dsl`, `compute_action_ledger_health`, `extract_dom_ui_tuples` |
| `src/plumbing/mod.rs` | `PLUMBING` | Stateless plumbing module exports |
| `src/domain/interfaces/mod.rs` | `DOMAIN` | `22` core port traits (`ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal`, `IWorkspaceSnapshotCas`, `IModelPricingRegistry`, `ISpatialMediator`, `ISpatialGroundingGate`, `IExecutionAssuranceGate`, `IEnvironmentalObserver`, `IBehavioralKnowledgeStore`, `IBehavioralStore`, `ISwarmIntelligenceConsultant`, `IEnvironmentalDiffer`, `ISpatialGeometry`, `IComputerUseOrchestrator`, `IWorkspacePromptProbe`) |
| `src/domain/types/*.rs` (`16` files) | `DOMAIN` | Pure value & behavioral domain types (`model_catalog.rs`, `posture_types.rs`, `messages.rs`, `runtime.rs`, `progress.rs`, `tick_result.rs`, `vfs_types.rs`, `tool_types.rs`, `tool_schedule.rs`, `skill_types.rs`, `computer_use_types.rs`, `spatial_types.rs`, `environmental_types.rs`, `swarm_intelligence_types.rs`, `learned_knowledge_types.rs`, `mod.rs`) |
| `src/domain/prompt_spec.rs` | `DOMAIN` | `HEAV3NS-MICRO`: `SystemPromptSpec` (`Standard`, `Compact`, `SwarmSpecialist`, `SideQuestion`), KV-cache-deterministic `static_prefix`, `ModelPromptFamily`, & `format_bounded_handoff_item` |
| `src/domain/prefix_and_manifest.rs` | `DOMAIN` | `HEAV3NS-MICRO`: `PrefixStabilityTracker` (`PinnedInitial`, `Stable`, `Repinned`, `Drift`), `RequestManifest` (`heav3ns.request_manifest/v1`), and `<context_update>` append blocks |
| `src/domain/pricing_and_taxonomy.rs` | `DOMAIN` | `HEAV3NS-MICRO`: `IBillingMeter`, `IErrorClassifier`, `BillingMetering` (`Metered`, `LocalNoBill`, `Subscription`, `Unknown`), `ErrorEnvelope` (`402` Budget vs `429` RateLimit) |
| `src/domain/turn_receipt.rs` | `DOMAIN` | `THE HARNESS`: `ITurnReceipt`, `TurnReceipt` (`heav3ns.receipt/v1` prose-free verification receipts with CAS diffs, command outcomes, honest billing, claims) |
| `src/domain/arena.rs` | `DOMAIN` | `REWIND`: 16-MiB 4-byte aligned bump-pointer `ArenaAllocator` & `SlabBufferSnapshot` ($O(1)$ rewind) |
| `src/domain/memory.rs` | `DOMAIN` | `BROCCOLI`: `MemoryStore` (hybrid TF-IDF + FNV-1a recall), `MemoryEntry`, `GameStateSnapshot` |
| `src/domain/compaction.rs` | `DOMAIN` | `HEAV3NS-MICRO`: `CompactionPolicy`, `CompactionReport`, `ContextBudgetInfo` |
| `src/domain/swarm.rs` | `DOMAIN` | `SYNTHETIC SWARM`: `SwarmAssignment`, `SwarmItemReport`, `SwarmFormationResult`, `classify_task_for_legion` (7-specialist Legion classifier) |
| `src/domain/tool_parser.rs` | `DOMAIN` | `HEAV3NS-MICRO`: 3-stage self-healing `ToolCallArgParser` with 60+ parameter aliases |
| `src/domain/computer_use.rs` | `DOMAIN` | `COMPUTER USE` & `REWIND`: `VirtualDisplayState` (`scroll_offset`, 32-deep undo/redo snapshots, epoch-fenced control lease, Set-of-Marks & DOM execution) |
| `src/domain/skill_tree.rs` | `DOMAIN` | `BROCCOLI`: `SkillTreeCatalog` (Kahn's topological DAG, 6 power-up packs, `SkillDoctor` linter, multi-format Soul Drop Vault import/export) |
| `src/domain/spatial_action.rs` | `DOMAIN` | `COMPUTER USE`: `GroundedSpatialTarget`, `SpatialGroundingResult`, `SpatialExecutionReceipt` |
| `src/domain/spatial_mediator.rs` | `DOMAIN` | `COMPUTER USE`: `SpatialActionMediator` (Wrapper A Grounding Gate + Wrapper B Execution Assurance Gate), `SpatialActionPlan`, `SpatialInteractionAudit` |
| `src/domain/mod.rs` | `DOMAIN` | Pure domain layer barrel exports |
| `src/infrastructure/http_provider.rs` | `INFRASTRUCTURE` | `HEAV3NS-MICRO`: Multi-protocol `HttpProvider` (OpenAI, Anthropic with consecutive `Role::Tool` coalescing, Gemini) + SSE stream accumulator |
| `src/infrastructure/pricing_updater.rs` | `INFRASTRUCTURE` | `HEAV3NS-MICRO`: `ModelPricingRegistry` implementing `IModelPricingRegistry`, atomic `0600` cache at `~/.heav3ns/pricing_catalog.json`, 12h TTL, background refreshing |
| `src/infrastructure/scripted_provider.rs` | `INFRASTRUCTURE` | `THE HARNESS`: Deterministic `ScriptedProvider` & `ProviderStep` for Flight Tests |
| `src/infrastructure/vfs_store.rs` | `INFRASTRUCTURE` | `REWIND`: `SessionVfs` (`VfsDiskBaseline` verification), `SkillsIngestor`, `FileSessionPersistence` |
| `src/infrastructure/workspace_cas.rs` | `INFRASTRUCTURE` | `REWIND`: `WorkspaceSnapshotCas` implementing `IWorkspaceSnapshotCas` (pure-Rust SHA-256 pre/post-turn snapshots, `/diff`, `/restore`, `WouldBlock`-safe `/undo`) |
| `src/infrastructure/transaction_journal.rs` | `INFRASTRUCTURE` | `REWIND`: `TransactionJournal` atomic pre-mutation file backups and rollback |
| `src/infrastructure/behavioral_store.rs` | `INFRASTRUCTURE` | `BROCCOLI` & `REWIND`: `BehavioralKnowledgeStore` (Laplace confidence calibration, workflow procedure registry, `reconcile_state` epoch drift check) |
| `src/infrastructure/shell_runner.rs` | `INFRASTRUCTURE` | `THE HARNESS`: `execute_shell_command_sandboxed` with concurrent pipe drain, Seatbelt containment, and Unix `-pgid` process-group termination |
| `src/infrastructure/fs_tools.rs` | `INFRASTRUCTURE` | `THE HARNESS`: `dispatch_fs_tool` (filesystem read/write/patch/rollback tool handlers) |
| `src/infrastructure/search_tools.rs` | `INFRASTRUCTURE` | `SPIDER`: `resolve_workspace_path`, `suggest_nearby_files`, `execute_grep_search`, `extract_workspace_symbols` |
| `src/infrastructure/mod.rs` | `INFRASTRUCTURE` | Infrastructure adapter exports |
| `src/core/engine.rs` | `CORE` | `HEAV3NS-MICRO`: `AgentEngine` multi-turn ReAct state machine, sticky tool routing, specialist isolation, context compaction, and model failover |
| `src/core/turn_orchestrator.rs` | `CORE` | `HEAV3NS-MICRO`: Turn lifecycle orchestration, CAS snapshot orchestration, honest billing computation, and `TurnReceipt` assembly |
| `src/core/batch_executor.rs` | `CORE` | `THE HARNESS`: Declarative `execute_tools` batch execution (`MAX_DECLARATIVE_BATCH_STEPS = 32`) and `tool_search` LRU cache (`MAX_UNLOCKED_DEFERRED_TOOLS = 8`) |
| `src/core/context_inspector.rs` | `CORE` | `HEAV3NS-MICRO`: `ContextInspectionSnapshot`, `/context` token budget HUD, `/prompt` prefix/profile inspector, and `/preview-request` manifest inspector |
| `src/core/computer_use_orchestrator.rs` | `CORE` | `COMPUTER USE` · `SPIDER` · `SYNTHETIC SWARM`: `ComputerUseOrchestrator` 9-stage closed loop, modal auto-intercept, auto-focus planning, and 5-specialist Swarm consultation |
| `src/core/spatial_engine.rs` | `CORE` | `COMPUTER USE`: `SpatialInteractionEngine` (Double-Wrapped dispatch, automatic off-screen pre-scroll, multi-step `execute_plan`, DOM/visual reconciliation, directional queries) |
| `src/core/slash_and_swarm.rs` | `CORE` | `SYNTHETIC SWARM`: `AgentEngine::run_swarm` topological wave execution & 35 interactive slash command arms |
| `src/core/session_store.rs` | `CORE` | `REWIND`: `SessionStore` (Arena + JSONL persistence) & `SessionCompactor` |
| `src/core/tool_governor.rs` | `CORE` | `THE HARNESS`: `ToolOutputGovernor`, `FileReadTracker` (read-before-write & external disk staleness), `ToolLoopBreaker`, `ToolExecutionCache` |
| `src/core/tool_specs.rs` | `CORE` | `THE HARNESS`: `IToolSpecProvider`, `builtin_tool_specs` (31 built-in tool schemas including `tool_search` & `execute_tools`) |
| `src/core/tool_registry.rs` | `CORE` | `THE HARNESS`: `ToolRegistry`, sticky on-demand specialist schema routing (`15` core + `16` on-demand), & parallel read / exclusive write wave scheduler (`partition_waves`) |
| `src/core/request_queue.rs` | `CORE` | `THE HARNESS`: `RuntimeRequestQueue` & `PromptComposer` (`compose_with_spec`, `LUMI-MEMORY/1`, `LUMI-CONTEXT/1`) |
| `src/core/flight_runner.rs` & `benchmark_runner.rs` | `CORE` | `THE HARNESS`: `run_agent_flight_test` (`FT-01..FT-05`), `run_micro_benchmarks`, & full benchmark suite |
| `src/core/substrate_tests.rs`, `spatial_tests.rs`, `computer_use_tests.rs`, `zenith_tests.rs`, `codewhale_zenith_tests.rs`, `mod.rs` | `CORE` | 77 unit, spatial, computer-use, prompt/shell-guard zenith, and Codewhale zenith test suites |
| `src/ui/cli_presenter.rs` & `onboarding.rs` | `UI` | CLI parser (`context`, `doctor`, `demo`, `init`, `models`, `receipts`, `preview-request`), terminal REPL presenter, permissions enforcer |
| `src/ui/stdio_worker.rs` | `UI` | Multiplexed Stdio JSON-RPC worker protocol engine (`v: 1` framing, `tokio::spawn` multi-tasking, handshake, token checks) |
| `src/ui/worker_handlers.rs` | `UI` | Worker request handlers for `task.submit`, `task.stop`, `approval.resolve`, `posture.*`, `cas.*`, `receipts.*`, `models.*` |
| `src/ui/rpc_dispatch.rs` | `UI` | Desktop JSON-RPC method handlers (`files.*`, `changes.*`, `spider.*`, `memory.*`, `context.inspect`, `prompt.inspect`, `task.*`, `swarm.*`, `spatial.*`, `cua.*`, `models.*`) |
| `src/ui/mod.rs`, `src/lib.rs`, `src/main.rs` | `UI` | UI module and binary/library Composition Roots |

### Native macOS Client Modules (`native/Sources/Heav3nsCode/`)

| File | Architectural Role | Line Cap Status (`SPI-203`) |
| :--- | :--- | :--- |
| `WorkspaceWindow.swift` | Primary window controller, view hierarchy assembly, window/table delegates, and `dispose()` | Tier 1 Optimal (`< 800` lines) |
| `WorkspaceAttachmentViews.swift` | `AttachmentTokenBar`, `ComposerView`, `TranscriptScrollView`, `TurnScrubberRail` | Tier 1 Optimal (`< 800` lines) |
| `WorkspacePostureAndReceiptViews.swift` | 5-stop posture dial (`Shift+Tab`), 3-choice session approval bar, live `TurnReceiptSummaryBar`, and receipts inspector | Tier 1 Optimal (`< 800` lines) |
| `WorkspaceWindow+OmnibarAndPosture.swift` | Unified Omnibar, shortcut badges (`F3`, `⌥⌘Z`, `⇧⌘I`, `⇧⌘K`), and posture step controller | Tier 1 Optimal (`< 800` lines) |
| `WorkspaceWindow+Swarm.swift` | Swarm formation graphs, specialist reconciliation loops, and radar state | Tier 1 Optimal (`< 800` lines) |
| `WorkspaceWindow+SwarmSettings.swift` | Swarm configuration popovers, command recovery sheets, and `MasterToggleController` | Tier 1 Optimal (`< 800` lines) |
| `WorkspaceWindow+Navigation.swift` | Unified left navigator (`[Chats \| Files \| Changes]`), file tree browser, and Git change navigator | Tier 1 Optimal (`< 800` lines) |
| `WorkspaceWindow+Inspection.swift` | File, diff, saved-state, spider check, request preview, and context report viewports | Tier 1 Optimal (`< 800` lines) |
| `WorkspaceWindow+Execution.swift` | Instruction submission, queue reordering, draft preservation, and checkpoint restoration | Tier 1 Optimal (`< 800` lines) |
| `WorkspaceWindow+Events.swift` | JSON-RPC event routing, action palette (`Cmd+K`), toolbar delegate, and Pod snapshots | Tier 1 Optimal (`< 800` lines) |
| `CodeOutline.swift` & `InspectionNavigation.swift` | Unicode-safe UTF-16 intra-line diff highlighter, lexical outline, `jump(to:)`, and `recentFiles(limit:)` | Tier 1 Optimal (`< 800` lines) |
| `RuntimeClient.swift` & `ControlFrameReader.swift` | Child process lifecycle management, `"v": 1` IPC stream transport, and promise resolution | Tier 1 Optimal (`< 800` lines) |
| `WalkthroughView.swift` & `PodsWindow.swift` | Live turn-by-turn agent activity inspector (Timeline/List/Kanban) and Multi-Project Pods command station | Tier 1 Optimal (`< 800` lines) |

---

## 3. Architectural Invariants & Governance

1. **Rule 1 (`[LAYER: <ZONE>]` Headers)**: Every file across Rust, Swift, and Markdown begins with a layer declaration header.
2. **Rule 2 (Geographic Alignment)**: Physical disk layout strictly matches architectural zone boundaries (`SPI-208`).
3. **Rule 3 (Import Discipline)**: Single-line `use` and `import` declarations; zero cyclic dependency loops (`SPI-201`).
4. **Rule 4 (Contractual Sovereignty)**: All concrete infrastructure adapters and domain entities adhere to `64` formal `I<Name>` traits (`SPI-107`).
5. **Rule 5 (Simplicity Caps)**: All `309+` workspace files remain in **Tier 1 Optimal (`< 800` lines, `SPI-203`)**; domain files contain `<= 3` structs (`<= 1` struct if `> 500` lines).
6. **Rule 6 (Resource Lifecycle Hygiene)**: Every Swift class in `native/` implements explicit `dispose()` teardown (`0` `SPI-603` advisories).
