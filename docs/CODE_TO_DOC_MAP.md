<!-- [LAYER: UI] -->
# Code-to-Doc Map (`HEAV3NS-MICRO`)

Use this lookup table whenever you modify code in `src/` or `native/` to locate the corresponding documentation pages to update.

## Stateless Plumbing (`src/plumbing/`)

| Source | Documentation |
| :--- | :--- |
| `src/plumbing/hash_and_math.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) |
| `src/plumbing/text_tokens.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) |
| `src/plumbing/json_repair.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`README.md`](../README.md) |
| `src/plumbing/secret_redactor.rs` | [`SECURITY.md`](../SECURITY.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/plumbing/fuzzy_patch.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`README.md`](../README.md) |

## Domain & Contracts (`src/domain/`)

| Source | Documentation |
| :--- | :--- |
| `src/domain/interfaces/mod.rs` | [`docs/CONSTITUTION.md`](CONSTITUTION.md) · [`docs/CODEBASE_STANDARDS.md`](CODEBASE_STANDARDS.md) |
| `src/domain/types/*.rs` | [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/domain/arena.rs` | [`README.md`](../README.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/domain/memory.rs` | [`README.md`](../README.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/domain/compaction.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`VALIDATION_REPORT.md`](../VALIDATION_REPORT.md) |
| `src/domain/swarm.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`README.md`](../README.md) |
| `src/domain/tool_parser.rs` | [`README.md`](../README.md) · [`docs/architecture/current.md`](architecture/current.md) |

## Infrastructure Adapters (`src/infrastructure/`)

| Source | Documentation |
| :--- | :--- |
| `src/infrastructure/http_provider.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`README.md`](../README.md) |
| `src/infrastructure/scripted_provider.rs` | [`VALIDATION_REPORT.md`](../VALIDATION_REPORT.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/infrastructure/vfs_store.rs` | [`README.md`](../README.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/infrastructure/transaction_journal.rs` | [`SECURITY.md`](../SECURITY.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/infrastructure/shell_runner.rs` | [`SECURITY.md`](../SECURITY.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/infrastructure/fs_tools.rs` & `search_tools.rs` | [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) · [`SECURITY.md`](../SECURITY.md) |

## Core Orchestration (`src/core/`)

| Source | Documentation |
| :--- | :--- |
| `src/core/engine.rs` & `slash_and_swarm.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`README.md`](../README.md) |
| `src/core/session_store.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`README.md`](../README.md) |
| `src/core/tool_governor.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`SECURITY.md`](../SECURITY.md) |
| `src/core/tool_specs.rs` & `tool_registry.rs` | [`README.md`](../README.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `src/core/request_queue.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) |
| `src/core/flight_runner.rs` & `substrate_tests.rs` | [`VALIDATION_REPORT.md`](../VALIDATION_REPORT.md) · [`README.md`](../README.md) |

## UI, IPC Protocols & Composition Roots (`src/ui/`, `src/lib.rs`, `src/main.rs`)

| Source | Documentation |
| :--- | :--- |
| `src/ui/cli_presenter.rs` | [`README.md`](../README.md) · [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) |
| `src/ui/stdio_worker.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/adr/0010-native-desktop-integration-and-rpc-protocol.md`](adr/0010-native-desktop-integration-and-rpc-protocol.md) |
| `src/ui/rpc_dispatch.rs` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/adr/0010-native-desktop-integration-and-rpc-protocol.md`](adr/0010-native-desktop-integration-and-rpc-protocol.md) |
| `src/lib.rs` & `src/main.rs` | [`README.md`](../README.md) · [`docs/architecture/current.md`](architecture/current.md) |

## Native macOS AppKit Client (`native/Sources/Heav3nsCode/`)

| Source | Documentation |
| :--- | :--- |
| `WorkspaceWindow.swift` | [`native/PRODUCT.md`](../native/PRODUCT.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `WorkspaceAttachmentViews.swift` | [`native/DESIGN.md`](../native/DESIGN.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `WorkspaceWindow+Navigation.swift` | [`native/ROADMAP.md`](../native/ROADMAP.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `WorkspaceWindow+Inspection.swift` | [`native/PRODUCT.md`](../native/PRODUCT.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `WorkspaceWindow+Execution.swift` | [`native/ROADMAP.md`](../native/ROADMAP.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `WorkspaceWindow+Swarm.swift` & `+SwarmSettings.swift` | [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `WorkspaceWindow+Events.swift` | [`docs/architecture/current.md`](architecture/current.md) · [`docs/adr/0010-native-desktop-integration-and-rpc-protocol.md`](adr/0010-native-desktop-integration-and-rpc-protocol.md) |
| `RuntimeClient.swift` & `ControlFrameReader.swift` | [`docs/adr/0010-native-desktop-integration-and-rpc-protocol.md`](adr/0010-native-desktop-integration-and-rpc-protocol.md) · [`docs/architecture/current.md`](architecture/current.md) |
| `PodsWindow.swift` & `PodLiveView.swift` | [`native/PRODUCT.md`](../native/PRODUCT.md) · [`native/DESIGN.md`](../native/DESIGN.md) |
