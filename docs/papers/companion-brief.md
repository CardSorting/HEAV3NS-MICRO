<!-- [LAYER: UI] -->
# HEAV3NS-MICRO Companion Brief

**Executive summary · v0.1.0 · workspace-verified**

_Companion to [The Ascendant Computational Intelligence](ascendant-intelligence.md) and the [Technical Whitepaper](whitepaper.md). All figures below are measured directly from the repository (`Cargo.toml`, `src/domain/`, `src/core/`, `src/infrastructure/`, `src/plumbing/`, `src/ui/`, `native/`) and verified by `npm run docs:check`._

---

## One sentence

**HEAV3NS-MICRO** (`v0.1.0`) is the distilled Rust intelligence runtime at the heart of **HEAV3NS — The Sovereign Intelligence Architecture**, uniting a 16-MiB $O(1)$-rewind memory arena, self-healing tool parser, transactional VFS, Double-Wrapped Spatial Action Architecture (`Wrapper A` + `Wrapper B`), 9-stage Closed-Loop Computer Use orchestrator, `SPIDER` structural perception, `BROCCOLI` persistent behavioral learning, and `SYNTHETIC SWARM` collective cognition with a native macOS AppKit IDE (`HEAV3NS-CODE`).

---

## By the numbers

| Metric | Value | Where |
|--------|-------|-------|
| Crate version | **0.1.0** (`2021` edition) | [`Cargo.toml`](../../Cargo.toml) |
| Sovereign Intelligence Pillars | **7** (`HEAV3NS-MICRO`, `SPIDER`, `REWIND`, `BROCCOLI`, `SYNTHETIC SWARM`, `COMPUTER USE`, `THE HARNESS`) | [`doc.md`](../../doc.md) · [`ascendant-intelligence.md`](ascendant-intelligence.md) |
| Canonical architectural zones | **5** (`PLUMBING`, `DOMAIN`, `INFRASTRUCTURE`, `CORE`, `UI`) | [`stability.config.json`](../../stability.config.json) |
| Rust source modules (`src/`) | **82** (`12` Plumbing · `31` Domain · `11` Infra · `20` Core · `8` UI/roots, all `< 800` lines Tier 1 Optimal) | [`docs/PROJECT_MAP.md`](../PROJECT_MAP.md) |
| Total Joyzoning-audited workspace files | **309+** files (`502` local dependency edges) | [`VALIDATION_REPORT.md`](../../VALIDATION_REPORT.md) |
| Formal `I<Name>` domain interfaces | **64** traits (`22` port interfaces + `42` entity contracts) | [`src/domain/interfaces/mod.rs`](../../src/domain/interfaces/mod.rs) |
| Built-in native agent tools | **31** tools (`15` core active + `16` on-demand via `tool_search` 8-slot LRU + `execute_tools` batch) | [`src/core/tool_specs.rs`](../../src/core/tool_specs.rs) |
| Interactive slash commands | **35** commands | [`src/core/slash_and_swarm.rs`](../../src/core/slash_and_swarm.rs) |
| Default memory arena slab | **16 MiB** contiguous buffer with $O(1)$ rewind | [`src/domain/arena.rs`](../../src/domain/arena.rs) |
| Spatial grounding throughput | **$\le 0.40\ \mu\text{s}$** per operation (`> 240,000` ops/sec) | [`src/core/spatial_tests.rs`](../../src/core/spatial_tests.rs) |
| Rust unit & integration tests | **77** passed (`0` failed) across substrate, spatial, computer-use, prompt/shell-guard zenith, and Codewhale zenith suites | `cargo test` |
| Agent Flight Test scenarios | **5** (`FT-01..FT-05`, `19/19` checks passing) | [`src/core/flight_runner.rs`](../../src/core/flight_runner.rs) |
| Native macOS AppKit Tests | **321** passed (`0` failed) | [`native/Tests/Heav3nsCodeTests/`](../../native/Tests/Heav3nsCodeTests/) |
| Joyzoning Stability Index ($SI$) | **100% (`SOVEREIGN`, $E_S = 0.0043$)** | [`VALIDATION_REPORT.md`](../../VALIDATION_REPORT.md) |
| Joyzoning Build Health | **100% (`Grade: A+ — SOVEREIGN`, `0` violations, `0` advisories)** | [`VALIDATION_REPORT.md`](../../VALIDATION_REPORT.md) |
| Architecture Decision Records (ADRs) | **15** + template (`ADR-0001..0015`) | [`docs/adr/README.md`](../adr/README.md) |
| Operational Playbooks & Postmortems | **6** playbooks · **5** postmortems | [`docs/playbooks/README.md`](../playbooks/README.md) · [`docs/postmortems/README.md`](../postmortems/README.md) |
| Concept Wiki & Knowledge Base Articles | **4** wiki guides · **3** KB articles · **5** papers | [`docs/wiki/README.md`](../wiki/README.md) · [`docs/kb/README.md`](../kb/README.md) · [`docs/papers/README.md`](README.md) |

