<!-- [LAYER: UI] -->
# AGENTS.md — Operational Acquisition Protocol for Coding Agents (`HEAV3NS-MICRO`)

This file is the direct operational entrypoint for any autonomous coding agent, pair-programming assistant, or automated workflow encountering **HEAV3NS-MICRO**, the distilled runtime of **[`HEAV3NS — The Sovereign Intelligence Architecture`](doc.md)**.

<!-- joy:begin agents -->
## Joyzoning — architecture protocol (HEAV3NS-JOY 1.0.0)

Every source file belongs to exactly one zone, tagged in its first 10,000 characters
(`/** [LAYER: DOMAIN] */`, `# [LAYER: CORE]`, `<!-- [LAYER: UI] -->` …) and matching its folder.

| Zone | Folder | Rule |
| :--- | :--- | :--- |
| `DOMAIN` | `src/domain/` | pure business rules and interfaces (`src/domain/interfaces/I*`); zero I/O; imports nothing from infrastructure or ui |
| `CORE` | `src/core/` | use-case orchestration; depends on domain interfaces, never on ui |
| `INFRASTRUCTURE` | `src/infrastructure/` | concrete I/O adapters implementing domain interfaces; never imports ui |
| `PLUMBING` | `src/plumbing/ or src/utils/` | stateless leaf helpers; imports no other zone; no mutable top-level state |
| `UI` | `src/ui/` | rendering and user intent; reaches infrastructure only through core |

Also: relative imports use at most 3 `../`; no runtime import cycles; core depends on `I<Name>` interfaces in `src/domain/interfaces/`;
files stay under ~800 lines. Build producers (plumbing, domain, infrastructure) before consumers (core, ui).

### Loop (cheap in tokens)
1. Before editing: `npx -y heav3ns-joy@1.0.0 check . --phase pre-edit --scope <file> --neighborhood 1 --format compact`
2. After editing: `npx -y heav3ns-joy@1.0.0 check . --phase post-edit --scope <file> --format compact`
3. Exit `0` = done (`[INFO]` is optional) · `1` = fix each blocking finding · `2` = you mis-invoked the tool (never treat as success).
4. Explain any code: `npx -y heav3ns-joy@1.0.0 check --explain SPI-206`. Fix headers: `npx -y heav3ns-joy@1.0.0 check . --heal --dry-run`, review, then `--heal`.
5. Never weaken config to make a check pass; report the finding instead. Stop after 3 failed attempts on one finding.

### Durable work primitives (no harness integration needed)
- `npx -y heav3ns-joy@1.0.0 trinity selftest` then `npx -y heav3ns-joy@1.0.0 trinity --help`: durable queue (`queue add|list|drain`), buffered tables (`db push|select`), specialist swarm.
- Swarm without touching your harness: `swarm plan --task "<goal>" --id f1` → loop `swarm next f1` / `swarm report f1 --assignment <id> --status completed --summary "<evidence>"` → `swarm finish f1`.
- Output is one JSON document on stdout; exit `0` ok, `1` failure (read `error.message`), `2` usage error (follow the `hint`).
- If MCP tools named `joy_*` are available, prefer them: same operations, typed arguments.

Skills `joyzoning`, `joy-trinity`, `joy-swarm` hold the details; `SOUL.md` holds the working principles. Health check: `npx -y heav3ns-joy@1.0.0 doctor`.
<!-- joy:end agents -->

---

## Phase 1: `HEAV3NS-MICRO` 5-Zone Rust Architecture (`96` Modules · `7` Sovereign Pillars)

Every source file in `HEAV3NS-MICRO` belongs to one of the five canonical Joyzoning zones and serves the Seven Sovereign Pillars ([`doc.md`](doc.md): `HEAV3NS-MICRO`, `SPIDER`, `REWIND`, `BROCCOLI`, `SYNTHETIC SWARM`, `COMPUTER USE`, `THE HARNESS`):

