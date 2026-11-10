<!-- [LAYER: UI] -->
# Command-Line & Stdio Interface Reference

<p align="center">
  <a href="README.md">API Hub</a> ·
  <a href="public-api.md">Public API</a> ·
  <a href="errors.md">Errors</a> ·
  <a href="../OPERATIONS.md">Operations</a> ·
  <a href="../architecture/current.md">Architecture</a>
</p>

Complete normative reference for command-line arguments, environment variables, slash commands, and the bidirectional Stdio JSON-RPC protocol (`v: 1`) powering the CLI and AppKit native desktop client (`native/`).

---

## 1. CLI Usage & Flags

```text
USAGE:
  heav3ns-micro [OPTIONS] [PROMPT]
```

### Execution Modes & Subcommands

| Flag / Subcommand | Long Flag / Form | Description |
| :--- | :--- | :--- |
| *(none)* | *(default)* | Starts an interactive pair-programming REPL session in the terminal. |
| `context` | `context [--json]` | Displays the visual Context & Prompt Inspector HUD (or JSON telemetry). |
| `doctor` | `doctor` | Runs comprehensive diagnostic audit across credentials, SLA, and substrate. |
| `init` | `init` | Interactive credential and provider onboarding wizard. |
| `demo` | `demo` | Runs offline guided tour and substrate self-test. |
| `-p` | `--prompt <TEXT>` | Executes a single agent turn with the given prompt and exits cleanly. |
| *(none)* | `--flight` / `--benchmark-flight` | Runs the 5-scenario Agent Flight benchmark suite (`FT-01..FT-05`). |
| *(none)* | `--bench` | Runs substrate micro-benchmarks along with the flight suite. |
| *(none)* | `--smoke` | Executes deterministic runtime self-checks and smoke verification. |
| *(none)* | `--stdio` | Runs the headless newline-delimited JSON-RPC worker protocol over stdin/stdout. |

### Configuration Options

| Flag | Parameter | Default | Description |
| :--- | :--- | :--- | :--- |
| `-m` | `--model <NAME>` | `gpt-5.6-terra` | Target LLM model name passed to provider requests. |
| *(none)* | `--provider <NAME>` | `openai` | Protocol format: `openai`, `anthropic`, or `gemini`. |
| *(none)* | `--endpoint <URL>` | *(official default)* | Custom API endpoint URL for self-hosted or proxy models. |
| *(none)* | `--cwd <DIR>` | `.` | Sets the active workspace root directory. |
| *(none)* | `--desktop-worker` | `false` | Enables full desktop IDE RPC dispatch mode for `native/`. |
| *(none)* | `--desktop-account <ACC>` | *(none)* | Configures specific authenticated account context for desktop worker. |
| *(none)* | `--format <FMT>` | `text` | Flight output format: `text` or `json`. |
| `-o` | `--output <PATH>` | *(none)* | Writes the JSON flight benchmark report to a file. |
| `-h` | `--help` | *(none)* | Displays usage instructions and exits. |

---

## 2. Environment Variables

| Variable | Description | Default |
| :--- | :--- | :--- |
| `OPENAI_API_KEY` | Authentication bearer token for OpenAI provider calls. | *(none)* |
| `ANTHROPIC_API_KEY` | Authentication token for Anthropic Claude provider calls. | *(none)* |
| `GEMINI_API_KEY` | Authentication token for Google Gemini provider calls. | *(none)* |
| `HEAV3NS_ARENA_CAPACITY_BYTES` | Override default 16-MiB arena capacity in bytes. | `16777216` |
| `HEAV3NS_AUTHORITY` | Override execution authority (`ReadOnly`, `WorkspaceWrite`, etc.). | `WorkspaceWrite` |

---

## 3. Interactive Slash Commands (REPL Mode)

When running in interactive REPL mode, commands starting with `/` execute deterministic control routines without LLM inference:

| Command | Arguments | Description |
| :--- | :--- | :--- |
| `/help` | *(none)* | Displays available commands and runtime shortcuts. |
| `/status` | *(none)* | Prints current session state, transcript length, context budget, and arena memory usage. |
| `/context` | `[json]` | Displays the visual 20-cell token bar HUD, KV-cache prefix fingerprint, and tool routing. |
| `/prompt` | `[prefix\|compact\|specialist\|side\|json]` | Inspects deterministic static prompt prefix, full prompt, or profile outputs. |
| `/tools` | `[NAME]` | Displays active core vs. on-demand specialist tools, unlocked tools, and token savings. |
| `/compact` | `[RATIO]` | Triggers deterministic transcript compaction (default threshold: 75%). |
| `/clear` | *(none)* | Clears active conversation memory while preserving VFS baseline. |
| `/memory` | `[KEY] [VAL]` | Inspects, sets, or lists entries in the persistent `MemoryStore`. |
| `/vfs` | `[diff\|staged]` | Inspects staged virtual file system modifications. |
| `/commit` | *(none)* | Atomically flushes staged VFS edits to disk with conflict detection. |
| `/skills` | *(none)* | Ingests and lists available `.agents/skills/` capabilities. |
| `/rewind` | `[TICKS]` | Performs $O(1)$ multi-state rewind using `SlabBufferSnapshot`. |
| `/doctor` | *(none)* | Executes architectural self-diagnostics, prompt hygiene, and health checks. |
| `/cua` | *(none)* | Inspects virtual display state and Set-of-Marks layout. |
| `/browser` | *(none)* | Inspects zero-trust browser session and DOM navigation state. |
| `/vision` | `[PATH]` | Inspects image metadata or generates synthetic test SVG/BMP assets. |
| `/forge` | `[CMD]` | Interacts with the evolutionary Skill DAG and Soul Drop Vault. |
| `/soul` | *(none)* | Inspects active `SoulProfile` axioms and operational directives. |
| `/legion` | `[TASK]` | Classifies task into a 7-specialist topological swarm formation. |

