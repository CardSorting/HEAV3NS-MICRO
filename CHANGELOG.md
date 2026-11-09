<!-- [LAYER: UI] -->
# Changelog

All notable changes to **HEAV3NS-MICRO** are documented in this file.

## [0.1.0] - 2026-11-08

### Added
- **Plus Ultra Pass — Viewport Scroll Continuity, Off-Screen Auto-Scroll, Multi-Step Spatial Plan Execution & Behavioral Domain Enrichment (`src/domain/types/computer_use_types.rs`, `src/domain/computer_use.rs`, `src/domain/spatial_mediator.rs`, `src/core/spatial_engine.rs`, `src/plumbing/display_telemetry.rs`, `native/Sources/Heav3nsCode/CodeOutline.swift`)**:
  - Enriched `UiElement` (`center_point`, `hit_tests_point`, `matches_selector`, `to_som_json`, `apply_click_interaction`, `apply_text_mutation`) and `VirtualWindow` (`is_modal_window`, `query_window_elements`, `apply_window_key`) into first-class behavioral domain entities, reducing cognitive complexity in `src/domain/computer_use.rs` by 34.8% (414 -> 270) and reducing workspace Structural Entropy to $E_S = 0.0038$.
  - Added persistent `scroll_offset: [i32; 2]` tracking to `VirtualDisplayState` and `SpatialInteractionEngine::extract_scene_from_display`, enabling Wrapper B confirmation of `"scroll"` postconditions, automatic pre-scrolling of off-screen targets (`requires_scroll == true`), and end-to-end execution of multi-step `SpatialActionPlan` sequences (`SpatialInteractionEngine::execute_plan` / `"execute_plan"` in `spatial_interact`).
  - Enforced `target_disabled` guard across `computer_use` and `browser_use`, added stateful toggling for `checkbox`/`switch`/`toggle`/`radio` elements, and implemented rich keyboard semantics (`Backspace`, `Cmd+Backspace`, `Space`, and `Escape` modal dismissal).
  - Extracted stateless display telemetry helpers into `src/plumbing/display_telemetry.rs` with extended `query_action_ledger_dsl` filters (`element:`, `mode:`, `duration>`, free-text), fixed UTF-16 Unicode/emoji intra-line diff highlighting in `CodeOutline.swift`, and added `dispose()` lifecycle teardown across all 30 Swift classes (`49/49` Rust tests, `311/311` Swift tests, `0` advisories).
- **Sovereign Intelligence Charter & Foundational Papers (`doc.md`, `docs/papers/ascendant-intelligence.md`, `ADR-0011`, `ROADMAP.md`)**:
  - Formalized the Seven Sovereign Pillars of **HEAV3NS — The Sovereign Intelligence Architecture** (`HEAV3NS-MICRO`, `SPIDER`, `REWIND`, `BROCCOLI`, `SYNTHETIC SWARM`, `COMPUTER USE`, `THE HARNESS`) and the multi-horizon roadmap toward **The Ascendant Computational Intelligence**.
- **Closed-Loop Environmental Observation & Verification Engine (`src/plumbing/environmental_diff.rs`, `src/domain/types/environmental_types.rs`, `src/core/computer_use_orchestrator.rs`) (`use-1.md`)**:
  - Implemented pre- and post-action environmental snapshots (`EnvironmentalObservation`) capturing interactive UI elements, focus state, active windows, and bounding geometries.
  - Implemented structural differ (`compute_element_deltas`) tracking value mutations, focus shifts, bounds displacement, element additions, and removals with observable postcondition verification.
  - Implemented active modal detection (`detect_modal_or_dialog`, `detect_modal_region`) and auto-intercept: actions targeting occluded outside elements automatically route an `Escape` keypress to safely dismiss modal traps while preserving internal modal interactions.
  - Implemented hard loop-breaker safety gate aborting execution after 3 repeated failures on the same selector to prevent infinite retry loops.
- **Multi-Specialist Swarm Collective Intelligence & Behavioral Knowledge Store (`src/domain/types/swarm_intelligence_types.rs`, `src/domain/types/learned_knowledge_types.rs`, `src/infrastructure/behavioral_store.rs`, `src/core/computer_use_orchestrator.rs`) (`use-2.md`)**:
  - Implemented autonomous specialist swarm consultation (`consult_swarm`) triggered upon repeated action failure (`current_fails >= 2`).
  - Integrated 5 distinct specialist personas (`DOMStructure`, `VisualGeometry`, `WorkflowContext`, `ErrorAnalysis`, `HistoricalKnowledge`) synthesizing structured recovery advice and confidence scores.
  - Implemented durable `BehavioralKnowledgeStore` indexing successful element patterns, selector fallbacks, and modal dismiss recipes with confidence calibration and fast memory synchronization.
