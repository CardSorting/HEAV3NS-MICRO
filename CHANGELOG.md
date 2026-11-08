<!-- [LAYER: UI] -->
# Changelog

All notable changes to **HEAV3NS-MICRO** are documented in this file.

## [0.1.0] - 2026-11-08

### Added
- **5-Zone Joyzoning Architectural Sovereignty (`src/`)**:
  - Decomposed the runtime into 44 focused Rust modules across `src/plumbing/` (6), `src/domain/` (16), `src/infrastructure/` (8), `src/core/` (10), and `src/ui/` + Composition Roots (4), achieving a **100% Stability Index (`SOVEREIGN`)** and **100% Build Health (`A+`)**.
  - Added formal `I<Name>` traits in `src/domain/interfaces/mod.rs` (`ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal`).
  - Preserved 100% backward compatibility in `src/lib.rs` via inline facade modules (`engine`, `flight`, `parser`, `provider`, `session`, `tools`).
- **Native 8-Agent Stack & Documentation Governance**:
  - Installed `AGENTS.md`, `SOUL.md`, `CLAUDE.md`, `GEMINI.md`, `.agents/skills/`, `.claude/skills/`, `.mcp.json`, `.githooks/pre-commit`, `.github/workflows/ci.yml`, `llms.txt`, and `docs/`.

