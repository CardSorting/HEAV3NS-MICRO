<!-- [LAYER: UI] -->
# Runtime & Programmatic API

<p align="center">
  <a href="../README.md">Docs Hub</a> ·
  <a href="public-api.md">Public API</a> ·
  <a href="cli.md">CLI Reference</a> ·
  <a href="errors.md">Error Catalog</a> ·
  <a href="../architecture/current.md">Architecture</a>
</p>

Reference documentation for the **HEAV3NS-MICRO** Rust library exports, public traits, command-line interfaces, and error catalogs.

---

## API Documentation Chapters

| Document | Description | Target Audience |
| :--- | :--- | :--- |
| **[Public API (`public-api.md`)](public-api.md)** | Public Rust structs, traits (`ILlmProvider`, `ISessionVfs`, `ITransactionJournal`), and engine configuration options. | Rust developers & embedding applications |
| **[CLI Reference (`cli.md`)](cli.md)** | Flags, execution modes (`--flight`, `--bench`, `--smoke`, `--stdio`), slash commands, and stdio JSONL wire protocol. | Tool integrators, IDE builders, & operators |
| **[Error Catalog (`errors.md`)](errors.md)** | Enumeration of runtime errors, provider errors, VFS conflict codes, and diagnostic remediations. | Developers & SRE responders |

---

## Quick Example: Using HEAV3NS-MICRO as a Crate

```rust
use heav3ns_micro::core::engine::{MicroAgentEngine, EngineConfig};
use heav3ns_micro::domain::types::runtime::ExecutionAuthorityLevel;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = EngineConfig {
        workspace_root: PathBuf::from("."),
        active_model: "gpt-5.6-terra".to_string(),
        authority: ExecutionAuthorityLevel::WorkspaceWrite,
        ..Default::default()
    };

    let mut engine = MicroAgentEngine::new(config)?;
    let response = engine.step_turn("Inspect src/domain and report public traits").await?;
    println!("Agent response: {}", response);

    Ok(())
}
```
