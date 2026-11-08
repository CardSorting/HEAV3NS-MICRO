<!-- [LAYER: UI] -->
# Documentation Guide

<p align="center">
  <a href="README.md">Docs Hub</a> ·
  <a href="adr/README.md">ADRs</a> ·
  <a href="playbooks/README.md">Playbooks</a> ·
  <a href="wiki/README.md">Wiki</a> ·
  <a href="postmortems/README.md">Postmortems</a> ·
  <a href="kb/README.md">Knowledge Base</a> ·
  <a href="MAINTAINER.md">Maintainer Guide</a>
</p>

Complete map of the **HEAV3NS-MICRO** documentation system, mirroring the **HEAV3NS-JOY** documentation style and combining the **Diátaxis framework** (Tutorials, How-To Playbooks, Reference, Explanation/Wiki) with **Institutional Engineering Memory** (MADR Architecture Decision Records, Google SRE Blameless Postmortems, and a 3-Tier Knowledge Base), continuously verified by `npm run docs:check` ([ADR-0009](adr/0009-diataxis-documentation-and-knowledge-governance.md)).

---

## Principles

1. **1-to-1 with code** — Architecture, ADR, Playbook, Postmortem, Wiki, KB, and API pages mirror real paths under `src/`, `bin/`, and `docs/`.
2. **Every markdown file is Joyzoned** — Every `.md` file begins with a valid `<!-- [LAYER: <ZONE>] -->` header tag within the first 10KB and passes `node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict`.
3. **Diátaxis + Institutional Memory separation** — High-level architecture, ADRs, operational playbooks, engineering wiki, blameless postmortems, knowledge base, executive/technical papers, and programmatic/CLI references live in `docs/`.
4. **Automated link, taxonomy, & metric verification** — `npm run docs:check` (`bin/docs-check.mjs`) verifies that all required docs exist, every hub indexes its children, mandatory ADR/Playbook/Postmortem sections are present, every relative link resolves on disk, and all quantitative claims match the live codebase.

---

## Documentation map

### 1. Navigation & Governance (`docs/`)

| Doc | Purpose |
|-----|---------|
| [README.md](README.md) | Central documentation hub, Diátaxis map, audience reading paths, and Mermaid diagram |
| [DOCS_GUIDE.md](DOCS_GUIDE.md) | Documentation principles and complete directory index |
| [AGENT_STACK.md](AGENT_STACK.md) | Three-surface map: Joyzoning spatial rules + Substrate Runtime + Native Agent Execution |
| [PORTABLE_TRINITY.md](PORTABLE_TRINITY.md) | Rust micro runtime primitives: Arena Allocator, Staged VFS Store, and Wave Scheduler |
| [PROJECT_MAP.md](PROJECT_MAP.md) | 1-to-1 directory and module map of the repository |
| [CODE_TO_DOC_MAP.md](CODE_TO_DOC_MAP.md) | Source file → documentation page lookup table |
| [CONSTITUTION.md](CONSTITUTION.md) | The 5 immutable laws of the HEAV3NS-MICRO substrate |
| [CODEBASE_STANDARDS.md](CODEBASE_STANDARDS.md) | Repository layout, zone discipline, and AI-friendly coding rules |
| [MAINTAINER.md](MAINTAINER.md) | Update checklist and `npm run docs:check` CI guardrails |
| [GETTING_STARTED.md](GETTING_STARTED.md) | 10-minute tutorial: audit, cargo build, flight tests, and interactive execution |
| [GLOSSARY.md](GLOSSARY.md) | Definitions of every framework term and code prefix |
| [CI_INTEGRATION.md](CI_INTEGRATION.md) | GitHub Actions, Git hooks, editors, and pre-commit verification |
| [PROPAGATION.md](PROPAGATION.md) | Adoption guide, agent integration, prompt templates, and architectural cheat sheet |
| [OPERATIONS.md](OPERATIONS.md) | Operator runbook: CLI modes, stdio protocol, failure modes, and recovery procedures |
| [SECURITY_BEST_PRACTICES.md](SECURITY_BEST_PRACTICES.md) | Security model, secret redactor, sensitive path interdiction, and deterministic auditing |
| [RELEASING.md](RELEASING.md) | Versioning rules, changelog hygiene, and release procedure |

