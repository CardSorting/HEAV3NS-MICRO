<!-- [LAYER: UI] -->
# Changelog

All notable changes to **HEAV3NS-MICRO** are documented in this file.

## [0.1.0] - 2026-11-08

### Added
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

