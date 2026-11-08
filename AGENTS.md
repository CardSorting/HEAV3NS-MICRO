<!-- [LAYER: UI] -->
# AGENTS.md — Operational Acquisition Protocol for Coding Agents (`HEAV3NS-MICRO`)

This file is the direct operational entrypoint for any autonomous coding agent, pair-programming assistant, or automated workflow encountering **HEAV3NS-MICRO**.

<!-- joy:begin agents -->
## Joyzoning — architecture protocol (HEAV3NS-JOY 1.0.0)

Every source file belongs to exactly one zone, tagged in its first 10,000 characters
(`/** [LAYER: DOMAIN] */`, `# [LAYER: CORE]`, `<!-- [LAYER: UI] -->` …) and matching its folder.

| Zone | Folder | Rule |
| :--- | :--- | :--- |
| `DOMAIN` | `src/domain/` | pure business rules and interfaces (`src/domain/interfaces/I*`); zero I/O; imports nothing from infrastructure or ui |
| `CORE` | `src/core/` | use-case orchestration; depends on domain interfaces, never on ui |
| `INFRASTRUCTURE` | `src/infrastructure/` | concrete I/O adapters implementing domain interfaces; never imports ui |
| `PLUMBING` | `src/plumbing/ or src/utils/` | stateless leaf helpers; imports no other zone; no mutable top-level state |
| `UI` | `src/ui/` | rendering and user intent; reaches infrastructure only through core |

Also: relative imports use at most 3 `../`; no runtime import cycles; core depends on `I<Name>` interfaces in `src/domain/interfaces/`;
files stay under ~800 lines. Build producers (plumbing, domain, infrastructure) before consumers (core, ui).

### Loop (cheap in tokens)
1. Before editing: `npx -y heav3ns-joy@1.0.0 check . --phase pre-edit --scope <file> --neighborhood 1 --format compact`
2. After editing: `npx -y heav3ns-joy@1.0.0 check . --phase post-edit --scope <file> --format compact`
3. Exit `0` = done (`[INFO]` is optional) · `1` = fix each blocking finding · `2` = you mis-invoked the tool (never treat as success).
4. Explain any code: `npx -y heav3ns-joy@1.0.0 check --explain SPI-206`. Fix headers: `npx -y heav3ns-joy@1.0.0 check . --heal --dry-run`, review, then `--heal`.
5. Never weaken config to make a check pass; report the finding instead. Stop after 3 failed attempts on one finding.

### Durable work primitives (no harness integration needed)
- `npx -y heav3ns-joy@1.0.0 trinity selftest` then `npx -y heav3ns-joy@1.0.0 trinity --help`: durable queue (`queue add|list|drain`), buffered tables (`db push|select`), specialist swarm.
- Swarm without touching your harness: `swarm plan --task "<goal>" --id f1` → loop `swarm next f1` / `swarm report f1 --assignment <id> --status completed --summary "<evidence>"` → `swarm finish f1`.
- Output is one JSON document on stdout; exit `0` ok, `1` failure (read `error.message`), `2` usage error (follow the `hint`).
- If MCP tools named `joy_*` are available, prefer them: same operations, typed arguments.

Skills `joyzoning`, `joy-trinity`, `joy-swarm` hold the details; `SOUL.md` holds the working principles. Health check: `npx -y heav3ns-joy@1.0.0 doctor`.
<!-- joy:end agents -->

---

## Phase 1: `HEAV3NS-MICRO` 5-Zone Rust Architecture

Every source file in `HEAV3NS-MICRO` belongs to one of the five canonical Joyzoning zones:

