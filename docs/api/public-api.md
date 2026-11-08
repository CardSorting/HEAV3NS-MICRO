<!-- [LAYER: UI] -->
# Public Rust API Reference

<p align="center">
  <a href="README.md">API Hub</a> ·
  <a href="cli.md">CLI Reference</a> ·
  <a href="errors.md">Errors</a> ·
  <a href="../architecture/current.md">Architecture</a>
</p>

Comprehensive reference of the public types, contracts, and interfaces exposed by the `heav3ns-micro` crate.

---

## 1. Domain Interfaces (`src/domain/interfaces/mod.rs`)

All core interactions with infrastructure adapters are decoupled through formal `I<Name>` traits:

### `ILlmProvider`
```rust
pub trait ILlmProvider: Send + Sync {
    fn query(&self, request: ProviderRequest) -> BoxFuture<'_, Result<ProviderResponse, ProviderError>>;
}
```
Implemented by [`HttpProvider`](../../src/infrastructure/http_provider.rs) and [`ScriptedProvider`](../../src/infrastructure/scripted_provider.rs).

### `ISessionPersistence`
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
Implemented by [`SkillsIngestor`](../../src/infrastructure/fs_tools.rs).

---

## 2. Core Engine & Orchestrator (`src/core/engine.rs`)

### `MicroAgentEngine`
The central state machine orchestrating agent turns, memory compaction, tool dispatch, and streaming events.

```rust
pub struct MicroAgentEngine {
    // Orchestration state
}

impl MicroAgentEngine {
    pub fn new(config: EngineConfig) -> Result<Self, EngineInitError>;
    pub async fn step_turn(&mut self, prompt: &str) -> Result<String, EngineTurnError>;
    pub fn session_status(&self) -> SessionStatusReport;
    pub fn compact_memory(&mut self) -> Result<CompactionSummary, CompactionError>;
}
```

### `EngineConfig`
```rust
pub struct EngineConfig {
    pub workspace_root: PathBuf,
    pub active_model: String,
    pub authority: ExecutionAuthorityLevel,
    pub arena_capacity_bytes: usize,
    pub max_turns: usize,
}
```

---

## 3. Substrate Types (`src/domain/types/`)

- **`ExecutionAuthorityLevel`**: `ReadOnly`, `WorkspaceWrite`, `ShellExecute`, `FullSovereign`
- **`VfsFileOverlay`**: Encapsulates staged in-memory file additions, edits, and deletions with baseline hashes.
- **`ToolExecutionRecord`**: Captures duration, stdout, stderr, and exit status for auditing.
- **`RollbackResult`**: Returns status and affected paths after a journal reversal.
