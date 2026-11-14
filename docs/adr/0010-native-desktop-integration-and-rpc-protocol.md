<!-- [LAYER: UI] -->
# ADR-0010: Native Desktop AppKit Integration, Modular UI Decomposition, and Stdio JSON-RPC Protocol

- **Status**: Accepted
- **Date**: 2026-11-08
- **Deciders**: HEAV3NS Core Architecture Team
- **Technical Story**: [`src/ui/stdio_worker.rs`](../../src/ui/stdio_worker.rs) · [`src/ui/rpc_dispatch.rs`](../../src/ui/rpc_dispatch.rs) · [`native/Sources/Heav3nsCode/`](../../native/Sources/Heav3nsCode/)

---

## Status

Accepted.

## Context

The native macOS desktop client for `HEAV3NS-CODE` was previously coupled to legacy node/mock runtimes and had grown into monolithic source files exceeding 3,700 lines (`WorkspaceWindow.swift`), directly violating Joyzoning Rule 5 (`SPI-203`: files must remain in Tier 1 Optimal, `< 800` lines).

Additionally, the desktop client required a high-performance, deterministic IPC mechanism to drive multi-agent swarm operations, live file inspections, Base64 binary page rendering, unified git diffing, spider audits, and safe cancellation without stalling or locking the UI thread.

### Decision Drivers
1. **Direct Substrate Integration**: Connect AppKit directly to the high-performance `heav3ns-micro` Rust runtime via standard I/O streams without network sockets or third-party daemon dependencies.
2. **Strict Joyzoning Line Caps (`SPI-203`)**: Deconstruct all monolithic native Swift source files so that 100% of workspace files remain strictly under 800 lines.
3. **Contractual Framing & Versioning**: Ensure the transport stream strictly adheres to the `"v": 1` control frame contract expected by `ControlFrameReader.swift`.
4. **Lifecycle & Memory Safety (`SPI-110`)**: Implement deterministic `dispose()` lifecycle hooks across all window controllers and auxiliary views.

---

## Decision

We executed a comprehensive three-pillar architectural integration:

### 1. Dual-Layer Bidirectional JSON-RPC Engine (`v: 1`)
- Built `src/ui/stdio_worker.rs` and `src/ui/rpc_dispatch.rs` into the Rust substrate.
- Every frame emitted to stdout incorporates the mandatory protocol version marker (`"v": 1`).
- Implemented real production handlers for:
  - `files.list`, `files.read`, and chunked Base64 `files.readBinary`.
  - `changes.read` (generating unified diffs against Git working tree and staging).
  - `spider.check` (performing syntax, import, and circular dependency checks).
  - `memory.list`, `memory.save`, and `memory.remove` (binding directly to `src/domain/memory.rs`).
  - `context.inspect` and `prompt.inspect` (token budget HUD and KV-cache prefix telemetry).
  - `task.submit`, `task.cancel`, `task.cancelAll`, `task.lookup` (safe non-blocking submission reconciliation).
  - `swarm.plan`, `swarm.next`, `swarm.report` (topological specialist graph execution).
  - `signIn`, `cancel`, `status`, `accounts`, `switch`, `rename`, `remove`, `signOut`, `clearLimit`, `setAuto` (ChatGPT / OpenAI Codex OAuth PKCE loopback flow and multi-account lifecycle via `ICodexAccountStore`).
  - `project.discover`, `project.start`, `project.stop`, `project.status`, `project.configure` (process-backed workspace runner via Composition Root–injected `Arc<dyn IProjectRunner>`).
  - Strict JSON-RPC error contracts: unknown `is_rpc` methods return `UNKNOWN_METHOD` errors, `side.query` propagates `SIDE_QUERY_FAILED` / `SIDE_QUERY_EMPTY` errors, and `shutdown`/`exit` cancels active tokens and stops `project_runner` before a bounded 3s drain.

### 2. Modular Swift UI Decomposition
- Decomposed the 3,772-line `WorkspaceWindow.swift` into 8 sovereign modules:
  - `WorkspaceWindow.swift` (791 lines): Core lifecycle, stored properties, window/table delegates.
  - `WorkspaceAttachmentViews.swift` (222 lines): Attachment tokens, composer, transcript scroller.
  - `WorkspaceWindow+Swarm.swift` (603 lines): Swarm formations and radar integration.
  - `WorkspaceWindow+SwarmSettings.swift` (223 lines): Swarm configuration and recovery.
  - `WorkspaceWindow+Navigation.swift` (308 lines): Chats switcher, file tree, Git diff navigation.
  - `WorkspaceWindow+Inspection.swift` (557 lines): Inspection viewports (diff, state, spider, context).
  - `WorkspaceWindow+Execution.swift` (515 lines): Draft persistence, instruction queue, checkpoints.
  - `WorkspaceWindow+Events.swift` (776 lines): Runtime event stream, action palette, Pod snapshots.
- Decomposed test harness into `RuntimeClientTestCase.swift`, `RuntimeClientTests.swift`, and `RuntimeClientProtocolTests.swift`.

### 3. Resource Lifecycle & Dispose Protocols (`SPI-110`)
- Added explicit `dispose()` routines across window controllers to cleanly release timers (`viewportBranchTimer`, `draftTimer`, `elapsedTimer`), notification observers, and auxiliary child windows upon window closure or app shutdown.

---

## Consequences

### Positive
- **Zero Monolithic Debt**: 100% of Swift and Rust files in the repository now satisfy Tier 1 Optimal (< 800 lines).
- **Sub-Millisecond IPC Latency**: Direct stdio JSON-RPC transport bypasses socket buffers and network stack overhead.
- **Flawless Test Suite**: All 315 native AppKit tests and all 66 Rust substrate tests pass cleanly with zero failures.
- **Architectural Health**: `joy-check` verifies 100% Stability Index and 0 blocking Joyzoning violations across both layers.

### Negative / Trade-Offs
- Stdio stderr must remain dedicated strictly to diagnostic logs; stdout is reserved exclusively for framed JSON-RPC packets.

---

## Verification

```bash
# 1. Verify Rust substrate unit & integration tests
cargo test

# 2. Verify native macOS AppKit test suite
cd native && swift test && cd ..

# 3. Verify Joyzoning architectural sovereignty
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
```