---

## What problem it solves

Autonomous coding and computer-use agents frequently suffer from runtime bloat, ungrounded pointer clicks, infinite failure loops, brittle JSON parser crashes, and unverified filesystem mutations. HEAV3NS-MICRO resolves these challenges across seven integrated pillars:

| Challenge | HEAV3NS-MICRO Solution |
| :--- | :--- |
| **Blind coordinate clicks & occluded UI targets** | **Double-Wrapped Spatial Action Architecture (`COMPUTER USE`)**: Wrapper A enforces stale-epoch fencing, exact 2D sweep-line occlusion union, off-screen auto-scroll, and 15% inset safe points; Wrapper B enforces role permissions and verifies post-action focus/value/scroll deltas ([ADR-0011](../adr/0011-sovereign-intelligence-and-spatial-computer-use.md)). |
| **Repeated UI failures & modal dialog traps** | **9-Stage Closed-Loop Orchestrator + `SPIDER` + `SYNTHETIC SWARM`**: Automatically detects modal traps, intercepts outside clicks with `Escape` dismissal, auto-focuses unfocused inputs before typing, and convenes 5 specialist personas with `ConflictResolver` grounding sovereignty ([ADR-0011](../adr/0011-sovereign-intelligence-and-spatial-computer-use.md)). |
| **Amnesia across interaction turns** | **`BROCCOLI` Durable Behavioral Store**: Calibrates interaction pattern confidence via Laplace-smoothed outcomes, synthesizes multi-step workflow procedures, and syncs proven knowledge into the hybrid TF-IDF `MemoryStore`. |
| **Heap thrashing & irreversible state corruption** | **`REWIND` Temporal Continuity**: 16-MiB arena with $O(1)$ pointer rewind ([ADR-0002](../adr/0002-arena-allocator-and-o1-tick-rewind.md)), pure-Rust SHA-256 CAS snapshots with `WouldBlock` undo safety ([ADR-0014](../adr/0014-codewhale-distillation-posture-cas-and-receipts.md)), two-phase `SessionVfs` with FNV-1a disk baseline checks ([ADR-0004](../adr/0004-two-phase-vfs-staging-and-transaction-journal.md)), `TransactionJournal` rollback, and 32-deep display undo/redo. |
| **Malformed JSON & unclosed markdown fences** | **3-Stage Self-Healing Streaming Parser (`HEAV3NS-MICRO`)** that reconstructs unbalanced braces, escaped strings, and streaming argument deltas ([ADR-0003](../adr/0003-self-healing-tool-call-parser.md)). |
| **Tool contention, shell escapes & credential leakage** | **`THE HARNESS`**: DAG Wave Scheduler executing read waves in parallel and write barriers in sequence across 31 tools ([ADR-0005](../adr/0005-dag-wave-scheduler-concurrency.md), [ADR-0014](../adr/0014-codewhale-distillation-posture-cas-and-receipts.md)), parameterized macOS Seatbelt sandboxing, dual-scope credential protection, and auto-refreshing 2026 model pricing ([ADR-0015](../adr/0015-auto-refreshing-frontier-model-catalog.md)). |

---

## Architecture at a glance

```text
Native macOS AppKit IDE (native/Sources/Heav3nsCode/ · 321 tests) / CLI REPL
         ↕ (stdio JSON-RPC 2.0 · "v": 1 framing)
UI: cli_presenter.rs · stdio_worker.rs · rpc_dispatch.rs · lib.rs · main.rs
         ↕
CORE: engine.rs · computer_use_orchestrator.rs · spatial_engine.rs · tool_registry.rs
      tool_governor.rs · slash_and_swarm.rs · session_store.rs · request_queue.rs · flight_runner.rs
         ↕ (via 39 formal I<Name> traits in src/domain/interfaces/mod.rs & src/domain/*)
INFRASTRUCTURE: http_provider.rs · scripted_provider.rs · vfs_store.rs · behavioral_store.rs
                transaction_journal.rs · fs_tools.rs · search_tools.rs · shell_runner.rs
         ↕
DOMAIN: arena.rs · computer_use.rs · spatial_mediator.rs · spatial_action.rs · skill_tree.rs
        memory.rs · compaction.rs · swarm.rs · tool_parser.rs · interfaces/* · types/* (0% I/O)
         ↕
PLUMBING: spatial_geometry.rs · environmental_diff.rs · display_telemetry.rs · vision_and_dom.rs
          json_repair.rs · fuzzy_patch.rs · secret_redactor.rs · text_tokens.rs · hash_and_math.rs (0% State)
```

---

## Verification

```bash
cargo test
cargo run -- --flight
cd native && swift test && cd ..
npm run joy-check
npm run docs:check
```
