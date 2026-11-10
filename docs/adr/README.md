<!-- [LAYER: UI] -->
# Architecture Decision Records (ADRs)

<p align="center">
  <a href="../README.md">Docs Hub</a> ·
  <a href="../../doc.md">Foundational Charter (doc.md)</a> ·
  <a href="../wiki/README.md">Wiki</a> ·
  <a href="../playbooks/README.md">Playbooks</a> ·
  <a href="../postmortems/README.md">Postmortems</a> ·
  <a href="../kb/README.md">Knowledge Base</a> ·
  <a href="../architecture/current.md">Architecture</a> ·
  <a href="../CONSTITUTION.md">Constitution</a>
</p>

An **Architecture Decision Record (ADR)** captures a significant architectural choice in **HEAV3NS-MICRO**, its context, the alternatives evaluated, its consequences, and the **executable verification commands** (`cargo test`, `swift test`, `joy-check`, or `docs-check`) that keep the decision continuously enforced in the codebase.

Our ADR format follows **MADR (Markdown Any Decision Records)**, extended with an explicit `## Verification` section so no ADR ever degrades into unverified prose.

---

## Decision dependency graph

```mermaid
flowchart TD
  ADR1["ADR-0001: 5 Canonical Zones & PGA"]
  ADR2["ADR-0002: Arena Allocator & O(1) Rewind"]
  ADR3["ADR-0003: Self-Healing Tool Call Parser"]
  ADR4["ADR-0004: Two-Phase VFS & Transaction Journal"]
  ADR5["ADR-0005: DAG Wave Scheduler Concurrency"]
  ADR6["ADR-0006: Stdio JSONL & Flight Benchmark"]
  ADR7["ADR-0007: Contractual Sovereignty & I* Traits"]
  ADR8["ADR-0008: Secret Redactor & Security Policy"]
  ADR9["ADR-0009: Diátaxis Docs & Automated Guardrails"]
  ADR10["ADR-0010: Native Desktop AppKit Integration & RPC"]
  ADR11["ADR-0011: Sovereign Intelligence, Spatial Mediation & Closed-Loop CUA"]
  ADR12["ADR-0012: Lean System Prompt, KV-Cache Prefixing & Sticky Tool Routing"]
  ADR13["ADR-0013: Shell Command Guard & Pipeline Anti-Starvation Defenses"]

  ADR1 --> ADR7
  ADR1 --> ADR2
  ADR2 --> ADR4
  ADR3 --> ADR5
  ADR4 --> ADR5
  ADR4 --> ADR6
  ADR7 --> ADR6
  ADR8 --> ADR6
  ADR1 --> ADR9
  ADR6 --> ADR10
  ADR1 --> ADR10
  ADR5 --> ADR11
  ADR7 --> ADR11
  ADR8 --> ADR11
  ADR3 --> ADR12
  ADR7 --> ADR12
  ADR8 --> ADR13
  ADR5 --> ADR13
```

---

## Canonical ADR index

| ADR | Title | Status | Surface | Enforced By |
| :--- | :--- | :---: | :--- | :--- |
| [Template](0000-adr-template.md) | Canonical MADR Template for HEAV3NS-MICRO | Template | Governance | [`bin/docs-check.mjs`](../../bin/docs-check.mjs) |
| [ADR-0001](0001-five-canonical-zones-and-geographic-alignment.md) | Five Canonical Zones & Principle of Geographic Alignment (PGA) | **Accepted** | Spatial Architecture | `joy-check --theme strict` |
| [ADR-0002](0002-arena-allocator-and-o1-tick-rewind.md) | Contiguous 16-MiB Arena Slab & $O(1)$ Tick Watermark Rewind | **Accepted** | Substrate Memory (`REWIND`) | `test_arena_allocator_alignment_and_o1_rewind` |
| [ADR-0003](0003-self-healing-tool-call-parser.md) | Streaming Self-Healing JSON Repair & Deduplication Engine | **Accepted** | Substrate Parsing (`HEAV3NS-MICRO`) | `test_self_healing_parser_and_streaming_dedup` |
| [ADR-0004](0004-two-phase-vfs-staging-and-transaction-journal.md) | Two-Phase Staged VFS with Disk Baseline & Rollback Journal | **Accepted** | Filesystem Safety (`REWIND`) | `test_vfs_baseline_verification_and_snapshot_rewind` |
| [ADR-0005](0005-dag-wave-scheduler-concurrency.md) | Topological Wave Scheduling for Concurrent Read/Write Tool Waves | **Accepted** | Tool Execution (`THE HARNESS`) | `test_wave_scheduler_partitioning` |
| [ADR-0006](0006-stdio-jsonl-and-flight-test-harness.md) | Headless Stdio JSONL Worker Protocol & 5-Scenario Flight Test | **Accepted** | Agent Execution (`THE HARNESS`) | `test_agent_flight_suite_passes_100_percent` |
| [ADR-0007](0007-contractual-sovereignty-and-i-interfaces.md) | Contractual Sovereignty via Pure `I<Name>` Port Interfaces | **Accepted** | Core Architecture (`39` traits) | `src/domain/interfaces/mod.rs` |
| [ADR-0008](0008-secret-redaction-and-security-policy.md) | Deterministic Secret Redaction & Sensitive Path Interdiction | **Accepted** | Runtime Security (`THE HARNESS`) | `test_secret_redactor_and_sensitive_path_guard` |
| [ADR-0009](0009-diataxis-documentation-and-knowledge-governance.md) | Diátaxis + Institutional Memory Documentation Architecture | **Accepted** | Documentation | `node bin/docs-check.mjs` |
| [ADR-0010](0010-native-desktop-integration-and-rpc-protocol.md) | Native Desktop AppKit Integration, Modular UI Decomposition, and Stdio JSON-RPC Protocol | **Accepted** | Desktop IDE & IPC | `swift test` & `cargo test` |
| [ADR-0011](0011-sovereign-intelligence-and-spatial-computer-use.md) | Sovereign Intelligence Architecture, Double-Wrapped Spatial Mediation & Closed-Loop Computer Use | **Accepted** | `SPIDER` · `BROCCOLI` · `SWARM` · `COMPUTER USE` | `cargo test core::spatial_tests` & `computer_use_tests` |
| [ADR-0012](0012-lean-system-prompt-and-kv-cache-prefix-determinism.md) | Lean System Prompt Specification, KV-Cache Prefix Determinism & Sticky Dynamic Tool Routing | **Accepted** | Prompt Architecture (`HEAV3NS-MICRO`) | `test_prompt_cache_prefix_determinism_and_model_families` |
| [ADR-0013](0013-shell-command-defenses-and-pipeline-sanitization.md) | Shell Command Guard, Heredoc File-Write Interdiction & Terminal File-Read Anti-Starvation | **Accepted** | Runtime Defenses (`THE HARNESS`) | `test_adr_0015_compound_multiline_and_closing_heredoc_defenses` |

---

## ADR lifecycle & governance rules

1. **Immutability after acceptance**: Once an ADR is marked **Accepted**, its core decision is historical record. If requirements evolve, create a new ADR superseding the previous one.
2. **Required sections**: Every numbered ADR must include:
   - `## Status`
   - `## Context`
   - `## Decision`
   - `## Consequences`
   - `## Verification`
3. **Automated guardrail**: `npm run docs:check` verifies that every ADR in `docs/adr/` starts with `<!-- [LAYER: UI] -->`, contains all required sections, is indexed in `docs/adr/README.md`, and has zero broken links.