### 2. Architecture Decision Records (`docs/adr/`)

| Doc | Purpose |
|-----|---------|
| [adr/README.md](adr/README.md) | ADR index, decision dependency graph, and MADR lifecycle rules |
| [adr/0000-adr-template.md](adr/0000-adr-template.md) | Canonical MADR template with mandatory `## Verification` section |
| [adr/0001-five-canonical-zones-and-geographic-alignment.md](adr/0001-five-canonical-zones-and-geographic-alignment.md) | 5 Canonical Zones & Principle of Geographic Alignment (PGA) |
| [adr/0002-arena-allocator-and-o1-tick-rewind.md](adr/0002-arena-allocator-and-o1-tick-rewind.md) | Contiguous 16-MiB memory arena slab and $O(1)$ tick unwinding |
| [adr/0003-self-healing-tool-call-parser.md](adr/0003-self-healing-tool-call-parser.md) | Resilient streaming JSON repair and deduplication engine |
| [adr/0004-two-phase-vfs-staging-and-transaction-journal.md](adr/0004-two-phase-vfs-staging-and-transaction-journal.md) | Disk baseline verification and atomic rollback journaling |
| [adr/0005-dag-wave-scheduler-concurrency.md](adr/0005-dag-wave-scheduler-concurrency.md) | Topological wave scheduling for concurrent read/write tool execution |
| [adr/0006-stdio-jsonl-and-flight-test-harness.md](adr/0006-stdio-jsonl-and-flight-test-harness.md) | Headless stdio JSONL worker protocol and 5-scenario Agent Flight suite |
| [adr/0007-contractual-sovereignty-and-i-interfaces.md](adr/0007-contractual-sovereignty-and-i-interfaces.md) | Strict `I<Name>` Rust traits decoupling core orchestration from infrastructure |
| [adr/0008-secret-redaction-and-security-policy.md](adr/0008-secret-redaction-and-security-policy.md) | Deterministic secret redaction and high-risk file interdiction |
| [adr/0009-diataxis-documentation-and-knowledge-governance.md](adr/0009-diataxis-documentation-and-knowledge-governance.md) | Diátaxis + Institutional Memory Documentation Architecture & Automated Guardrails |

### 3. Operational Playbooks (`docs/playbooks/`)

| Doc | Purpose |
|-----|---------|
| [playbooks/README.md](playbooks/README.md) | Playbook index, severity matrix, and 10-second triage router |
| [playbooks/architectural-violation-remediation.md](playbooks/architectural-violation-remediation.md) | `PB-01`: Remediating `SPI-201..209` and Joyzoning header/layer violations |
| [playbooks/arena-exhaustion-and-oom-triage.md](playbooks/arena-exhaustion-and-oom-triage.md) | `PB-02`: Diagnosing arena exhaustion, slab fragmentation, and tick unwinding |
| [playbooks/flight-test-failures-and-parser-desync.md](playbooks/flight-test-failures-and-parser-desync.md) | `PB-03`: Triage for Flight Test regression (`FT-01..05`) and streaming parser desync |
| [playbooks/vfs-commit-conflict-and-journal-rollback.md](playbooks/vfs-commit-conflict-and-journal-rollback.md) | `PB-04`: Resolving VFS disk baseline mismatch and executing atomic rollbacks |
| [playbooks/secret-leakage-and-security-interdiction.md](playbooks/secret-leakage-and-security-interdiction.md) | `PB-05`: Incident response for credentials, API tokens, and blocked tool calls |

### 4. Engineering Wiki (`docs/wiki/`)

| Doc | Purpose |
|-----|---------|
| [wiki/README.md](wiki/README.md) | Concept index, visual system map, and quick navigation |
| [wiki/mental-models.md](wiki/mental-models.md) | Core mental models: Arena Slab, Two-Phase VFS, Wavefront, and Reality Shield |
| [wiki/rust-substrate-architecture.md](wiki/rust-substrate-architecture.md) | Deep dive into memory layout, arena pointer arithmetic, and zero-allocation parsing |
| [wiki/tool-system-and-execution-wavefront.md](wiki/tool-system-and-execution-wavefront.md) | Wave scheduler, read/write concurrency partition, and 23 native tool specs |
| [wiki/metrics-and-memory-encyclopedia.md](wiki/metrics-and-memory-encyclopedia.md) | Memory budgets, token estimation, structural metrics, and stability indexing |

