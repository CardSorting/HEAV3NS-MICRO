---
name: galxai-documentation-sovereignty
description: Enforces the GALXAI strict documentation strategy, Diátaxis workspace topology, deterministic README blueprint, autorolling ROADMAP/CHANGELOG governance, and zero-dependency docs-check CI gates for every project created with HEAV3NS-MICRO.
---
<!-- [LAYER: UI] -->

# GALXAI Strict Documentation & Workspace Sovereignty (Zenith Edition)

Every project created or maintained with `HEAV3NS-MICRO` MUST adopt the deterministic documentation strategy and strict engineering culture of **GALXAI**. Documentation is a CI-enforced, zero-drift engineering surface—never hand-waved prose.

---

## 1. Instant Bootstrap (Executable Zero-Dependency Tooling)

Do **not** hand-write documentation scaffolds or validators from scratch. Materialize and run the deterministic scripts bundled with this skill:

```bash
# 0. In a brand-new workspace, materialize bundled scripts via HEAV3NS-MICRO:
#    use_skill(name="galxai-documentation-sovereignty", materialize=true)

# 1. Bootstrap any new or existing project (non-destructive; atomic writes; wires package.json, CI & node:test; self-verifies)
node .agents/skills/galxai-documentation-sovereignty/scripts/scaffold-docs.mjs --root <project-dir> --project "<ProjectName>"

# 2. Preview without mutating disk
node .agents/skills/galxai-documentation-sovereignty/scripts/scaffold-docs.mjs --root <project-dir> --dry-run --json

# 3. Synchronize autorolled ROADMAP.md & CHANGELOG.md (midnight-safe date stability)
node bin/docs-roll.mjs
node bin/docs-roll.mjs --check --json

# 4. Run full fail-closed documentation audit (human, JSON, or SARIF 2.1.0 with exact line numbers)
node bin/docs-check.mjs .
node bin/docs-check.mjs . --json
node bin/docs-check.mjs . --sarif > docs-check.sarif
```

---

## 2. Core Engineering Principles (Non-Negotiable)

1. **Documentation follows implementation.** Every behavioral, quantitative, or security claim links to its owning source file, test, or reproducible command.
2. **Strict Diátaxis + Operational Taxonomy.** Tutorials (`GETTING_STARTED.md`), conceptual guides (`wiki/`), stable references (`knowledgebase/`), normative rules (`spec/`, `CONSTITUTION.md`), durable decisions (`adr/`), recovery runbooks (`playbooks/`), and incident reviews (`post-mortems/`) each have an exclusive home.
3. **Repository-relative links & verified `#anchors`.** Never commit `file:///...`, `/Users/...`, `/home/...`, `C:\...`, or root-escaping `../` links. Both target files AND GitHub-flavored `#anchor` headings (same-file and cross-file) are validated mechanically.
4. **Evidence vs. interpretation separation.** Label synthetic/modeled outputs, estimates, assumptions, and measured production telemetry explicitly. Preserve UTC timestamps, seeds, profiles, commands, and conflicting measurements side by side.
5. **100% index completeness & portal reachability.** Every `.md` file in a maintained collection is linked by filename in its directory `README.md`, and every collection `README.md` is linked from `docs/README.md`.
6. **Zero-drift, midnight-safe autorolling.** Root `ROADMAP.md` and `CHANGELOG.md` are generated views (`<= 160` lines) synced atomically from `docs/roadmap/` and `docs/changelog/`. Existing `Synced YYYY-MM-DD` stamps are preserved on no-op `--check` runs so CI never flakes at UTC midnight.
7. **Zero secret or local-path leakage.** Live credentials (`sk-proj-`, `sk_live_`, `ghp_`, `AKIA`, `AIza`, PEM private keys) and local user paths fail closed in `docs-check.mjs`.

---

## 3. Deterministic Workspace Document Topology

