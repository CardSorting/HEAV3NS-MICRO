<!-- [LAYER: UI] -->
# PB-06: Desktop IPC Disconnection & Unconfirmed Submission Recovery

- **Playbook ID**: `PB-06`
- **Severity**: `SEV-2`
- **Owner**: Desktop Integration & IPC Engineering Team
- **Trigger Symptoms**: "Check Status" pill in AppKit toolbar, `SUBMISSION_UNCONFIRMED`, worker exit code `99`, frame parser timeout.
- **Governing Architecture**: [`ADR-0010`](../adr/0010-native-desktop-integration-and-rpc-protocol.md) · [`src/ui/stdio_worker.rs`](../../src/ui/stdio_worker.rs) · [`native/Sources/Heav3nsCode/WorkspaceWindow+Execution.swift`](../../native/Sources/Heav3nsCode/WorkspaceWindow+Execution.swift)

---

## 1. Executive Summary & Blast Radius

When the native AppKit client communicates with the `heav3ns-micro` substrate over stdio, network delays, system sleep cycles, or worker restarts can cause an in-flight prompt submission to become unconfirmed. 

Under the **Zero-Replay Invariant**, the native client must never blindly resubmit an unconfirmed task, as doing so could execute mutating tools twice. Instead, the UI preserves the composer draft and displays a non-blocking **"Check Status"** control to verify whether the agent accepted the instruction before the interruption.

---

## 2. Triage & Incident Flowchart

```mermaid
flowchart TD
  START["Unconfirmed Submission or Worker Exit"] --> DRAFT["Verify Local Draft File"]
  DRAFT --> STATUS{"Worker Process Running?"}
  STATUS -->|"No (Process Exited)"| RESTART["Click Reconnect / Re-launch Worker"]
  STATUS -->|"Yes (Process Active)"| LOOKUP["Click Check Status Button"]
  RESTART --> LOOKUP
  LOOKUP --> FOUND{"Task Found in Runtime?"}
  FOUND -->|"Yes (Accepted)"| RECOVER["Adopt Existing Task & Stream History"]
  FOUND -->|"No (Not Accepted)"| EDIT["Draft Preserved in Composer -> Safe to Send"]
```

---

## 3. Step-by-Step Recovery Procedure

### Step 1: Verify Local Draft Preservation
The AppKit client automatically persists composer contents and attachments via `WorkspaceDraftStore`.
```bash
# Inspect the active workspace draft file
cat "$WORKSPACE_ROOT/.heav3ns/drafts/draft.json"
```
*Expected*: The unconfirmed prompt text and file attachments are intact.

### Step 2: Query Unconfirmed Task Status (`task.lookup`)
When the worker process reconnects, click the **"Check Status"** button in the composer notice bar. This issues a lightweight `task.lookup` call:
```json
{
  "jsonrpc": "2.0",
  "id": 101,
  "method": "task.lookup",
  "params": {
    "submissionId": "<UUID>"
  }
}
```

- **If `found: true`**: The substrate recorded the instruction before the transport dropped. The client updates the transcript, attaches to the active run, and follows tool progress in the Walkthrough pane without resubmission.
- **If `found: false`**: The substrate never received or accepted the instruction. The client notifies the user: `"LUMI has no acceptance record for that instruction. Review your draft and send it when ready."`

### Step 3: Verify Substrate Health
Run a fast smoke test to confirm the local Rust runtime is functional:
```bash
cargo run -- --smoke
```
*Expected*: Exits `0` with `[OK] Substrate smoke test passed`.

---

## 4. Prevention & Hardening Invariants

1. **Atomic Draft Write**: `WorkspaceDraftStore` writes drafts to a temporary file before renaming, preventing partial-write corruption.
2. **Idempotent Submission Lookups**: `task.lookup` queries only read-only submission journals; it never mutates transcript state.
3. **Dedicated Stderr Logging**: Never allow diagnostic messages or panic backtraces to write to `stdout`, as this corrupts JSON-RPC frame parsing.

---

## 5. Verification Commands

```bash
# Verify protocol test coverage for lookup timeouts and draft preservation
cd native && swift test --filter RuntimeClientProtocolTests/testWorkspaceOffersStatusRetryAfterLookupTimeoutWithoutLosingDraft && cd ..
```
