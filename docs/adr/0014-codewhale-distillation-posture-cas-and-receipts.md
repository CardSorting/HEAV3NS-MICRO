<!-- [LAYER: UI] -->
# ADR-0014: Codewhale Zenith Distillation — 5-Stop Execution Posture, Pure-Rust Workspace CAS, Seatbelt Sandboxing, KV-Cache Pinning & Turn Receipts

## Status

**Accepted**

## Context

While **HEAV3NS-MICRO** established a deterministic 5-Zone sovereign runtime (`ADR-0001`..`ADR-0013`), scaling interactive pair-programming across both the CLI REPL and the native macOS AppKit IDE (`HEAV3NS-CODE`) exposed five last-mile operational ceilings inspired by high-assurance agent runtimes (`Codewhale`):

1. **KV-Cache Thrashing on Mode Changes & Unbounded Tool Schemas**: Dynamically removing mutating tools from the wire tool array when entering read-only planning modes invalidates the provider's KV-cache prefix across the entire session. Similarly, unlocking specialist tools without an LRU bound causes monotonic schema growth.
2. **Destructive Git Rewinds vs. Safe Per-Turn Undo**: Relying on `git checkout` or manual rollback risks clobbering uncommitted user edits made *after* an agent turn completes, and fails completely in non-Git directories.
3. **Subprocess Pipe Deadlocks & Escape Paths**: Sequential `stdout`/`stderr` reads can deadlock when a child process fills OS pipe buffers (`> 64 KiB`) on both streams simultaneously, and shell subprocesses lacked parameterized macOS Seatbelt (`/usr/bin/sandbox-exec`) containment and dual-scope credential denylists (`.env` blocked in model context while `.env.example` remains readable).
4. **Opaque Billing, Misclassified `402` vs `429` Errors & Unverified Claims**: Reporting `$0.0000` for unpriced third-party models misleads operators; retrying HTTP `402` / `insufficient_quota` wastes latency; and turn completions lacked an auditable receipt bounding what the agent actually verified.
5. **Single-Threaded Stdio Worker Blocking**: Running long LLM/tool turns synchronously inside the stdio JSON-RPC read loop prevented concurrent `snapshot`, `task.stop`, `approval.resolve`, and `posture.set` RPC dispatch from the native macOS UI.

## Decision

We distill and harden six cohesive mechanisms across all five Joyzoning layers without adding external runtime bloat or violating the `< 800` line Tier 1 Optimal ceiling:

1. **5-Stop `ExecutionPosture` Dial & Fail-Closed `RepoConstitution` (`DOMAIN` + `CORE` + `UI`)**:
   - Introduce `ExecutionPosture` (`Ask` -> `AcceptEdits` -> `Plan` -> `AutoReview` -> `FullAccess`, stepped via `Shift+Tab` in AppKit or `/posture` in REPL) in [`posture_types.rs`](../../src/domain/types/posture_types.rs).
   - Enforce `Plan` mode centrally inside `evaluate_tool_posture` *without* mutating the wire `tools[]` schema, preserving 100% KV-cache prefix stability across mode switches.
   - Parse `.heav3ns/constitution.json` via `RepoConstitution::parse_strict`: malformed JSON fails **closed** (`AuthorizationVerdict::AskUser`) rather than silently defaulting to permissive execution.
   - Enforce `FileReadTracker` (`read-before-write` + FNV-1a external disk staleness detection) and hard-ceiling `ToolLoopBreaker` in [`tool_governor.rs`](../../src/core/tool_governor.rs).

2. **Parameterized macOS Seatbelt Sandbox, Concurrent Pipe Drain & Dual-Scope Credential Guard (`PLUMBING` + `INFRASTRUCTURE`)**:
   - Generate parameterized SBPL profiles (`-DWORKSPACE_ROOT=...`) for `/usr/bin/sandbox-exec` in [`seatbelt_profile.rs`](../../src/plumbing/seatbelt_profile.rs), blocking writes to `.git/hooks`, `.git/config`, `~/.ssh`, `~/.aws`, and `~/.gnupg`.
   - Drain `stdout` and `stderr` concurrently via `tokio::join!` with bounded capture caps and Unix `.process_group(0)` tree termination (`SIGKILL` to `-pgid`) in [`shell_runner.rs`](../../src/infrastructure/shell_runner.rs).
   - Enforce `CredentialDenyScope` (`Process` vs `ModelContext`) in [`secret_redactor.rs`](../../src/plumbing/secret_redactor.rs), blocking `.env*` secrets in model read/search tools while explicitly permitting `.env.example` / `.env.template`.

