<!-- [LAYER: UI] -->
# Command-Line & Stdio Interface Reference

<p align="center">
  <a href="README.md">API Hub</a> ·
  <a href="public-api.md">Public API</a> ·
  <a href="errors.md">Errors</a> ·
  <a href="../OPERATIONS.md">Operations</a>
</p>

Complete reference for command-line arguments, environment variables, slash commands, and the stdio JSONL worker protocol for **HEAV3NS-MICRO**.

---

## 1. CLI Usage & Flags

```text
USAGE:
  heav3ns-micro [OPTIONS] [PROMPT]
```

### Execution Modes

| Flag | Long Flag | Description |
| :--- | :--- | :--- |
| *(none)* | *(default)* | Starts an interactive pair-programming REPL session. |
| `-p` | `--prompt <TEXT>` | Executes a single agent turn with the given prompt and exits. |
| *(none)* | `--flight` / `--benchmark-flight` | Runs the 5-scenario Agent Flight benchmark suite (`FT-01..FT-05`). |
| *(none)* | `--bench` | Runs substrate micro-benchmarks along with the flight suite. |
| *(none)* | `--smoke` | Executes deterministic runtime self-checks and smoke verification. |
| *(none)* | `--stdio` | Runs the headless newline-delimited JSONL worker protocol. |

### Configuration Options

| Flag | Parameter | Default | Description |
| :--- | :--- | :--- | :--- |
| `-m` | `--model <NAME>` | `gpt-5.6-terra` | Target model name passed to provider requests. |
| *(none)* | `--provider <NAME>` | `openai` | Protocol format: `openai`, `anthropic`, or `gemini`. |
| *(none)* | `--endpoint <URL>` | *(official default)* | Custom API endpoint URL for self-hosted or proxy models. |
| *(none)* | `--cwd <DIR>` | `.` | Sets the active workspace root directory. |
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

## 3. Stdio JSONL Worker Protocol (`--stdio`)

When spawned with `--stdio`, HEAV3NS-MICRO communicates via newline-delimited JSON RPC on standard I/O:

### Turn Request Schema
```json
{
  "jsonrpc": "2.0",
  "method": "turn",
  "params": {
    "prompt": "String",
    "authority": "WorkspaceWrite"
  },
  "id": 1
}
```

### Streaming Progress Event Schema
```json
{
  "jsonrpc": "2.0",
  "method": "progress",
  "params": {
    "step": "executing_tool",
    "tool": "fs_read",
    "arguments": "{\"path\":\"Cargo.toml\"}"
  }
}
```

### Turn Response Schema
```json
{
  "jsonrpc": "2.0",
  "result": {
    "response": "Agent answer...",
    "tokens_used": 256,
    "staged_files": ["src/domain/arena.rs"]
  },
  "id": 1
}
```