---

## 4. Stdio JSON-RPC Protocol Specification (`v: 1`)

When spawned with `--stdio` (and optionally `--desktop-worker`), `heav3ns-micro` acts as a deterministic JSON-RPC 2.0 host communicating over standard I/O. 

### Protocol Invariants
1. **Single-Line Framing**: Every message is serialized as a single JSON object terminated by `\n`.
2. **Version Tagging**: Every outbound frame emitted by `heav3ns-micro` includes `"v": 1` as verified by `ControlFrameReader.swift`.
3. **Stderr Isolation**: Diagnostic logs, traces, and panics are routed exclusively to `stderr` to prevent frame desynchronization on `stdout`.

### Startup & Connection Handshake

Immediately upon launching, the worker outputs a ready event:
```json
{"v": 1, "type": "connection", "state": "ready"}
```

The host client then performs an initialization call to negotiate capabilities:

#### Request (`initialize`)
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "capabilities": [
      "conversations",
      "spider-check-v1",
      "runtime-state-v1",
      "task-continuation-v1",
      "project-repair-v1",
      "checkpoints-v1",
      "prompt-inspect-v1",
      "run-details",
      "queue-edit",
      "queue-remove"
    ]
  }
}
```

#### Response (`initialize`)
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "1.0",
    "capabilities": [
      "conversations",
      "spider-check-v1",
      "runtime-state-v1",
      "task-continuation-v1",
      "project-repair-v1",
      "checkpoints-v1",
      "prompt-inspect-v1",
      "run-details",
      "queue-edit",
      "queue-remove"
    ],
    "workspace": "ws-1731099600",
    "session": "session-default"
  }
}
```

---

## 5. Desktop RPC Method Catalog

### Context & Prompt Telemetry (`context.inspect` & `prompt.inspect`)
Inspects active token budgets, KV-cache prefix fingerprint, dynamic workspace facts, and tool routing breakdowns.
- **Request**: `{"jsonrpc": "2.0", "id": 2, "method": "context.inspect", "params": {}}`
- **Result**: `{"profile": "standard", "modelName": "gpt-5.6-terra", "modelFamily": "openai_reasoning", "cachePrefixHash": "c88ab3c9", "activeTools": 15, "totalTools": 29, "governanceSources": ["SOUL.md"]}`
- **Request**: `{"jsonrpc": "2.0", "id": 3, "method": "prompt.inspect", "params": {"view": "prefix"}}`
- **Result**: `{"view": "prefix", "content": "..."}`

### Snapshot & Synchronization (`snapshot`)
Requests complete snapshot of workspace state, active runs, and transcript history.
- **Request**: `{"jsonrpc": "2.0", "id": 2, "method": "snapshot", "params": {}}`
- **Result**:
  ```json
  {
    "workspace": "ws-1731099600",
    "session": "session-default",
    "runtime": "heav3ns-micro-0.1.0",
    "seq": 4,
    "runs": [
      {
        "id": "run-001",
        "state": "completed",
        "text": "Refactor memory store",
        "createdAt": 1731099600000,
        "activities": []
      }
    ]
  }
  ```

### File Operations (`files.*`)
- **`files.list`**: Returns recursive directory structure with file sizes and modification timestamps.
- **`files.read`**: Reads UTF-8 source code with pagination parameters (`offset`, `limit`).
- **`files.readBinary`**: Reads binary documents (PDF, PNG, CSV) returning Base64 chunks:
  ```json
  {
    "path": "docs/architecture.pdf",
    "encoding": "base64",
    "chunkIndex": 0,
    "totalChunks": 1,
    "data": "JVBERi0xLjQKJeLjz9MKMSAwIG9ia..."
  }
  ```

### Git & Working Tree Diffs (`changes.read`)
Returns unified diff representations of file modifications:
- **Params**: `{"path": "src/core/engine.rs", "staged": false}`
- **Result**: `{"path": "src/core/engine.rs", "diff": "--- a/src/core/engine.rs\n+++ b/src/core/engine.rs\n@@ -10,3 +10,4 @@\n..."}`

### Spider Static Checks (`spider.check`)
Runs local static syntax checks, import resolution, and circular dependency checks:
- **Params**: `{"scope": "all"}`
- **Result**:
  ```json
  {
    "status": "passed",
    "entropyScore": 0.0031,
    "findings": [],
    "files": ["src/lib.rs", "src/core/engine.rs"]
  }
  ```

### Long-Term Memory Store (`memory.*`)
- **`memory.list`**: Returns all persistent key-value facts for the workspace.
- **`memory.save`**: Upserts a key-value memory entry with timestamp.
- **`memory.remove`**: Deletes a memory entry by key.

### Task Execution & Lifecycle (`task.*`)
- **`task.submit`**: Submits a prompt for autonomous agent execution.
- **`task.stop`**: Initiates immediate, safe cooperative cancellation of active work.
- **`task.lookup`**: Queries unconfirmed submission status by UUID to safely prevent replays.

### Swarm Orchestration (`swarm.*`)
- **`swarm.plan`**: Generates a dependency-ordered multi-specialist formation graph.
- **`swarm.next`**: Claims the next actionable specialist assignment.
- **`swarm.report`**: Submits specialist completion evidence and marks downstream tasks ready.
