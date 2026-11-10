<!-- [LAYER: UI] -->
# Codebase Standards & Rules (`HEAV3NS-MICRO`)

This guide defines the engineering standards enforced across **HEAV3NS-MICRO**.

## 1. 5-Zone Workspace Layout

| Directory | Zone | Header Tag | Invariants |
| :--- | :--- | :--- | :--- |
| `src/plumbing/` | `PLUMBING` | `// [LAYER: PLUMBING]` | Stateless leaf helpers; 0 upward imports; `< 800` lines |
| `src/domain/` | `DOMAIN` | `// [LAYER: DOMAIN]` | Pure entities, value objects, & `I<Name>` traits; 0% I/O; `<= 3` structs/file; `< 800` lines |
| `src/infrastructure/` | `INFRASTRUCTURE` | `// [LAYER: INFRASTRUCTURE]` | Concrete HTTP, model pricing registry, VFS, CAS snapshots, filesystem, ripgrep, and shell adapters implementing `domain/interfaces`; 0 `ui` imports; `< 800` lines |
| `src/core/` | `CORE` | `// [LAYER: CORE]` | ReAct engine, turn orchestrator, batch executor, swarm orchestrator, session store, tool registry, request queue, and Flight Test runner; 0 `ui` imports; `< 800` lines |
| `src/ui/`, `src/lib.rs`, `src/main.rs` | `UI` | `// [LAYER: UI]` | CLI parser, stdio JSONL worker, and Composition Roots |

## 2. Rust + Joyzoning Conventions

1. **Line-1 `[LAYER: <ZONE>]` Tags**: Every `.rs` file starts with `// [LAYER: <ZONE>]` on line 1; every `.md` file starts with `<!-- [LAYER: <ZONE>] -->` on line 1.
2. **Single-Line `use` Declarations**: Write `use crate::...;` and `pub use crate::...;` on a single line so polyglot dependency scanners (`sourceScanner.mjs`) resolve 100% of symbol edges.
3. **Role-Specific Symbol Naming (`SPI-106`)**: Avoid exporting identical `pub fn` symbol names across distinct zones.
4. **Formal `I<StructName>` Traits (`SPI-107`)**: Whenever `src/core/` imports a struct `<Name>` from `src/infrastructure/`, define and implement `pub trait I<Name>` in `src/domain/interfaces/mod.rs`.

## 3. Pre-Commit Verification Commands

```bash
cargo test
cargo build --release
./target/release/heav3ns-micro --flight
./target/release/heav3ns-micro --smoke
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --doctor
```

