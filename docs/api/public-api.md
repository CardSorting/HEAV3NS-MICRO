<!-- [LAYER: UI] -->
# Public Rust API Reference

<p align="center">
  <a href="README.md">API Hub</a> ·
  <a href="cli.md">CLI Reference</a> ·
  <a href="errors.md">Errors</a> ·
  <a href="../architecture/current.md">Architecture</a>
</p>

Comprehensive reference of the public types, contracts, and programmatic interfaces exposed by the `heav3ns-micro` crate.

---

## 1. Domain Interfaces (`src/domain/interfaces/mod.rs`)

All core interactions with infrastructure adapters are decoupled through formal `I<Name>` traits under the Principle of Contractual Sovereignty (`ADR-0007`):

### `ILlmProvider`
```rust
pub trait ILlmProvider: Send + Sync {
    fn query(&self, request: ProviderRequest) -> BoxFuture<'_, Result<ProviderResponse, ProviderError>>;
}
```
Implemented by [`HttpProvider`](../../src/infrastructure/http_provider.rs) and [`ScriptedProvider`](../../src/infrastructure/scripted_provider.rs).

### `ISessionPersistence` & `IFileSessionPersistence`
```rust
pub trait ISessionPersistence: Send + Sync {
    fn save_jsonl(&self, path: &Path, payload: &str) -> Result<bool, String>;
    fn load_jsonl(&self, path: &Path) -> Result<String, String>;
}
```
Implemented by [`FileSessionPersistence`](../../src/infrastructure/vfs_store.rs).

### `ISessionVfs`
```rust
pub trait ISessionVfs: Send + Sync {
    fn export_staged_overlays(&self) -> Vec<VfsFileOverlay>;
    fn commit_staged(&mut self) -> Result<Vec<String>, String>;
}
```
Implemented by [`SessionVfs`](../../src/infrastructure/vfs_store.rs).

### `ITransactionJournal`
```rust
pub trait ITransactionJournal: Send + Sync {
    fn record_file_mutation(&mut self, tool_name: &str, target_path: &Path) -> String;
    fn rollback_last_mutation(&mut self) -> RollbackResult;
}
```
Implemented by [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs).

### `ISkillsIngestor`
```rust
pub trait ISkillsIngestor: Send + Sync {
    fn discover_manifests(workspace_root: &Path) -> Vec<SkillManifest>;
}
```
Implemented by [`SkillsIngestor`](../../src/infrastructure/vfs_store.rs).

---

## 2. Core Engine & Orchestrator (`src/core/engine.rs`)

### `AgentEngine`
The central ReAct state machine orchestrating agent turns, context compaction, tool dispatch, and streaming progress events.

```rust
pub struct AgentEngine {
    // Internal substrate components
}

impl AgentEngine {
    /// Constructs a new engine with default providers, tools, and 16-MiB arena slab.
    pub fn with_defaults(workspace_root: impl Into<PathBuf>) -> Self;

    /// Executes a single multi-step ReAct agent tick with tool execution waves.
    pub async fn tick(&self, input: EngineTickInput) -> EngineTickResult;

    /// Answers a stateless side question without tool invocation or transcript retention.
    pub async fn ask_side_question(&self, question: &str) -> Result<String, ProviderError>;

    /// Handles an interactive slash command (/help, /status, /compact, /commit, etc.).
    pub fn handle_slash_command(&self, input: &str) -> Option<String>;

    /// Orchestrates a topological multi-agent specialist swarm formation.
    pub fn run_swarm(&self, goal: &str) -> SwarmFormationResult;

    /// Returns a list of captured snapshots across transcript, memory, and VFS.
    pub fn snapshots_list(&self) -> Vec<GameStateSnapshot>;

    /// Captures an atomic multi-state snapshot.
    pub fn create_snapshot(&self) -> GameStateSnapshot;

    /// Rewinds transcript, memory, VFS, and Arena slab to a previous snapshot index.
    pub fn rewind_to_snapshot(&self, index: usize) -> Result<GameStateSnapshot, String>;

    /// Introspects or dynamically switches active model configuration.
    pub fn inspect_or_switch_model(&self, arg: &str) -> String;

    /// Accessors for substrate components.
    pub fn workspace_root(&self) -> &Path;
    pub fn tools(&self) -> &Arc<ToolRegistry>;
    pub fn session(&self) -> &Mutex<SessionStore>;
    pub fn vfs(&self) -> &Mutex<SessionVfs>;
    pub fn memory(&self) -> &Arc<Mutex<MemoryStore>>;
    pub fn queue(&self) -> &Arc<RuntimeRequestQueue>;
    pub fn compactor(&self) -> &SessionCompactor;
    pub fn config_snapshot(&self) -> AgentConfig;
}
```

---

## 3. UI & IPC Dispatch (`src/ui/`)

### Stdio Worker Entrypoint (`src/ui/stdio_worker.rs`)
```rust
pub async fn run_stdio_worker(
    workspace_root: PathBuf,
    account: Option<String>,
) -> Result<(), Box<dyn std::error::Error>>;
```
Runs the standard I/O JSON-RPC 2.0 loop, emitting `"v": 1` version-tagged frames to `stdout` and diagnostic logs to `stderr`.

### Desktop RPC Router (`src/ui/rpc_dispatch.rs`)
```rust
pub struct RpcDispatcher;

impl RpcDispatcher {
    pub async fn dispatch(
        engine: &AgentEngine,
        request: RpcRequest,
    ) -> RpcResponse;
}
```
Routes incoming desktop requests to handlers (`files.list`, `files.readBinary`, `changes.read`, `spider.check`, `memory.*`, `task.*`, `swarm.*`, `checkpoint.*`).

---

## 4. Key Substrate Primitives (`src/plumbing/`)

- **`encode_base64(data: &[u8]) -> String`**: RFC 4648 Base64 encoder for binary file inspection.
- **`estimate_text_tokens(text: &str) -> usize`**: Deterministic BPE token estimation ($O(N)$).
- **`truncate_text_to_token_budget(text: &str, budget: usize) -> String`**: Head/tail preservation with explicit truncation markers.
- **`extract_json_roots(text: &str) -> Vec<&str>`**: Balanced brace extraction ignoring braces inside string literals.
- **`repair_json_string(raw: &str) -> String`**: 3-stage heuristic repair (markdown fence stripping, quote healing, unclosed bracket recovery).
- **`DeterministicSecretRedactor::redact_text(input: &str) -> String`**: Replaces API keys, tokens, and passwords with cryptographic redaction badges.
- **`heal_fuzzy_patch(source: &str, patch: &str) -> Result<String, PatchError>`**: Whitespace-tolerant V4A patch applier ($95\%$ tolerance).