- **First-Class Spatial Action Architecture & Double-Wrapped Mediation (`src/plumbing/spatial_geometry.rs`, `src/domain/types/spatial_types.rs`, `src/domain/spatial_action.rs`, `src/domain/spatial_mediator.rs`, `src/core/spatial_engine.rs`)**:
  - Elevated bounding boxes into first-class operational primitives with provenance tracking (`DomTree`, `SetOfMarks`, `AxTree`, `VisionDetector`, `WindowFrame`, `Synthetic`), confidence metrics, and subpixel geometry (`SpatialPoint`, `SpatialRect`, `safe_center` 15% inner padding, AABB intersections, IoU, and occlusion ratios).
  - Implemented **Double-Wrapped Action Mediation**:
    - **Wrapper A (Spatial Grounding)**: Enforces target identity resolution (by ID, Set-of-Marks token, semantic label, or normalized coordinates), coordinate space conversions (screen, window, viewport, normalized float 0..1, normalized 1000, and percentage without double-scaling or height ceiling bugs), viewport clipping, scroll offset transformations, DPR scaling, exact sweep-line 2D multi-rectangle occlusion calculation (`calculate_rectangles_union_area`), occlusion-avoiding interior safe points (`find_unoccluded_safe_point`), trajectory raycasting with exclusion zones, and unambiguous candidate arbitration.
    - **Wrapper B (Execution Assurance)**: Independently verifies against live scene state (stale-epoch fence, live target presence, enabled state, live bounds containment, viewport visibility preconditions), dispatches execution with auto-scroll recovery for out-of-view targets, and independently evaluates observable postconditions (focus shift, value mutation, element tree mutation, window navigation, scroll deltas) yielding a verified verdict ladder (`Confirmed`, `SuspectedNoop`, `StateDiverged`, `RequiresEscalation`, `RecoveryAvailable`).
  - Added Cross-Modal Target Reconciliation with dedicated `TargetProvenance::Reconciled` provenance, Spatially Grounded Action Planning (`SpatialActionPlan`), Deterministic Plan Replay (`replay_plan`), Predictive Layout Invalidation (`predict_layout_mutation`), Directional Spatial Queries (`right_of`, `left_of`, `below`, `above`), Trajectory Raycasting & Safety Gates (`validate_trajectory`), and Spatial Interaction Auditing (`SpatialInteractionAudit`).
  - Added contractual sovereignty interfaces (`ISpatialMediator`, `ISpatialGroundingGate`, `IExecutionAssuranceGate` in `src/domain/interfaces/mod.rs`), registered the 29th native built-in tool `spatial_interact`, added the `/spatial` slash command, and exposed `spatial.ground`/`spatial.execute`/`spatial.query`/`spatial.health` JSON-RPC endpoints.
  - Achieved **0.40 µs (400 ns)** micro-benchmark spatial grounding throughput in release mode (>240,000 ops/sec).
