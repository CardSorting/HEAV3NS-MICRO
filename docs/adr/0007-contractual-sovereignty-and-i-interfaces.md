<!-- [LAYER: UI] -->
# ADR-0007: Contractual Sovereignty via Pure I* Port Interfaces

- **Status**: Accepted
- **Date**: 2026-02-07
- **Deciders**: HEAV3NS Core Architecture Team
- **Technical Story**: [`src/domain/interfaces/mod.rs`](../../src/domain/interfaces/mod.rs) · [`docs/CONSTITUTION.md`](../CONSTITUTION.md)

---

## Status

Accepted — Constitutional Law #2 of HEAV3NS-MICRO.

## Context

In complex software systems, core application logic frequently couples directly to concrete infrastructure implementations (e.g. `reqwest` HTTP clients, concrete filesystem wrappers, or specific database drivers). When infrastructure requirements change or unit testing requires mocking, untangling hardwired dependencies requires massive, high-risk refactoring.

### Decision drivers

- 100% testability with zero network or disk dependencies.
- Strict architectural decoupling between orchestration policy (`src/core/`) and concrete mechanics (`src/infrastructure/`).
- Automated enforcement via static analysis.

### Considered options

1. **Direct Struct Coupling**: Simple initially, but impossible to mock cleanly in unit tests without global state or heavy macros.
2. **Conditional Compilation (`#[cfg(test)]`)**: Pollutes production code with test branches.
3. **Formal `I<Name>` Port Interfaces Declared in `src/domain/interfaces/`**: Use Rust traits to decouple all infrastructure dependencies from core orchestrators.

## Decision

We chose **Option 3: Contractual Sovereignty via Formal `I<Name>` Interfaces** declared in [`src/domain/interfaces/mod.rs`](../../src/domain/interfaces/mod.rs):
- `ILlmProvider`, `IHttpProvider`, `IScriptedProvider`
- `ISessionPersistence`, `IFileSessionPersistence`
- `ISessionVfs`, `ITransactionJournal`
- `ISkillsIngestor`

The `CORE` layer depends exclusively on these abstract traits, instantiated and injected by Composition Roots in `src/lib.rs` and `src/main.rs`.

## Consequences

### Positive

- Flawless unit test mocking (e.g., [`ScriptedProvider`](../../src/infrastructure/scripted_provider.rs)).
- Full compliance with `joy-check` rule `SPI-107` and Constitutional Law #2.

### Negative / trade-offs

- Requires declaring trait definitions in `domain/interfaces/` and implementing them in `infrastructure/`.

## Verification

```bash
cargo check
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
```