### 5. Blameless Engineering Postmortems (`docs/postmortems/`)

| Doc | Purpose |
|-----|---------|
| [postmortems/README.md](postmortems/README.md) | Postmortem registry, SRE standards, and closed-loop learning pipeline |
| [postmortems/0000-postmortem-template.md](postmortems/0000-postmortem-template.md) | Canonical blameless postmortem template |
| [postmortems/PM-001-arena-allocator-oom-on-unbounded-streaming-tokens.md](postmortems/PM-001-arena-allocator-oom-on-unbounded-streaming-tokens.md) | `PM-001`: Arena exhaustion during massive token generation → dynamic compaction |
| [postmortems/PM-002-vfs-disk-baseline-mismatch-and-clobbered-writes.md](postmortems/PM-002-vfs-disk-baseline-mismatch-and-clobbered-writes.md) | `PM-002`: Concurrent external file edits causing clobbered writes → disk baseline checks |
| [postmortems/PM-003-streaming-json-parser-deadlock-on-escaped-delimiters.md](postmortems/PM-003-streaming-json-parser-deadlock-on-escaped-delimiters.md) | `PM-003`: Nested escaped quotes causing parser hang → self-healing bracket repair |
| [postmortems/PM-004-unredacted-bearer-tokens-in-tool-call-arguments.md](postmortems/PM-004-unredacted-bearer-tokens-in-tool-call-arguments.md) | `PM-004`: Sensitive API keys leaking to progress events → deterministic redactor |
| [postmortems/PM-005-wave-scheduler-circular-dependency-deadlock.md](postmortems/PM-005-wave-scheduler-circular-dependency-deadlock.md) | `PM-005`: Mutual dependency cycle between staged tool calls → cycle detection and partition |

### 6. Knowledge Base & Knowledge-Based Handling (`docs/kb/`)

| Doc | Purpose |
|-----|---------|
| [kb/README.md](kb/README.md) | Knowledge Base hub and top-10 symptom router |
| [kb/knowledge-handling-architecture.md](kb/knowledge-handling-architecture.md) | Complete 3-Tier Knowledge-Based Handling architecture in HEAV3NS-MICRO |
| [kb/symptom-diagnosis-matrix.md](kb/symptom-diagnosis-matrix.md) | Master Symptom → Root Cause → Fix Command → Playbook/ADR resolution table |
| [kb/faq-and-edge-cases.md](kb/faq-and-edge-cases.md) | Technical FAQ, token budgeting, VFS conflict handling, and custom tool extensions |

### 7. Architecture (`docs/architecture/`)

| Doc | Purpose |
|-----|---------|
| [architecture/current.md](architecture/current.md) | System architecture, 5-zone topology, and execution pipelines |

### 8. Papers (`docs/papers/`)

| Doc | Purpose |
|-----|---------|
| [papers/README.md](papers/README.md) | Reading order and verification commands |
| [papers/companion-brief.md](papers/companion-brief.md) | Executive summary with workspace-verified metrics |
| [papers/philosophy.md](papers/philosophy.md) | *A Philosophy of Sovereign Agency* — Rust micro runtime design values |
| [papers/whitepaper.md](papers/whitepaper.md) | Full technical whitepaper (arena allocation, wave scheduling, self-healing parsing) |

### 9. Runtime & Programmatic API (`docs/api/`)

| Doc | Purpose |
|-----|---------|
| [api/README.md](api/README.md) | Programmatic API and CLI reference index |
| [api/public-api.md](api/public-api.md) | Public Rust API exports, traits, structures, and engine configuration |
| [api/cli.md](api/cli.md) | CLI commands, execution flags, environment variables, and stdio JSONL protocol |
| [api/errors.md](api/errors.md) | Typed error catalog, provider errors, and diagnostic reference |

---

## Verification

```bash
npm run docs:check
npm run joy-check
cargo test
```