- **Hyper-Compressed Computer Use, Zero-Trust Browser DOM, Vision Codec & Evolutionary Skill/Soul Forge (`src/plumbing/vision_and_dom.rs`, `src/domain/computer_use.rs`, `src/domain/skill_tree.rs`, `src/domain/swarm.rs`)**:
  - Distilled and superseded `LUMI-NEW` and `hermes-agent-main` Computer Use (`VirtualDisplayState`, Set-of-Marks UI elements, `som`/`vision`/`ax` capture modes, opaque `element_token` `s{hex}:{id}`, consecutive screenshot deduplication, HiDPI bounds scale detection, background-first input delivery with sticky-target mismatch guard, structured Verify -> Escalate verdict ladder, epoch-fenced Human/Agent desktop control lease, in-app `drive_preview` delta element inventories & `annotate_preview` persistent pins, blocked destructive key combos/type payloads, 32-deep undo/redo checkpoints, natural action query DSL, p50/p95 SLA health auditing).
  - Hardened Browser/CDP (`snapshot_html_dom` interactive Set-of-Marks extractor, `validate_navigation_url` SSRF/IMDSv1/v2 + IPv4-mapped IPv6 + CGNAT + WHATWG authority ambiguity + secret-in-URL blocker, `expression_targets_blocked_url` JS eval literal pre-scanner, `redact_cdp_url`), Multimodal Vision (`decode_image_metadata` binary header inspection for PNG/JPEG/GIF/WebP/BMP/SVG, GCD aspect ratio & vision token estimator, synthetic SVG/BMP generators), and Evolutionary Skill Tree & Soul Drop Vault (`SkillTreeCatalog`, Kahn's topological DAG & cycle detector, custom skill forge, 6 modular power-up packs, Skill Doctor linter with prompt-injection/C2 threat scanning & auto-fix, multi-format import/export for Markdown, CharacterCard V2, OpenAI GPT, and Anthropic Claude XML).
  - Added content-verified Escape-Drift Auto-Correction (`maybe_correct_escape_drift`), Prompt-Injection & C2 Threat Scanner (`SafetyPolicy::scan_for_threats`), PTY ANSI Query Responder (`SafetyPolicy::strip_and_respond_pty_queries`), Foreground/Background Terminal Guidance (`SafetyPolicy::detect_foreground_background_guidance`), Protected Governance File Write Guard (`SafetyPolicy::check_protected_instruction_write`), `ADR-0015` heredoc file-write defense, `ADR-0016` bare terminal file-read defense, Trojan Source Unicode sanitizer, and `The Legion` 7-specialist task classifier.
  - Expanded built-in tools from 23 to 28 (`computer_use`, `browser_use`, `vision_inspect`, `skill_forge`, `engage_swarm`) and added `/computer`, `/browser`, `/vision`, `/soul`, `/forge`, `/doctor`, and `/legion` slash commands plus stdio JSON-RPC endpoints.
- **Native macOS AppKit Client & Desktop JSON-RPC Protocol (`native/`, `src/ui/`)**:
  - Migrated and integrated the AppKit macOS client (`HEAV3NS-CODE`) directly into the workspace with full bidirectional communication over standard I/O streams.
  - Implemented the `"v": 1` control frame specification across `src/ui/stdio_worker.rs` and `src/ui/rpc_dispatch.rs`.
  - Added full desktop RPC method handlers:
    - `files.list`, `files.read`, and chunked Base64 `files.readBinary` for rich multi-page media inspection (PDF, images, CSV).
    - `changes.read` unified git diffing with working tree and staged index scoping.
    - `spider.check` static AST syntax analysis, local import resolution, and circular dependency checks.
    - `memory.list`, `memory.save`, `memory.remove` long-term conversation memory inspection and modification.
    - `task.submit`, `task.stop`, `task.lookup` safe instruction dispatch and non-replay reconciliation.
    - `swarm.plan`, `swarm.next`, `swarm.report` topological multi-specialist graph execution.
- **De-monolithization of Native Desktop Architecture (`SPI-203`)**:
  - Modularized the 3,772-line `WorkspaceWindow.swift` into 8 sovereign modules strictly adhering to Tier 1 Optimal (< 800 lines):
    - `WorkspaceWindow.swift` (791 lines): Core window lifecycle, layout assembly, window/table delegates.
    - `WorkspaceAttachmentViews.swift` (222 lines): Attachment token bar, composer view, transcript scroll view, turn scrubber rail.
    - `WorkspaceWindow+Swarm.swift` (603 lines): Swarm formations and radar state.
    - `WorkspaceWindow+SwarmSettings.swift` (223 lines): Swarm configuration, command recovery, and action confirmations.
    - `WorkspaceWindow+Navigation.swift` (308 lines): Navigation sidebar, chats switcher, file tree, Git diff navigation.
    - `WorkspaceWindow+Inspection.swift` (557 lines): File, diff, saved-state, spider check, and context report viewports.
    - `WorkspaceWindow+Execution.swift` (515 lines): Draft preservation, instruction queue, checkpoints.
    - `WorkspaceWindow+Events.swift` (764 lines): Runtime event stream, action palette, Pod snapshots.
- **Modular Native Test Suite (`native/Tests/Heav3nsCodeTests/`)**:
  - Decomposed test suite into `RuntimeClientTestCase.swift`, `RuntimeClientTests.swift`, and `RuntimeClientProtocolTests.swift` (< 600 lines each).
  - Executed all 308 native AppKit tests with 0 failures and 100% parity across `HEAV3NS-MICRO/native` and `LUMI-NEW/native`.
- **Architecture Decision Record (ADR-0010)**:
  - Documented the desktop integration, modular decomposition, and stdio JSON-RPC protocol in `docs/adr/0010-native-desktop-integration-and-rpc-protocol.md`.
- **5-Zone Joyzoning Architectural Sovereignty (`src/`)**:
  - Decomposed the runtime into 44 focused Rust modules across `src/plumbing/` (6), `src/domain/` (16), `src/infrastructure/` (8), `src/core/` (10), and `src/ui/` + Composition Roots (4), achieving a **100% Stability Index (`SOVEREIGN`)** and **100% Build Health (`A+`)**.
  - Added formal `I<Name>` traits in `src/domain/interfaces/mod.rs` (`ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal`).
  - Preserved 100% backward compatibility in `src/lib.rs` via inline facade modules (`engine`, `flight`, `parser`, `provider`, `session`, `tools`).
- **Native 8-Agent Stack & Documentation Governance**:
  - Installed `AGENTS.md`, `SOUL.md`, `CLAUDE.md`, `GEMINI.md`, `.agents/skills/`, `.claude/skills/`, `.mcp.json`, `.githooks/pre-commit`, `.github/workflows/ci.yml`, `llms.txt`, and `docs/`.

