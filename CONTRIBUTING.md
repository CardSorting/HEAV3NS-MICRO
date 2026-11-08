<!-- [LAYER: UI] -->
# Contributing to HEAV3NS-MICRO

Thank you for contributing to **HEAV3NS-MICRO**. Every change must preserve **100% Joyzoning Conformance** and pass the full Rust test and Agent Flight Test suites.

## Architectural Rules

1. **Read [`AGENTS.md`](AGENTS.md) and [`SOUL.md`](SOUL.md) first.**
2. **Zone Header Tags**: Every `.rs` file starts with `// [LAYER: <ZONE>]` at line 1; every `.md` file starts with `<!-- [LAYER: <ZONE>] -->` at line 1.
3. **Module Size Cap**: Keep every file in Tier 1 Optimal (`< 800` lines, `SPI-203`). Keep `src/domain/` files focused (`<= 3` structs per file, `0%` I/O).
4. **Contractual Sovereignty**: Any new adapter in `src/infrastructure/` consumed by `src/core/` must implement a formal `I<StructName>` trait declared in `src/domain/interfaces/mod.rs`.

## Verification Before Submitting

```bash
cargo test
cargo run --release -- --flight
cargo run --release -- --smoke
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
```

