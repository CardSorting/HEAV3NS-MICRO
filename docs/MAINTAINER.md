<!-- [LAYER: UI] -->
# Maintainer Guide

<p align="center">
  <a href="README.md">Docs Hub</a> ·
  <a href="DOCS_GUIDE.md">Docs Guide</a> ·
  <a href="CI_INTEGRATION.md">CI Integration</a> ·
  <a href="RELEASING.md">Releasing</a> ·
  <a href="CODEBASE_STANDARDS.md">Standards</a>
</p>

Guidelines, automated guardrails, and quality checklists for maintainers of **HEAV3NS-MICRO**.

---

## 1. Quality Gates & Enforcement Commands

Before merging any pull request or cutting a release, maintainers must verify that all five local verification gates exit with code `0`:

```bash
# Gate 1: Rust compilation & test suite (77 tests)
cargo test

# Gate 2: 5-scenario Agent Flight suite (19 checks)
cargo run -- --flight

# Gate 3: Native macOS AppKit suite (321 tests)
cd native && swift test && cd ..

# Gate 4: Joyzoning spatial sovereignty audit (100% SI, Grade A+)
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict

# Gate 5: Diátaxis documentation guardrails (zero broken links)
node bin/docs-check.mjs
```

---

## 2. Maintaining Documentation Integrity

HEAV3NS-MICRO strictly enforces Diátaxis documentation standards and institutional memory practices ([ADR-0009](adr/0009-diataxis-documentation-and-knowledge-governance.md)):

### Adding a New Source Module
When adding a new file under `src/`:
1. Place it in the appropriate zone directory:
   - `src/plumbing/`: Stateless leaf helpers, 0% I/O, no dependencies.
   - `src/domain/`: Pure domain types and traits, 0% I/O.
   - `src/infrastructure/`: Concrete I/O implementations.
   - `src/core/`: Application orchestration logic.
   - `src/ui/`: CLI and user presentation.
2. Put `// [LAYER: <ZONE>]` on line 1.
3. Keep the file under 800 lines (`SPI-203`).
4. Update [`docs/PROJECT_MAP.md`](PROJECT_MAP.md) and [`docs/CODE_TO_DOC_MAP.md`](CODE_TO_DOC_MAP.md).

### Recording Significant Architectural Decisions
When altering runtime semantics, adding a new tool category, or modifying memory layout:
1. Create a new ADR in `docs/adr/` using [`docs/adr/0000-adr-template.md`](adr/0000-adr-template.md).
2. Ensure mandatory sections are present (`## Status`, `## Context`, `## Decision`, `## Consequences`, `## Verification`).
3. Add the ADR to [`docs/adr/README.md`](adr/README.md) and update the Mermaid decision graph.

### Responding to Runtime Incidents
When an operational issue, crash, or edge case is resolved:
1. File a blameless postmortem in `docs/postmortems/` using [`docs/postmortems/0000-postmortem-template.md`](postmortems/0000-postmortem-template.md).
2. Add a regression test in `src/core/substrate_tests.rs`.
3. Add or update the corresponding symptom entry in [`docs/kb/symptom-diagnosis-matrix.md`](kb/symptom-diagnosis-matrix.md).

---

## 3. Automated Guardrail Checks

The `node bin/docs-check.mjs` script validates:
- **Required Documents**: All 52 essential documentation files are present on disk.
- **Header Tags**: 100% of `.md` files contain valid `<!-- [LAYER: <ZONE>] -->` tags.
- **Hub Indexing**: Every directory hub (`adr/`, `playbooks/`, `wiki/`, `postmortems/`, `kb/`) lists its child documents.
- **Link Resolution**: All relative Markdown links resolve to valid disk files or headings.
- **Metrics Parity**: Quantitative claims in `docs/README.md` and `docs/papers/companion-brief.md` match codebase realities.

---

## 4. Pre-Release Checklist

- [ ] All 8 unit tests in `cargo test` pass.
- [ ] Agent Flight test (`--flight`) completes with 19/19 passing checks.
- [ ] Joyzoning Stability Index is 100% (`SOVEREIGN`) with 0 blocking violations.
- [ ] Documentation guardrail `npm run docs:check` reports OK with 0 broken links.
- [ ] Version numbers in `Cargo.toml`, `package.json`, and documentation hubs match.
- [ ] `CHANGELOG.md` reflects all recent architectural improvements.