1. **`PLUMBING` (`src/plumbing/` — 14 modules)** — Stateless leaf primitives (`// [LAYER: PLUMBING]`):
   - `hash_and_math.rs`: `now_ms`, `fnv1a_32`, `levenshtein`
   - `text_tokens.rs`: `safe_utf8_prefix`, `safe_utf8_slice`, `mask_secret_preview`, `estimate_text_tokens`, `truncate_text_to_token_budget`, positional LCS `render_unified_diff`, `encode_base64`, `canonical_tool_name`
   - `oauth_pkce.rs`: pure PKCE S256 challenge/verifier generation, Base64URL encoding/decoding, URL query parsing, and OpenAI JWT claim extraction (`chatgpt_account_id`, `email`, `chatgpt_plan_type`, `exp`)
   - `json_repair.rs`: `extract_json_roots`, `repair_json_string`, `accumulate_streaming_tool_name`, `compute_streaming_arguments_delta`, `normalize_completed_tool_call_arguments`
   - `schema_and_sse.rs`: `parse_sse_events` (8 MiB frame cap, CRLF, `: comment` heartbeats, compact & multiline `data:`), `sanitize_tool_schema`, `sanitize_tool_schema_for_responses`, `redact_endpoint_for_display`
   - `secret_redactor.rs`: `DeterministicSecretRedactor`, `CredentialDenyScope`, `SafetyPolicy` (Trojan Source, SSRF, Heredoc `ADR-0015`/`0016`, C2 threat scanner, `is_sensitive_env_var` credential env scrubbing)
   - `seatbelt_profile.rs`: `SeatbeltSandboxMode`, `build_macos_seatbelt_invocation`, `inspect_command_security`
   - `fuzzy_patch.rs`: `heal_fuzzy_patch`, `DeterministicPatchEngine`, `PatchHunkLine`, `PatchOperation`
   - `vision_and_dom.rs`: `decode_image_metadata`, `snapshot_html_dom`, `validate_navigation_url`, `generate_synthetic_svg`, `generate_synthetic_bmp`
   - `spatial_geometry.rs`: `SpatialPoint`, `SpatialRect`, `ViewportTransform`, `calculate_rectangles_union_area`, `normalize_coordinate_value`, `validate_trajectory`
   - `environmental_diff.rs`: `compute_element_deltas`, `classify_element_affordance`, `detect_modal_or_dialog`, `analyze_spider_structural_relationships`, `diagnose_failure_cause`
   - `display_telemetry.rs`: `compute_preview_delta`, `apply_annotation_mutation`, `query_action_ledger_dsl`, `compute_action_ledger_health`, `extract_dom_ui_tuples`
   - `shell_guard.rs`: `ShellCommandGuard`, `CommandInspection`, `CommandAction` (Heredoc ADR-0015, Terminal Dump ADR-0016, UTF-8 pipeline preservation)
2. **`DOMAIN` (`src/domain/` — 34 modules)** — Pure domain entities, state models, and `24` formal `I<Name>` traits in `interfaces/mod.rs` (`// [LAYER: DOMAIN]`, **0% I/O**, `<= 3` structs per file):
   - `interfaces/mod.rs`: `ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal`, `IWorkspaceSnapshotCas`, `IModelPricingRegistry`, `ICodexAccountStore`, `IProjectRunner`, `ISpatialMediator`, `ISpatialGroundingGate`, `IExecutionAssuranceGate`, `IEnvironmentalObserver`, `IBehavioralKnowledgeStore`, `IBehavioralStore`, `ISwarmIntelligenceConsultant`, `IEnvironmentalDiffer`, `ISpatialGeometry`, `IComputerUseOrchestrator`, `IWorkspacePromptProbe`
   - `types/`: `messages.rs`, `runtime.rs`, `account_types.rs` (`ChatGptAccountCredentials`, `ChatGptAccountSummary`, `ChatGptAccountStoreData`), `provider_registry.rs` (`43` canonical `PROVIDER_DESCRIPTORS`, `WireDialect`, `ProviderChain`, `resolve_provider_route`), `model_catalog.rs` (`ModelPricingCatalog`), `posture_types.rs` (`ExecutionPosture`, `RepoConstitution`, `evaluate_tool_posture`), `progress.rs`, `tick_result.rs`, `vfs_types.rs`, `tool_types.rs`, `tool_schedule.rs`, `skill_types.rs`, `computer_use_types.rs` (`UiElement`, `VirtualWindow`), `spatial_types.rs` (`SpatialTarget`, `SpatialScene`, `SpatialActionContract`), `environmental_types.rs` (`EnvironmentalObservation`, `EnvironmentalRegion`, `BehavioralAffordance`), `swarm_intelligence_types.rs` (`SwarmConsultationRequest`, `SwarmConsultationResponse`, `SpecialistInsight`), `learned_knowledge_types.rs` (`LearnedInteractionPattern`, `LearnedWorkflowProcedure`, `EnvironmentalReconciliationReport`)
   - `arena.rs`, `memory.rs`, `compaction.rs`, `swarm.rs`, `tool_parser.rs`, `computer_use.rs` (`VirtualDisplayState`), `skill_tree.rs` (`SkillTreeCatalog`), `spatial_action.rs` (`GroundedSpatialTarget`, `SpatialExecutionReceipt`), `spatial_mediator.rs` (`SpatialActionMediator`, `SpatialActionPlan`, `SpatialInteractionAudit`), `prompt_spec.rs` (`SystemPromptSpec`, `ModelPromptFamily`, `PromptSectionBudget`), `pricing_and_taxonomy.rs` (`BillingMetering`, `ErrorEnvelope`), `prefix_and_manifest.rs` (`PrefixStabilityTracker`, `RequestManifest`), `turn_receipt.rs` (`TurnReceipt`)
