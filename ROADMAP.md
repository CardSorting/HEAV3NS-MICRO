<!-- [LAYER: UI] -->
# Roadmap

## Completed (`v0.1.0`)
- [x] Distill `LUMI-NEW` into a self-contained Rust runtime with 16-MiB `ArenaAllocator`, transactional `SessionVfs`, self-healing `ToolCallArgParser`, 23 native tools, and `FT-01..FT-05` Flight Test suite.
- [x] Decompose `src/` into the 5-Zone **HEAV3NS-JOY** architecture (`PLUMBING`, `DOMAIN`, `INFRASTRUCTURE`, `CORE`, `UI`) with 100% Stability Index and 100% Build Health (`A+`).
- [x] Install native 8-agent Joyzoning skills, hooks, and MCP configuration (`joy init`).

## Planned (`v0.2.0`)
- [ ] Native MCP stdio server mode (`--mcp`) exposing the 23 built-in tools over JSON-RPC 2.0.
- [ ] Optional tree-sitter symbol indexer adapter in `src/infrastructure/` implementing a `DOMAIN` symbol index trait.

