<!-- [LAYER: UI] -->
# HEAV3NS-MICRO Companion Brief

**Executive summary · v0.1.0 · workspace-verified**

_Companion to the [Technical Whitepaper](whitepaper.md). All figures below are measured directly from the repository (`Cargo.toml`, `src/domain/`, `src/core/`, `src/infrastructure/`, `src/plumbing/`, `src/ui/`) and verified by `npm run docs:check`._

---

## One sentence

**HEAV3NS-MICRO** (`v0.1.0`) is a hyper-compressed, high-performance Rust micro agent runtime structured under the **HEAV3NS-JOY** 5-Zone Joyzoning architecture, delivering a 16-MiB contiguous memory arena with $O(1)$ tick pointer rewinding, self-healing streaming tool call parsing, two-phase staged filesystem transactions, and topological DAG wave scheduling.

---

## By the numbers

| Metric | Value | Where |
|--------|-------|-------|
| Crate version | **0.1.0** (`2021` edition) | [`Cargo.toml`](../../Cargo.toml) |
| Canonical architectural zones | **5** | [`stability.config.json`](../../stability.config.json) |
| Rust source modules (`src/`) | **44** (all `< 800` lines Tier 1 Optimal) | [`docs/PROJECT_MAP.md`](../PROJECT_MAP.md) |
| Formal `I<Name>` domain interfaces | **9** traits | [`src/domain/interfaces/mod.rs`](../../src/domain/interfaces/mod.rs) |
| Built-in native agent tools | **23** tools | [`src/core/tool_specs.rs`](../../src/core/tool_specs.rs) |
| Interactive slash commands | **10** commands | [`src/core/slash_and_swarm.rs`](../../src/core/slash_and_swarm.rs) |
| Default memory arena slab | **16 MiB** contiguous buffer | [`src/domain/arena.rs`](../../src/domain/arena.rs) |
| Agent Flight Test scenarios | **5** (`FT-01..FT-05`, `19/19` checks passing) | [`src/core/flight_runner.rs`](../../src/core/flight_runner.rs) |
| Joyzoning Stability Index ($SI$) | **100% (`SOVEREIGN`)** | [`VALIDATION_REPORT.md`](../../VALIDATION_REPORT.md) |
| Joyzoning Build Health | **100% (`Grade: A+ — SOVEREIGN`)** | [`VALIDATION_REPORT.md`](../../VALIDATION_REPORT.md) |
| Maximum relative import traversal | **3** (`../../..`) | `SPI-209` rule |
| Tier 1 Optimal module size cap | **800** lines | `SPI-203` rule |
| Architecture Decision Records (ADRs) | **10** + template | [`docs/adr/README.md`](../adr/README.md) |
| Native macOS AppKit Tests | **308** passed (`0` failed) | [`native/Tests/Heav3nsCodeTests/`](../../native/Tests/Heav3nsCodeTests/) |
| Operational Playbooks & Postmortems | **6** playbooks · **5** postmortems | [`docs/playbooks/README.md`](../playbooks/README.md) · [`docs/postmortems/README.md`](../postmortems/README.md) |
| Concept Wiki & Knowledge Base Articles | **4** wiki guides · **3** KB articles | [`docs/wiki/README.md`](../wiki/README.md) · [`docs/kb/README.md`](../kb/README.md) |

---

## What problem it solves

Autonomous coding agents frequently suffer from runtime bloat, non-deterministic memory consumption, brittle parser crashes on LLM-generated JSON, and dangerous, unverified filesystem mutations. HEAV3NS-MICRO solves these structural problems through four concrete innovations:

| Challenge | HEAV3NS-MICRO Solution |
| :--- | :--- |
| **Heap thrashing & memory leaks** | Pre-allocated 16-MiB memory arena with constant-time $O(1)$ bump-pointer rewinds at turn boundaries ([ADR-0002](../adr/0002-arena-allocator-and-o1-tick-rewind.md)). |
| **Malformed JSON & unclosed markdown fences** | Self-healing streaming parser that reconstructs unbalanced braces, escaped strings, and streaming argument deltas ([ADR-0003](../adr/0003-self-healing-tool-call-parser.md)). |
| **Clobbered writes & unsafe disk mutations** | Two-phase staged VFS with FNV-1a disk baseline verification and atomic rollback journaling ([ADR-0004](../adr/0004-two-phase-vfs-staging-and-transaction-journal.md)). |
| **Tool contention & race conditions** | DAG Wave Scheduler that executes read tools in parallel and mutative tools in serialized stages ([ADR-0005](../adr/0005-dag-wave-scheduler-concurrency.md)). |
| **Credential leakage into event streams** | Zero-allocation deterministic redactor sanitizing API tokens and sensitive file paths ([ADR-0008](../adr/0008-secret-redaction-and-security-policy.md)). |

---

## Architecture at a glance

```text
User / IDE Supervisor (Headless Stdio JSONL / Interactive REPL)
         ↕
UI: cli_presenter.rs · lib.rs · main.rs
         ↕
CORE: engine.rs · flight_runner.rs · tool_governor.rs · slash_and_swarm.rs · tool_specs.rs
         ↕ (via IHttpProvider, IScriptedProvider, ISessionVfs, ITransactionJournal)
INFRASTRUCTURE: http_provider.rs · scripted_provider.rs · vfs_store.rs · transaction_journal.rs
         ↕
DOMAIN: arena.rs · compaction.rs · memory.rs · interfaces/mod.rs · types/* (0% I/O)
         ↕
PLUMBING: json_repair.rs · hash_and_math.rs · text_tokens.rs · secret_redactor.rs · fuzzy_patch.rs (0% State)
```

---

## Verification

```bash
cargo test
cargo run -- --flight
npm run joy-check
npm run docs:check
```