### Root Governance & Entrypoints (`<!-- [LAYER: UI] -->`)
- `README.md` — Authoritative project entrypoint (Section 4 Blueprint)
- `AGENTS.md` — 5-zone mental model, 5 mandatory coding rules, Triad Audit, Producer-Before-Consumer workflow, Agent Self-Check Rubric
- `SOUL.md` — 7 operating principles (Sovereignty first, Proof before mutation, Smallest sufficient context, Honest reporting, Reversible by default, Rules are load-bearing, Leave it better and legible)
- `ROADMAP.md` & `CHANGELOG.md` — Autorolled views (`<= 160` lines, exactly one balanced `<!-- AUTOROLL:START/END -->` pair)
- `VALIDATION_REPORT.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `SUPPORT.md`, `LICENSE`, `llms.txt`

### Authoritative `docs/` Tree (`<!-- [LAYER: UI] -->`)
- `docs/README.md` — Documentation Portal (Quick Query task table, Reading Paths by Role, Core Collections index, Governance index, Active Reports, Local Verification)
- `docs/DOCS_GUIDE.md`, `docs/PROJECT_MAP.md`, `docs/CODE_TO_DOC_MAP.md`, `docs/CONSTITUTION.md`, `docs/CODEBASE_STANDARDS.md`
- `docs/GETTING_STARTED.md`, `docs/MAINTAINER.md`, `docs/GLOSSARY.md`, `docs/OPERATIONS.md`, `docs/SECURITY_BEST_PRACTICES.md`, `docs/RELEASING.md`, `docs/CI_INTEGRATION.md`, `docs/PRE_DEPLOYMENT_CHECKLIST.md`
- `docs/adr/` (`README.md`, `0000-adr-template.md`, `ADR-XXXX-*.md`)
- `docs/playbooks/` (`README.md`, `PB-XXX-*.md`)
- `docs/post-mortems/` (`README.md`, `0000-postmortem-template.md`, `PM-XXX-*.md`)
- `docs/knowledgebase/`, `docs/wiki/`, `docs/roadmap/`, `docs/changelog/`, `docs/reports/`, `docs/strategy/`, `docs/i18n/`

### Normative Specs, Workflows & Tooling
- `spec/` (`<!-- [LAYER: DOMAIN] -->`) — `00-CANONICAL-MODEL.md` .. `05-PORTABLE-SUBSTRATE-TRINITY.md`
- `workflows/` (`<!-- [LAYER: CORE] -->`) — `01-TRIAD-AUDIT-PLANNING.md` .. `05-PORTABLE-SPIDER-AND-BROCCOLIDB.md`
- `bin/docs-check.mjs` & `bin/docs-roll.mjs` (`/** [LAYER: UI] */`)

---

## 4. Deterministic `README.md` Blueprint

Follow this exact high-signal navigation order (matching Stripe / Rust / Diátaxis industry ergonomics):

1. **Line 1 Layer Tag**: `<!-- [LAYER: UI] -->`
2. **Hero (`<div align="center">`)**: ASCII wordmark, single-line capability subtitle, `flat-square` badges (Runtime, `Joyzoning_Sovereign_100%`, verified Test count, Runtime boundary, License, Spec), concise bold summary.
3. **Optional Localization Drawer**: Collapsible `<details>` table linking `docs/i18n/README.<locale>.md` with `<span lang="...">` (and `dir="rtl"` for RTL).
4. **`## ⚡ Quick Navigation`**: Anchor links to every major section (all anchors mechanically verified).
5. **`## 🚀 30-Second Quickstart`**: Zero-credential build/run commands from repo root + verbatim terminal output + `> [!NOTE]` on local vs. production scope.
6. **`## 🧠 The Core Problem`**: Problem statement + `mermaid` architecture flowchart + numbered operational pillars.
7. **`## 🔬 Capabilities / Execution Matrix`**: Structured comparison table (mechanism, best-use, counter-indicators, overhead).
8. **`## 📊 Key Empirical Findings & Evidence`**: Reproducible metrics with seeds, commands, limitations, and `> [!IMPORTANT]` service-gate transparency.
9. **`## 🏗️ The 5-Zone Sovereign Architecture`**: ASCII 5-zone diagram + CI-enforced invariants.
10. **`## 💻 Interactive Terminal Demonstration`**: Verbatim demo and test-suite pass counts.
11. **`## 🛡️ System Implementation Boundary`**: Explicit status table (`✅ Full in-process`, `✅ Dual-mode`, `🔬 Synthetic/Modeled`).
12. **`## 🌐 API / CLI / Interface Surface`**: Structured endpoint/command/RPC table.
13. **`## 🧭 Documentation Portal & Deep Dives`**: ASCII `docs/` tree + links to `docs/DOCS_GUIDE.md` and `docs/README.md`.
14. **`## 🤝 Workspace Verification & Standards`**: Exact `npm run verify`, `docs:roll:check`, `docs:check`, `joy-check` commands + License footer.