1. **`PLUMBING` (`src/plumbing/`)** — Stateless leaf primitives (`// [LAYER: PLUMBING]`):
   - `hash_and_math.rs`: `now_ms`, `fnv1a_32`, `levenshtein`
   - `text_tokens.rs`: `estimate_text_tokens`, `truncate_text_to_token_budget`, `render_unified_diff`, `canonical_tool_name`
   - `json_repair.rs`: `extract_json_roots`, `repair_json_string`, `accumulate_streaming_tool_name`, `compute_streaming_arguments_delta`, `normalize_completed_tool_call_arguments`
   - `secret_redactor.rs`: `DeterministicSecretRedactor`, `SafetyPolicy`
   - `fuzzy_patch.rs`: `heal_fuzzy_patch`, `DeterministicPatchEngine`, `PatchHunkLine`, `PatchOperation`
2. **`DOMAIN` (`src/domain/`)** — Pure domain entities, state models, and formal `I<Name>` traits (`// [LAYER: DOMAIN]`, **0% I/O**, `<= 3` structs per file):
   - `interfaces/mod.rs`: `ILlmProvider`, `IHttpProvider`, `IScriptedProvider`, `IToolExecutor`, `ISessionPersistence`, `IFileSessionPersistence`, `ISessionVfs`, `ISkillsIngestor`, `ITransactionJournal`
   - `types/`: `messages.rs`, `runtime.rs`, `progress.rs`, `tick_result.rs`, `vfs_types.rs`, `tool_types.rs`, `tool_schedule.rs`, `skill_types.rs`
   - `arena.rs`, `memory.rs`, `compaction.rs`, `swarm.rs`, `tool_parser.rs`
3. **`INFRASTRUCTURE` (`src/infrastructure/`)** — Concrete I/O adapters implementing `src/domain/interfaces/` (`// [LAYER: INFRASTRUCTURE]`):
   - `http_provider.rs`, `scripted_provider.rs`, `vfs_store.rs`, `transaction_journal.rs`, `shell_runner.rs`, `fs_tools.rs`, `search_tools.rs`
4. **`CORE` (`src/core/`)** — Application use-case orchestration (`// [LAYER: CORE]`):
   - `engine.rs`, `slash_and_swarm.rs`, `session_store.rs`, `tool_governor.rs`, `tool_specs.rs`, `tool_registry.rs`, `request_queue.rs`, `flight_runner.rs`, `substrate_tests.rs`
5. **`UI` (`src/ui/`, `src/lib.rs`, `src/main.rs`)** — CLI presenters, stdio JSONL worker, and Composition Roots (`// [LAYER: UI]`):
   - `ui/cli_presenter.rs`, `lib.rs`, `main.rs`

---

## Phase 2: The Five Mandatory Coding Rules

1. **Rule 1 (`[LAYER: <ZONE>]` Header Tag)**: Every `.rs` file starts with `// [LAYER: <ZONE>]` on line 1; every `.md` file starts with `<!-- [LAYER: <ZONE>] -->`.
2. **Rule 2 (Principle of Geographic Alignment)**: A file's physical path (`src/<zone>/...`) must match its `[LAYER: <ZONE>]` header (`SPI-208`).
3. **Rule 3 (Import Depth & Single-Line `use` Discipline)**: Keep `use crate::...;` statements on a single line so polyglot scanners resolve all symbol edges; never introduce circular dependencies (`SPI-201`).
4. **Rule 4 (Contractual Sovereignty `I<ClassName>`)**: Every concrete adapter in `src/infrastructure/` consumed by `src/core/` must have a corresponding `pub trait I<StructName>` in `src/domain/interfaces/mod.rs` (`SPI-107`).
5. **Rule 5 (Module Size & Simplicity Caps)**: Every file must remain in **Tier 1 Optimal (`< 800` lines, `SPI-203`)**; every `src/domain/` file must contain `<= 3` structs.

---

## Phase 3: 8-Point Pre-Commit Verification Checklist

Run these commands before concluding any change:
```bash
# 1. Rust unit & integration test suite (8 suites + 5-scenario Flight Test)
cargo test

# 2. Strict Joyzoning & Spider audit (must exit 0 with 100% Stability Index)
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict

# 3. StabilityDoctor architectural health check
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --doctor
```
