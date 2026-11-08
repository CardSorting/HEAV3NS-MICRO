<!-- [LAYER: UI] -->
# HEAV3NS-MICRO Documentation Hub

Documentation for **HEAV3NS-MICRO** (`v0.1.0`), a hyper-compressed Rust micro agent runtime structured under the **HEAV3NS-JOY** 5-Zone Joyzoning architecture (`PLUMBING`, `DOMAIN`, `INFRASTRUCTURE`, `CORE`, `UI`).

<p align="center">
  <a href="PROJECT_MAP.md">Project Map</a> ·
  <a href="CODE_TO_DOC_MAP.md">Code-to-Doc Map</a> ·
  <a href="CONSTITUTION.md">Constitution</a> ·
  <a href="CODEBASE_STANDARDS.md">Codebase Standards</a> ·
  <a href="architecture/current.md">Architecture (Current)</a> ·
  <a href="../README.md">Repository README</a>
</p>

---

## At a Glance

| Metric | Value | Source |
| :--- | :--- | :--- |
| Crate version | **0.1.0** (`2021` edition) | [`Cargo.toml`](../Cargo.toml) |
| Canonical architectural zones | **5** (`PLUMBING`, `DOMAIN`, `INFRASTRUCTURE`, `CORE`, `UI`) | [`stability.config.json`](../stability.config.json) |
| Rust source modules (`src/`) | **44** (all `< 800` lines Tier 1 Optimal) | [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) |
| Formal `I<Name>` domain interfaces | **9** (`ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal`) | [`src/domain/interfaces/mod.rs`](../src/domain/interfaces/mod.rs) |
| Built-in agent tools | **23** | [`src/core/tool_specs.rs`](../src/core/tool_specs.rs) & [`src/core/tool_registry.rs`](../src/core/tool_registry.rs) |
| Interactive slash commands | **10** (`/help`, `/status`, `/compact`, `/clear`, `/memory`, `/vfs`, `/commit`, `/skills`, `/rewind`, `/doctor`) | [`src/core/slash_and_swarm.rs`](../src/core/slash_and_swarm.rs) |
| Agent Flight Test scenarios | **5** (`FT-01..FT-05`, `19/19` checks passing) | [`src/core/flight_runner.rs`](../src/core/flight_runner.rs) |
| Joyzoning Stability Index ($SI$) | **100% (`SOVEREIGN`)** | [`VALIDATION_REPORT.md`](../VALIDATION_REPORT.md) |
| Joyzoning Build Health | **100% (`Grade: A+ — SOVEREIGN`)** | [`VALIDATION_REPORT.md`](../VALIDATION_REPORT.md) |

---

## Navigation Index

- **[Project Map (`docs/PROJECT_MAP.md`)](PROJECT_MAP.md)**: Complete directory and file-by-file map across all 5 zones.
- **[Code-to-Doc Map (`docs/CODE_TO_DOC_MAP.md`)](CODE_TO_DOC_MAP.md)**: Lookup table mapping every Rust module in `src/` to its governing documentation.
- **[Sovereign Constitution (`docs/CONSTITUTION.md`)](CONSTITUTION.md)**: The 5 immutable architectural laws enforced across `HEAV3NS-MICRO`.
- **[Codebase Standards (`docs/CODEBASE_STANDARDS.md`)](CODEBASE_STANDARDS.md)**: Coding conventions, module size caps, and pre-commit verification gates.
- **[Architecture (Current) (`docs/architecture/current.md`)](architecture/current.md)**: Detailed end-to-end execution flow and module inventory.