---

## 5. Layer Tagging, Template Contracts & Rule Codes

| File Pattern | Required Layer Header |
|---|---|
| `spec/**/*.md` | `<!-- [LAYER: DOMAIN] -->` |
| `workflows/**/*.md` | `<!-- [LAYER: CORE] -->` |
| `docs/**/*.md`, root `*.md` | `<!-- [LAYER: UI] -->` |
| `Cargo.toml`, `.cargo/config.toml`, `pyproject.toml`, `.github/workflows/*.yml`, `docker/**`, `cloudflare/**`, `infra/**` | `# [LAYER: INFRASTRUCTURE]` |
| `.github/**` (non-workflow), `bin/*.sh`, `scripts/*.sh` | `# [LAYER: UI]` (after shebang for `.sh`) |

### Mandatory Template Sections
- **ADRs (`ADR-XXXX-*.md`)**: `## Status`, `## Context`, `## Decision`, `## Consequences`, `## Verification`
- **Playbooks (`PB-XXX-*.md`)**: `## Triage at a glance`, `## Remediation steps`, `## Verification`
- **Post-mortems (`PM-XXX-*.md`)**: `## Summary`, `## Root cause`, `## Action items`, `## Lessons learned`, `## Verification`

### `docs-check.mjs` Diagnostic Rule Codes (SARIF / JSON)
- `DOC-REQ-001`: Missing required workspace document
- `DOC-LAY-001` / `DOC-LAY-002`: Missing or mismatched Markdown `[LAYER: <ZONE>]` tag
- `DOC-CFG-001..003`: Invalid `.docs-check.json` or missing/mismatched config/script `# [LAYER: <ZONE>]` tag
- `DOC-STY-001` / `DOC-STY-002` / `DOC-STY-003` / `DOC-STY-004`: Missing/duplicate H1, unlabeled fenced code block, or unclosed code fence
- `DOC-LLM-001`: Missing local file referenced in `llms.txt`
- `DOC-LNK-001..005`: Forbidden absolute/`file://` link, invalid escape, root-escaping `../`, local path leak, or broken relative link
- `DOC-ANC-001`: Broken same-file or cross-file `#anchor` heading link
- `DOC-SEC-001..005`: Secret token or private key leaked in Markdown prose
- `DOC-TPL-001`: Missing required ADR / Playbook / Post-mortem heading
- `DOC-IDX-001` / `DOC-IDX-002`: Unindexed collection file or unreachable collection from `docs/README.md`
- `DOC-ROL-001` / `DOC-ROL-002`: Unbalanced `AUTOROLL` markers or `> 160` lines on `ROADMAP.md` / `CHANGELOG.md`
- `DOC-RDM-001`: Missing required GALXAI `README.md` blueprint section
- `DOC-CI-001` / `DOC-CI-002`: Missing `docs:check` wiring in `package.json` `scripts.verify` or `.github/workflows/ci.yml`

---

## 6. Mandatory Change Workflow

1. **Map impact**: Consult `docs/CODE_TO_DOC_MAP.md` for every modified source path.
2. **Update synchronously**: Update owning docs, `docs/PROJECT_MAP.md`, directory `README.md` indexes, and `docs/changelog/unreleased.md` in the same change.
3. **Roll & verify**:
   ```bash
   npm run docs:roll
   npm run docs:roll:check
   npm run docs:check
   npm run verify
   ```
4. **Report honestly**: Cite exact exit codes, finding counts, and any skipped gates. Never weaken `.docs-check.json` or `stability.config.json` to force a pass.
