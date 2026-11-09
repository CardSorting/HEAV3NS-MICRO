<!-- [LAYER: UI] -->
# Code-to-Doc Map (`HEAV3NS-MICRO`)

Use this lookup table whenever you modify code in `src/` or `native/` to locate the corresponding documentation pages and papers to update.

## Stateless Plumbing (`src/plumbing/` — 10 Modules)

| Source | Documentation |
| :--- | :--- |
| `src/plumbing/hash_and_math.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) |
| `src/plumbing/text_tokens.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) |
| `src/plumbing/json_repair.rs` | [`docs/papers/whitepaper.md`](papers/whitepaper.md) · [`docs/adr/0003-self-healing-tool-call-parser.md`](adr/0003-self-healing-tool-call-parser.md) |
| `src/plumbing/secret_redactor.rs` | [`SECURITY.md`](../SECURITY.md) · [`docs/adr/0008-secret-redaction-and-security-policy.md`](adr/0008-secret-redaction-and-security-policy.md) |
| `src/plumbing/fuzzy_patch.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`README.md`](../README.md) |
| `src/plumbing/vision_and_dom.rs` | [`docs/papers/ascendant-intelligence.md`](papers/ascendant-intelligence.md) · [`docs/adr/0011-sovereign-intelligence-and-spatial-computer-use.md`](adr/0011-sovereign-intelligence-and-spatial-computer-use.md) |
| `src/plumbing/spatial_geometry.rs` | [`docs/papers/whitepaper.md`](papers/whitepaper.md) · [`docs/adr/0011-sovereign-intelligence-and-spatial-computer-use.md`](adr/0011-sovereign-intelligence-and-spatial-computer-use.md) |
| `src/plumbing/environmental_diff.rs` | [`doc.md`](../doc.md) (`SPIDER`) · [`docs/papers/ascendant-intelligence.md`](papers/ascendant-intelligence.md) |
| `src/plumbing/display_telemetry.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) |

## Domain & Contracts (`src/domain/` — 25 Modules)

