<!-- [LAYER: DOMAIN] -->
# The Sovereign Constitution of HEAV3NS-MICRO

This document defines the five immutable laws of structural integrity, spatial sovereignty, and governed agentic execution for **HEAV3NS-MICRO**, aligned with the **HEAV3NS-JOY** framework.

## I. The Law of Purity (`DOMAIN` & `PLUMBING` Sovereignty)
The **`DOMAIN`** layer (`src/domain/`) is the pure state and contract layer; the **`PLUMBING`** layer (`src/plumbing/`) is its stateless foundation.
- **Rule**: Direct imports of concrete infrastructure, filesystem/network/process I/O (`std::fs`, `std::net`, `std::process`, `reqwest`, `tokio::fs`), or UI in `src/domain/` are strictly forbidden (`SPI-206`). `src/plumbing/` modules must have zero upward imports and zero mutable module-scope state (`SPI-205`).
- **Enforcement**: Zero-tolerance blocking in `joy-check --theme strict`.

## II. The Law of Contractual Identity (`I<ClassName>` Dependency Inversion)
Modules shall depend upon abstractions, not concretions.
- **Rule**: Every concrete adapter in `src/infrastructure/` (`HttpProvider`, `ScriptedProvider`, `SessionVfs`, `SkillsIngestor`, `FileSessionPersistence`, `TransactionJournal`) consumed by `src/core/` must implement a formal `I<StructName>` trait declared in [`src/domain/interfaces/mod.rs`](../src/domain/interfaces/mod.rs).
- **Enforcement**: Verified by `SpiderEngine` (`SPI-107`) and `StabilityDoctor` (`joy-check --doctor`).

## III. The Law of Geographic Alignment & Tier 1 Simplicity
Physical directory geography and module size must remain transparent to humans and coding agents alike.
- **Rule**: Every source file must carry its `[LAYER: <ZONE>]` header at line 1 matching its folder (`SPI-208`). Every file must remain in **Tier 1 Optimal (`< 800` lines, `SPI-203`)**, and every file in `src/domain/` must define at most `3` structs (`SPI-203`).

## IV. The Law of Transactional & Cryptographic Truth
Workspace mutations must be baseline-verified and reversible.
- **Rule**: Staged VFS overlays ([`SessionVfs`](../src/infrastructure/vfs_store.rs)) verify FNV-1a 32-bit hashes and byte lengths (`VfsDiskBaseline`) against disk before commit, and every direct file mutation is recorded in [`TransactionJournal`](../src/infrastructure/transaction_journal.rs) for deterministic rollback.

## V. The Law of Proof-Before-Mutation
No architectural or runtime change is complete without automated empirical verification.
- **Rule**: Every change must pass `cargo test`, `heav3ns-micro --flight` (`FT-01..FT-05`), `heav3ns-micro --smoke`, and `joy-check --theme strict` (`0` violations, `100%` Stability Index).

