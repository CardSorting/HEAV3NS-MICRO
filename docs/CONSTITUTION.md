<!-- [LAYER: DOMAIN] -->
# The Sovereign Constitution of HEAV3NS-MICRO

This document defines the seven immutable laws of structural integrity, spatial sovereignty, and emergent computational intelligence for **HEAV3NS-MICRO**, aligned with **[`HEAV3NS — The Sovereign Intelligence Architecture`](../doc.md)** and the **HEAV3NS-JOY** framework.

## I. The Law of Purity (`DOMAIN` & `PLUMBING` Sovereignty)
The **`DOMAIN`** layer (`src/domain/`) is the pure state, behavioral entity, and contract layer; the **`PLUMBING`** layer (`src/plumbing/`) is its stateless algorithmic foundation.
- **Rule**: Direct imports of concrete infrastructure, filesystem/network/process I/O (`std::fs`, `std::net`, `std::process`, `reqwest`, `tokio::fs`), or UI in `src/domain/` are strictly forbidden (`SPI-206`). `src/plumbing/` modules must have zero upward imports and zero mutable module-scope state (`SPI-205`).
- **Enforcement**: Zero-tolerance blocking in `joy-check --theme strict`.

## II. The Law of Contractual Identity (`I<ClassName>` Dependency Inversion)
Modules shall depend upon abstractions, not concretions.
- **Rule**: Every concrete adapter in `src/infrastructure/` (`HttpProvider`, `ScriptedProvider`, `SessionVfs`, `SkillsIngestor`, `FileSessionPersistence`, `TransactionJournal`, `BehavioralKnowledgeStore`) and every core domain subsystem (`SpatialActionMediator`, `ComputerUseOrchestrator`, `VirtualDisplayState`, `UiElement`, `VirtualWindow`) must implement formal `I<Name>` traits declared in [`src/domain/interfaces/mod.rs`](../src/domain/interfaces/mod.rs) and `src/domain/` (`39` traits total).
- **Enforcement**: Verified by `SpiderEngine` (`SPI-107`) and `StabilityDoctor` (`joy-check --doctor`).

## III. The Law of Geographic Alignment & Tier 1 Simplicity
Physical directory geography and module size must remain transparent to humans and coding agents alike.
- **Rule**: Every source file must carry its `[LAYER: <ZONE>]` header at line 1 matching its folder (`SPI-208`). Every file across the `280+` workspace files must remain in **Tier 1 Optimal (`< 800` lines, `SPI-203`)**, and every file in `src/domain/` must define at most `3` structs (`<= 1` struct if `> 500` lines).

## IV. The Law of Double-Wrapped Embodied Agency (`COMPUTER USE` & `SPIDER`)
No pointer or keyboard interaction may be dispatched without spatial grounding and observable postcondition assurance.
- **Rule**: Embodied actions mediated through [`SpatialActionMediator`](../src/domain/spatial_mediator.rs) and [`ComputerUseOrchestrator`](../src/core/computer_use_orchestrator.rs) must pass **Wrapper A** (epoch freshness, target disambiguation, 2D occlusion union check, off-screen auto-scroll, 15% inset safe point) and **Wrapper B** (enabled/role preconditions and post-action focus/value/scroll verification). Live environmental observations (`SPIDER`) always override conflicting historical assumptions (`BROCCOLI`).

## V. The Law of Transactional & Temporal Continuity (`REWIND`)
Workspace, memory, and display mutations must be baseline-verified and reversible.
- **Rule**: Scratch allocations unwind in $O(1)$ via [`ArenaAllocator`](../src/domain/arena.rs); staged VFS overlays ([`SessionVfs`](../src/infrastructure/vfs_store.rs)) verify FNV-1a 32-bit hashes and byte lengths (`VfsDiskBaseline`) against disk before commit; direct file mutations record undo patches in [`TransactionJournal`](../src/infrastructure/transaction_journal.rs); and `VirtualDisplayState` maintains a 32-deep undo/redo snapshot ring.

## VI. The Law of Deterministic Resource Lifecycle (`UI` & `THE HARNESS`)
Long-lived controllers and processes must release resources deterministically.
- **Rule**: Every AppKit window controller, view component, and manager class in `native/` must implement an explicit `dispose()` lifecycle teardown routine (`0` `SPI-603` advisories).

## VII. The Law of Proof-Before-Mutation
No architectural or runtime change is complete without automated empirical verification.
- **Rule**: Every change must pass `cargo test` (`66` suites), `cd native && swift test` (`315` suites), `heav3ns-micro --flight` (`FT-01..FT-05`), `joy-check --theme strict` (`100%` Stability Index, `0` violations, `0` advisories), and `node bin/docs-check.mjs`.