| Source | Documentation |
| :--- | :--- |
| `src/domain/interfaces/mod.rs` | [`docs/CONSTITUTION.md`](CONSTITUTION.md) · [`docs/adr/0007-contractual-sovereignty-and-i-interfaces.md`](adr/0007-contractual-sovereignty-and-i-interfaces.md) · [`docs/api/public-api.md`](api/public-api.md) |
| `src/domain/types/*.rs` (`14` modules) | [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/domain/arena.rs` | [`doc.md`](../doc.md) (`REWIND`) · [`docs/adr/0002-arena-allocator-and-o1-tick-rewind.md`](adr/0002-arena-allocator-and-o1-tick-rewind.md) |
| `src/domain/memory.rs` | [`doc.md`](../doc.md) (`BROCCOLI`) · [`docs/kb/knowledge-handling-architecture.md`](kb/knowledge-handling-architecture.md) |
| `src/domain/compaction.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`VALIDATION_REPORT.md`](../VALIDATION_REPORT.md) |
| `src/domain/swarm.rs` | [`doc.md`](../doc.md) (`SYNTHETIC SWARM`) · [`docs/papers/ascendant-intelligence.md`](papers/ascendant-intelligence.md) |
| `src/domain/tool_parser.rs` | [`docs/adr/0003-self-healing-tool-call-parser.md`](adr/0003-self-healing-tool-call-parser.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/domain/computer_use.rs` | [`doc.md`](../doc.md) (`COMPUTER USE`) · [`docs/adr/0011-sovereign-intelligence-and-spatial-computer-use.md`](adr/0011-sovereign-intelligence-and-spatial-computer-use.md) |
| `src/domain/skill_tree.rs` | [`doc.md`](../doc.md) (`BROCCOLI`) · [`docs/papers/ascendant-intelligence.md`](papers/ascendant-intelligence.md) |
| `src/domain/spatial_action.rs` & `spatial_mediator.rs` | [`docs/papers/whitepaper.md`](papers/whitepaper.md) · [`docs/adr/0011-sovereign-intelligence-and-spatial-computer-use.md`](adr/0011-sovereign-intelligence-and-spatial-computer-use.md) |

## Infrastructure Adapters (`src/infrastructure/` — 9 Modules)

| Source | Documentation |
| :--- | :--- |
| `src/infrastructure/http_provider.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`README.md`](../README.md) |
| `src/infrastructure/scripted_provider.rs` | [`VALIDATION_REPORT.md`](../VALIDATION_REPORT.md) · [`docs/adr/0006-stdio-jsonl-and-flight-test-harness.md`](adr/0006-stdio-jsonl-and-flight-test-harness.md) |
| `src/infrastructure/vfs_store.rs` | [`doc.md`](../doc.md) (`REWIND`) · [`docs/adr/0004-two-phase-vfs-staging-and-transaction-journal.md`](adr/0004-two-phase-vfs-staging-and-transaction-journal.md) |
| `src/infrastructure/transaction_journal.rs` | [`doc.md`](../doc.md) (`REWIND`) · [`docs/playbooks/vfs-commit-conflict-and-journal-rollback.md`](playbooks/vfs-commit-conflict-and-journal-rollback.md) |
| `src/infrastructure/behavioral_store.rs` | [`doc.md`](../doc.md) (`BROCCOLI` & `REWIND`) · [`docs/papers/ascendant-intelligence.md`](papers/ascendant-intelligence.md) |
| `src/infrastructure/shell_runner.rs` | [`SECURITY.md`](../SECURITY.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/infrastructure/fs_tools.rs` & `search_tools.rs` | [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) · [`SECURITY.md`](../SECURITY.md) |

## Core Orchestration (`src/core/` — 14 Modules)

| Source | Documentation |
| :--- | :--- |
| `src/core/engine.rs` & `slash_and_swarm.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`README.md`](../README.md) |
| `src/core/computer_use_orchestrator.rs` | [`doc.md`](../doc.md) · [`docs/papers/ascendant-intelligence.md`](papers/ascendant-intelligence.md) · [`docs/adr/0011-sovereign-intelligence-and-spatial-computer-use.md`](adr/0011-sovereign-intelligence-and-spatial-computer-use.md) |
| `src/core/spatial_engine.rs` | [`docs/papers/whitepaper.md`](papers/whitepaper.md) · [`docs/adr/0011-sovereign-intelligence-and-spatial-computer-use.md`](adr/0011-sovereign-intelligence-and-spatial-computer-use.md) |
| `src/core/session_store.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`README.md`](../README.md) |
| `src/core/tool_governor.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/adr/0005-dag-wave-scheduler-concurrency.md`](adr/0005-dag-wave-scheduler-concurrency.md) |
| `src/core/tool_specs.rs` & `tool_registry.rs` | [`README.md`](../README.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/core/request_queue.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) |
| `src/core/flight_runner.rs`, `substrate_tests.rs`, `spatial_tests.rs`, `computer_use_tests.rs` | [`VALIDATION_REPORT.md`](../VALIDATION_REPORT.md) · [`README.md`](../README.md) |

## UI, IPC Protocols & Native macOS AppKit Client (`src/ui/`, `native/`)

| Source | Documentation |
| :--- | :--- |
| `src/ui/cli_presenter.rs`, `stdio_worker.rs`, `rpc_dispatch.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/adr/0010-native-desktop-integration-and-rpc-protocol.md`](adr/0010-native-desktop-integration-and-rpc-protocol.md) |
| `src/lib.rs` & `src/main.rs` | [`README.md`](../README.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `native/Sources/Heav3nsCode/WorkspaceWindow*.swift` | [`native/PRODUCT.md`](../native/PRODUCT.md) · [`native/DESIGN.md`](../native/DESIGN.md) · [`native/ROADMAP.md`](../native/ROADMAP.md) |
| `native/Sources/Heav3nsCode/CodeOutline.swift` & `InspectionNavigation.swift` | [`native/ROADMAP.md`](../native/ROADMAP.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `native/Sources/Heav3nsCode/RuntimeClient.swift` & `ControlFrameReader.swift` | [`docs/adr/0010-native-desktop-integration-and-rpc-protocol.md`](adr/0010-native-desktop-integration-and-rpc-protocol.md) · [`docs/playbooks/desktop-ipc-and-unconfirmed-submission-recovery.md`](playbooks/desktop-ipc-and-unconfirmed-submission-recovery.md) |