3. **`INFRASTRUCTURE` (`src/infrastructure/` — 16 modules)** — Concrete I/O adapters implementing `src/domain/interfaces/` (`// [LAYER: INFRASTRUCTURE]`):
   - `http_provider.rs`, `wire_dialects.rs` (OpenAI Chat, OpenAI `/v1/responses`, Anthropic `/v1/messages` with dangling `tool_use` repair & adaptive thinking, Google Gemini), `sse_and_response_parsers.rs`, `codex_oauth.rs` (`CodexAccountStore` + `127.0.0.1:1455` loopback callback + single-flight refresh), `project_runner.rs` (`ProjectRunner` implementing `IProjectRunner` with `256 KiB` bounded output & `cwd` escape guard), `pricing_updater.rs` (`ModelPricingRegistry`), `scripted_provider.rs`, `vfs_store.rs`, `workspace_cas.rs` (`WorkspaceSnapshotCas`), `transaction_journal.rs`, `behavioral_store.rs` (`BehavioralKnowledgeStore`), `shell_runner.rs`, `fs_tools.rs`, `search_tools.rs`
4. **`CORE` (`src/core/` — 23 modules)** — Application use-case orchestration (`// [LAYER: CORE]`):
   - `engine.rs`, `turn_orchestrator.rs`, `batch_executor.rs`, `computer_use_orchestrator.rs` (`ComputerUseOrchestrator`), `spatial_engine.rs` (`SpatialInteractionEngine`), `slash_and_swarm.rs`, `session_store.rs`, `tool_governor.rs` (`FileReadTracker`, `ToolLoopBreaker`), `tool_specs.rs` (`31` built-in tools including `tool_search` & `execute_tools`), `tool_registry.rs`, `request_queue.rs`, `context_inspector.rs`, `flight_runner.rs`, `benchmark_runner.rs`, `substrate_tests.rs`, `spatial_tests.rs`, `computer_use_tests.rs`, `zenith_tests.rs`, `codewhale_zenith_tests.rs`, `codex_oauth_e2e_tests.rs`, `zenith_hardening_tests.rs`
5. **`UI` (`src/ui/` — 7 modules, `src/lib.rs`, `src/main.rs`, `native/`)** — CLI presenters, multiplexed stdio JSONL worker, RPC dispatch, Composition Roots, and Native macOS AppKit Client (`// [LAYER: UI]`):
   - `src/ui/`: `cli_presenter.rs`, `onboarding.rs`, `stdio_worker.rs`, `worker_handlers.rs`, `provider_rpc.rs`, `rpc_dispatch.rs`, `mod.rs`
   - `src/lib.rs`, `src/main.rs`
   - `native/Sources/Heav3nsCode/`: Native macOS AppKit client decomposed into sovereign modules under Tier 1 Optimal caps (`WorkspaceWindow.swift`, `WorkspaceAttachmentViews.swift`, `WorkspacePostureAndReceiptViews.swift`, `WorkspaceWindow+OmnibarAndPosture.swift`, `WorkspaceWindow+Events.swift`, `WorkspaceWindow+Execution.swift`, `WorkspaceWindow+Inspection.swift`, `WorkspaceWindow+Navigation.swift`, `WorkspaceWindow+Swarm.swift`, `WorkspaceWindow+SwarmSettings.swift`) with explicit `dispose()` lifecycle hygiene across all Swift view/controller classes.

---

## Phase 2: The Five Mandatory Coding Rules

1. **Rule 1 (`[LAYER: <ZONE>]` Header Tag)**: Every `.rs` and `.swift` file starts with `// [LAYER: <ZONE>]` on line 1; every `.md` file starts with `<!-- [LAYER: <ZONE>] -->`.
2. **Rule 2 (Principle of Geographic Alignment)**: A file's physical path (`src/<zone>/...` or `native/...`) must match its `[LAYER: <ZONE>]` header (`SPI-208`).
3. **Rule 3 (Import Depth & Single-Line `use`/`import` Discipline)**: Keep `use crate::...;` statements on a single line so polyglot scanners resolve all symbol edges; never introduce circular dependencies (`SPI-201`).
4. **Rule 4 (Contractual Sovereignty `I<ClassName>`)**: Every concrete adapter in `src/infrastructure/` consumed by `src/core/` must have a corresponding `pub trait I<StructName>` in `src/domain/interfaces/mod.rs` (`SPI-107`).
5. **Rule 5 (Module Size & Simplicity Caps)**: Every file must remain in **Tier 1 Optimal (`< 800` lines, `SPI-203`)**; every `src/domain/` file must contain `<= 3` structs (`<= 1` struct if `> 500` lines).

---

## Phase 3: Pre-Commit Verification Checklist

Run these commands before concluding any change:
```bash
# 1. Rust unit, spatial, computer-use, zenith & integration test suite (99 tests + 5-scenario Flight Test)
cargo test

# 2. Native macOS AppKit test suite (323 unit and integration tests)
cd native && swift test && cd ..

# 3. Strict Joyzoning & Spider audit (must exit 0 with 100% Stability Index)
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict

# 4. StabilityDoctor architectural health check (Grade A+ — 100% SOVEREIGN)
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --doctor

# 5. Documentation guardrails & link verification
node bin/docs-check.mjs
```