3. **Pure-Rust Content-Addressable `WorkspaceSnapshotCas` (`DOMAIN` + `INFRASTRUCTURE`)**:
   - Implement `IWorkspaceSnapshotCas` in [`workspace_cas.rs`](../../src/infrastructure/workspace_cas.rs) using SHA-256 blob storage under `.heav3ns/cas/`.
   - Capture automatic pre-turn and post-turn trees, enabling non-Git `/diff`, full `/restore`, and path-scoped `/undo` (`⌥⌘Z` in AppKit) that checks current disk hashes against the post-turn snapshot and refuses with `WouldBlock` if the user manually edited the file after the turn finished.

4. **Session-Pinned `PrefixStabilityTracker`, `RequestManifest`, Honest Pricing & `TurnReceipt` (`DOMAIN` + `CORE`)**:
   - Track wire prefix hashes via `PrefixStabilityTracker` (`PinnedInitial`, `Stable`, `Repinned`, `Drift`) and emit exact byte-summed `RequestManifest` (`heav3ns.request_manifest/v1`) in [`prefix_and_manifest.rs`](../../src/domain/prefix_and_manifest.rs). Inject mid-session posture/memory updates as append-only `<context_update>` blocks rather than mutating `Role::System`.
   - Cap dynamically unlocked deferred tools (`tool_search`) at an 8-slot LRU (`MAX_UNLOCKED_DEFERRED_TOOLS = 8`) and add declarative `execute_tools` batch execution (`MAX_DECLARATIVE_BATCH_STEPS = 32`) in [`batch_executor.rs`](../../src/core/batch_executor.rs).
   - Separate non-recoverable `ErrorCategory::Budget` (`402` / `insufficient_quota`) from retryable `ErrorCategory::RateLimit` (`429`), tag unsent `401`/`403` failures with `CREDENTIAL_REJECTED_UNSENT_CODE` for AppKit composer rescue, report unpriced models honestly as `"unknown"`, and emit `TurnReceipt` (`heav3ns.receipt/v1`) on every completed turn.

5. **Multiplexed Async JSON-RPC Worker & Native macOS AppKit Ergonomics (`UI` + `native/`)**:
   - Multiplex background `task.submit` execution onto `tokio::spawn` in [`stdio_worker.rs`](../../src/ui/stdio_worker.rs) and [`worker_handlers.rs`](../../src/ui/worker_handlers.rs) so `task.stop`, `approval.resolve` (`once` / `allow_session` / `deny`), `posture.set`, `cas.undo`, `receipts.list`, and `request.preview` remain immediately responsive.
   - Surface the 5-stop posture pill (`Shift+Tab`), right-aligned `⌘K` Omnibar shortcut badges, 3-choice session approval bar, live `TurnReceiptSummaryBar`, and `.requestPreview` (`⇧⌘I`) / `.receipts` inspectors in [`WorkspacePostureAndReceiptViews.swift`](../../native/Sources/Heav3nsCode/WorkspacePostureAndReceiptViews.swift) and [`WorkspaceWindow+OmnibarAndPosture.swift`](../../native/Sources/Heav3nsCode/WorkspaceWindow+OmnibarAndPosture.swift).

## Consequences

- **Positive**: Zero KV-cache invalidations when toggling `Plan` / `Ask` / `AcceptEdits` / `AutoReview` / `FullAccess`; deadlock-free subprocess I/O; safe non-destructive `/undo` even outside Git repositories; honest cost/claim reporting; and instant non-blocking AppKit UI responsiveness during long turns.
- **Trade-offs**: Pre-turn and post-turn CAS snapshots scan workspace files (`<= 4 MiB` per file, skipping `.git`, `target`, `node_modules`, `.build`, `.heav3ns`), bounded to sub-15ms overhead on typical repositories via SHA-256 deduplication.

## Verification

```bash
cargo test core::codewhale_zenith_tests
cd native && swift test --filter PostureOmnibarReceiptsTests
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```

