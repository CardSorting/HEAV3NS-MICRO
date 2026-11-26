#!/usr/bin/env node
/** [LAYER: UI] */
/**
 * GALXAI Documentation Sovereignty — Deterministic Project Scaffolder (Zenith Edition).
 *
 * Scaffolds the complete GALXAI documentation topology, templates, specs, workflows,
 * and zero-dependency `bin/docs-check.mjs` + `bin/docs-roll.mjs` into any target project.
 *
 * Usage:
 *   node scaffold-docs.mjs --root <project-dir> [--project <Name>] [--dry-run] [--force] [--json]
 *
 * Safety:
 *   - Non-destructive by default (never overwrites existing files unless `--force`).
 *   - Atomic temp-file + rename writes.
 *   - Runs `docs-roll.mjs` + `docs-check.mjs` on the target immediately after scaffolding (unless `--dry-run`).
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const argv = process.argv.slice(2);
if (argv.includes("--help") || argv.includes("-h")) {
  console.log(`Usage: node scaffold-docs.mjs --root <project-dir> [--project <Name>] [--dry-run] [--force] [--json]`);
  process.exit(0);
}
function flagValue(name) {
  const idx = argv.indexOf(name);
  return idx >= 0 && idx + 1 < argv.length && !argv[idx + 1].startsWith("--") ? argv[idx + 1] : "";
}
const hasFlag = (name) => argv.includes(name);
const dryRun = hasFlag("--dry-run");
const force = hasFlag("--force");
const wantJson = hasFlag("--json");
const targetRoot = path.resolve(flagValue("--root") || process.cwd());
const projectName = flagValue("--project").trim() || path.basename(targetRoot);
const today = new Date().toISOString().slice(0, 10);
const scriptDir = path.dirname(fileURLToPath(import.meta.url));

const files = {
  "README.md": `<!-- [LAYER: UI] -->
# ${projectName}

<div align="center">

\`\`\`text
${projectName.toUpperCase()}
\`\`\`

### Deterministic 5-Zone Sovereign Engineering • Verified Documentation • Zero-Drift CI

[![Joyzoning Sovereign](https://img.shields.io/badge/Architecture-Joyzoning_Sovereign_100%25-blueviolet.svg?style=flat-square)](docs/CONSTITUTION.md)
[![Docs Gate](https://img.shields.io/badge/Docs-GALXAI_Verified-brightgreen.svg?style=flat-square)](docs/DOCS_GUIDE.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)

<p align="center">
  <b>${projectName}</b> is engineered under strict Joyzoning boundaries and the GALXAI deterministic documentation strategy.
</p>

</div>

---

## ⚡ Quick Navigation

- [🚀 30-Second Quickstart](#-30-second-quickstart)
- [🏗️ The 5-Zone Sovereign Architecture](#-the-5-zone-sovereign-architecture)
- [🛡️ System Implementation Boundary](#-system-implementation-boundary)
- [🧭 Documentation Portal & Deep Dives](#-documentation-portal--deep-dives)
- [🤝 Workspace Verification & Standards](#-workspace-verification--standards)

## 🚀 30-Second Quickstart

Run all local checks from the repository root with zero external credentials:

\`\`\`bash
npm run docs:roll:check
npm run docs:check
\`\`\`

> [!NOTE]
> Replace this quickstart with project-specific build, run, and verbatim terminal output blocks as implementation lands.

## 🏗️ The 5-Zone Sovereign Architecture

\`\`\`text
UI -> CORE -> DOMAIN <- INFRASTRUCTURE
                  \\-> PLUMBING
\`\`\`

See [Constitution](docs/CONSTITUTION.md) and [Project Map](docs/PROJECT_MAP.md) for binding zone ownership.

## 🛡️ System Implementation Boundary

| Component | Current Implementation Status | Notes |
|---|---|---|
| **Documentation & CI Gates** | ✅ Active | \`bin/docs-check.mjs\` and \`bin/docs-roll.mjs\` verified |
| **Core Runtime** | 🚧 In Development | Update this table synchronously with implementation |

## 🧭 Documentation Portal & Deep Dives

Explore the [Documentation Portal](docs/README.md) and [Documentation Guide](docs/DOCS_GUIDE.md).

## 🤝 Workspace Verification & Standards

\`\`\`bash
npm run docs:roll
npm run docs:roll:check
npm run docs:check
npm run verify
\`\`\`

Licensed under the **MIT License**. See [LICENSE](LICENSE).
`,

  "AGENTS.md": `<!-- [LAYER: UI] -->
# AGENTS.md — Operational Acquisition Protocol (${projectName})

Every source file belongs to one of five canonical Joyzoning zones: \`DOMAIN\`, \`CORE\`, \`INFRASTRUCTURE\`, \`PLUMBING\`, \`UI\`.

## Mandatory Rules

1. Top-of-file \`[LAYER: <ZONE>]\` header tag matching folder geography.
2. Zero I/O in \`DOMAIN\`; zero upward imports in \`PLUMBING\`; \`CORE\` binds \`INFRASTRUCTURE\` via \`I<Name>\` interfaces.
3. Keep modules under 800 lines and avoid circular dependencies.
4. Update mapped docs in [\`docs/CODE_TO_DOC_MAP.md\`](docs/CODE_TO_DOC_MAP.md), collection indexes, and \`docs/changelog/unreleased.md\` in the same change.
5. Run \`npm run docs:roll:check\`, \`npm run docs:check\`, and \`npm run verify\` before concluding.
`,

  "SOUL.md": `<!-- [LAYER: UI] -->
# SOUL.md — Operating Principles for Coding Agents

1. **Sovereignty first.** Put every file in the zone that owns its responsibility.
2. **Proof before mutation.** Scope checks before editing and verify after.
3. **Smallest sufficient context.** Keep context focused and bounded.
4. **Honest reporting.** Report failures, gaps, and assumptions explicitly.
5. **Reversible by default.** Prefer atomic writes, dry runs, and small diffs.
6. **Rules are load-bearing.** Never weaken or silence checks to pass.
7. **Leave it better and legible.** Update docs and tests with the code.
`,

  "ROADMAP.md": `<!-- [LAYER: UI] -->
# ${projectName} Roadmap

> Autorolled from [\`docs/roadmap/\`](docs/roadmap/README.md) via \`npm run docs:roll\`. Do not edit inside \`AUTOROLL\` markers manually.

<!-- AUTOROLL:START -->
<!-- AUTOROLL:END -->
`,

  "CHANGELOG.md": `<!-- [LAYER: UI] -->
# Changelog

Changes are recorded in [\`docs/changelog/\`](docs/changelog/README.md) and autorolled via \`npm run docs:roll\`.

<!-- AUTOROLL:START -->
<!-- AUTOROLL:END -->
`,

  "VALIDATION_REPORT.md": `<!-- [LAYER: UI] -->
# Validation Report

Record reproducible verification commands, exit codes, and test counts here.
`,

  "CONTRIBUTING.md": `<!-- [LAYER: UI] -->
# Contributing to ${projectName}

Follow [AGENTS.md](AGENTS.md), [docs/CODEBASE_STANDARDS.md](docs/CODEBASE_STANDARDS.md), and [docs/DOCS_GUIDE.md](docs/DOCS_GUIDE.md). Run \`npm run verify\` before opening a pull request.
`,

  "CODE_OF_CONDUCT.md": `<!-- [LAYER: UI] -->
# Code of Conduct

All contributors are expected to maintain a respectful, constructive, and inclusive engineering environment.
`,

  "SECURITY.md": `<!-- [LAYER: UI] -->
# Security Policy

Report security vulnerabilities privately to the maintainers. Do not open public issues for unpatched vulnerabilities. See [docs/SECURITY_BEST_PRACTICES.md](docs/SECURITY_BEST_PRACTICES.md).
`,

  "SUPPORT.md": `<!-- [LAYER: UI] -->
# Support

Consult the [Documentation Portal](docs/README.md) and [Getting Started](docs/GETTING_STARTED.md) before opening an issue.
`,

  "LICENSE": `MIT License\n\nCopyright (c) ${today.slice(0, 4)} ${projectName}\n`,

  "llms.txt": `# ${projectName}\n\n> Authoritative agent index for ${projectName}.\n\n- README.md\n- AGENTS.md\n- SOUL.md\n- docs/README.md\n- docs/DOCS_GUIDE.md\n- docs/CODE_TO_DOC_MAP.md\n`,

  "docs/README.md": `<!-- [LAYER: UI] -->
# ${projectName} Documentation Portal

Authoritative documentation portal for **${projectName}**.

## Quick Query

| Question / Task | Go Directly To |
|---|---|
| Where does a code change require doc updates? | [Code-to-Doc Map](CODE_TO_DOC_MAP.md) |
| How do I build and verify locally? | [Getting Started](GETTING_STARTED.md) · [CI Integration](CI_INTEGRATION.md) |
| What are the architectural rules? | [Constitution](CONSTITUTION.md) · [Codebase Standards](CODEBASE_STANDARDS.md) · [Project Map](PROJECT_MAP.md) |
| How do I operate or recover the system? | [Operations](OPERATIONS.md) · [Playbooks](playbooks/README.md) · [Post-mortems](post-mortems/README.md) |

## Documentation Collections

| Collection | Purpose | Index |
|---|---|---|
| **Knowledgebase** | Stable engineering references | [knowledgebase/README.md](knowledgebase/README.md) |
| **Engineering Wiki** | Conceptual guides & FAQ | [wiki/README.md](wiki/README.md) |
| **ADRs** | Durable architectural decisions | [adr/README.md](adr/README.md) |
| **Playbooks** | Operational recovery SOPs | [playbooks/README.md](playbooks/README.md) |
| **Post-mortems** | Incident reviews & corrective actions | [post-mortems/README.md](post-mortems/README.md) |
| **Roadmap & Changelog** | Autorolled horizons & release history | [roadmap/README.md](roadmap/README.md) · [changelog/README.md](changelog/README.md) |
| **Reports & Strategy** | Verification reports & technical papers | [reports/README.md](reports/README.md) · [strategy/README.md](strategy/README.md) |

## Governance & Reference Index

- [Documentation Guide](DOCS_GUIDE.md) · [Glossary](GLOSSARY.md) · [Maintainer Guide](MAINTAINER.md)
- [Release Process](RELEASING.md) · [Pre-Deployment Checklist](PRE_DEPLOYMENT_CHECKLIST.md) · [Security Best Practices](SECURITY_BEST_PRACTICES.md)
`,

  "docs/DOCS_GUIDE.md": `<!-- [LAYER: UI] -->
# Documentation Guide

This repository follows the GALXAI Diátaxis + operational documentation strategy enforced by \`bin/docs-check.mjs\` and \`bin/docs-roll.mjs\`.

## Principles

1. **Documentation follows implementation.** Link behavioral claims to code, tests, or reproducible commands.
2. **Repository-relative links only.** Never commit \`file://\` or local absolute paths.
3. **Separate evidence from interpretation.** Label simulations, estimates, and observed data explicitly.
4. **100% index completeness.** Every file in a collection is linked from its directory \`README.md\`.
5. **Zero-drift autorolling.** Keep \`ROADMAP.md\` and \`CHANGELOG.md\` synced via \`npm run docs:roll\`.
`,

  "docs/CODE_TO_DOC_MAP.md": `<!-- [LAYER: UI] -->
# Code-to-Documentation Map

| Change surface | Primary documentation to review |
|---|---|
| \`src/domain/**\` | [Constitution](CONSTITUTION.md), [Project Map](PROJECT_MAP.md), [ADRs](adr/README.md) |
| \`src/core/**\` | [Knowledgebase](knowledgebase/README.md), [Project Map](PROJECT_MAP.md) |
| \`src/infrastructure/**\` | [Operations](OPERATIONS.md), [Security Best Practices](SECURITY_BEST_PRACTICES.md) |
| \`bin/**\`, \`.github/workflows/**\` | [CI Integration](CI_INTEGRATION.md), [Maintainer Guide](MAINTAINER.md), [README](../README.md) |
`,

  "docs/PROJECT_MAP.md": `<!-- [LAYER: UI] -->
# ${projectName} Project Map

| Zone | Paths | Responsibility |
|---|---|---|
| \`DOMAIN\` | \`src/domain/**\`, \`spec/**\` | Pure business rules, interfaces, and normative constraints (0% I/O) |
| \`CORE\` | \`src/core/**\`, \`workflows/**\` | Application orchestration and workflows |
| \`INFRASTRUCTURE\` | \`src/infrastructure/**\`, \`.github/workflows/**\` | Concrete I/O adapters and CI manifests |
| \`PLUMBING\` | \`src/plumbing/**\` | Stateless leaf helpers |
| \`UI\` | \`src/ui/**\`, \`docs/**\`, \`bin/**\`, root \`*.md\` | User-facing interfaces, tooling, and documentation |
`,

  "docs/CONSTITUTION.md": `<!-- [LAYER: UI] -->
# Architectural Constitution

1. Every file carries a \`[LAYER: <ZONE>]\` header aligned with its directory.
2. \`DOMAIN\` contains 0% I/O; \`PLUMBING\` imports no higher zone.
3. \`CORE\` consumes \`INFRASTRUCTURE\` strictly through \`I<Name>\` traits/interfaces.
`,

  "docs/CODEBASE_STANDARDS.md": `<!-- [LAYER: UI] -->
# Codebase Standards

Enforce formatting, unit/integration tests, strict Joyzoning checks, and \`npm run docs:check\` on every change.
`,

  "docs/GETTING_STARTED.md": `<!-- [LAYER: UI] -->
# Getting Started

Run \`npm run docs:roll:check\` and \`npm run docs:check\` from the repository root to verify workspace documentation health.
`,

  "docs/MAINTAINER.md": `<!-- [LAYER: UI] -->
# Maintainer Guide

Verify \`npm run verify\`, [Code-to-Doc Map](CODE_TO_DOC_MAP.md), and \`docs/changelog/unreleased.md\` before merging any pull request.
`,

  "docs/GLOSSARY.md": `<!-- [LAYER: UI] -->
# Glossary

- **Joyzoning**: 5-zone architectural partitioning (\`DOMAIN\`, \`CORE\`, \`INFRASTRUCTURE\`, \`PLUMBING\`, \`UI\`).
- **Autorolling**: Deterministic generation of root \`ROADMAP.md\` and \`CHANGELOG.md\` from modular slices in \`docs/\`.
`,

  "docs/OPERATIONS.md": `<!-- [LAYER: UI] -->
# Operations

Consult [Playbooks](playbooks/README.md) for incident remediation and [Post-mortems](post-mortems/README.md) for historical root-cause analyses.
`,

  "docs/SECURITY_BEST_PRACTICES.md": `<!-- [LAYER: UI] -->
# Security Best Practices

Never commit credentials, API keys, or machine-specific local paths. Validate all untrusted inputs at zone boundaries.
`,

  "docs/RELEASING.md": `<!-- [LAYER: UI] -->
# Release Process

1. Run \`node bin/docs-roll.mjs --release <version>\`.
2. Run \`npm run verify\`.
3. Tag and publish the verified release.
`,

  "docs/CI_INTEGRATION.md": `<!-- [LAYER: UI] -->
# CI Integration

\`.github/workflows/ci.yml\` executes \`npm run verify\`, which includes \`npm run docs:check\` and \`npm run docs:roll:check\`.
`,

  "docs/PRE_DEPLOYMENT_CHECKLIST.md": `<!-- [LAYER: UI] -->
# Pre-Deployment Checklist

- [ ] \`npm run verify\` passes with zero errors.
- [ ] \`npm run docs:roll:check\` reports zero drift.
- [ ] Security and rollback playbooks are up to date.
`,

  "docs/adr/README.md": `<!-- [LAYER: UI] -->
# Architectural Decision Records (ADRs)

Use [\`0000-adr-template.md\`](0000-adr-template.md) for every durable architectural decision.
`,

  "docs/adr/0000-adr-template.md": `<!-- [LAYER: UI] -->
# ADR-0000: [Decision title]

- **Status**: Proposed | Accepted | Deprecated
- **Date**: YYYY-MM-DD

## Status

State current status.

## Context

Describe problem, constraints, and evidence.

## Decision

State chosen approach and enforceable invariants.

## Consequences

Describe positive outcomes and trade-offs.

## Verification

List tests, validators, and paths confirming this decision.
`,

  "docs/playbooks/README.md": `<!-- [LAYER: UI] -->
# Operational Playbooks

Add runbooks as \`PB-XXX-<slug>.md\` with \`## Triage at a glance\`, \`## Remediation steps\`, and \`## Verification\`.
`,

  "docs/post-mortems/README.md": `<!-- [LAYER: UI] -->
# Post-Mortems

Record significant incidents using [\`0000-postmortem-template.md\`](0000-postmortem-template.md).
`,

  "docs/post-mortems/0000-postmortem-template.md": `<!-- [LAYER: UI] -->
# PM-000: [Incident title]

- **Date**: YYYY-MM-DD (UTC)
- **Severity**: SEV-1 | SEV-2 | SEV-3 | SEV-4

## Summary

State impact, duration, and current status.

## Root cause

Describe causal mechanism without personal blame.

## Action items

| Action | Owner | Status | Verification |
|---|---|---|---|
| Prevent recurrence | Engineering | Open | Regression test |

## Lessons learned

Capture key takeaways.

## Verification

Link regression tests and operational checks.
`,

  "docs/knowledgebase/README.md": `<!-- [LAYER: UI] -->
# Knowledgebase

Authoritative, implementation-backed engineering references (\`01-*.md\`, \`02-*.md\`, ...).
`,

  "docs/wiki/README.md": `<!-- [LAYER: UI] -->
# Engineering Wiki

Conceptual guides, onboarding walkthroughs, and troubleshooting FAQs.
`,

  "docs/reports/README.md": `<!-- [LAYER: UI] -->
# Verification & Engineering Reports

Active audit, UX, and verification reports.
`,

  "docs/strategy/README.md": `<!-- [LAYER: UI] -->
# Technical Strategy & Papers

Architecture papers, engineering philosophy, stakeholder briefs, and long-term vision.
`,

  "docs/roadmap/README.md": `<!-- [LAYER: UI] -->
# ${projectName} Roadmap Directory

Modular autorolling roadmap horizons projected into [\`ROADMAP.md\`](../../ROADMAP.md).

<!-- AUTOROLL:START -->
<!-- AUTOROLL:END -->
`,

  "docs/roadmap/now.md": `<!-- [LAYER: UI] -->
# Roadmap — Now

### 1. Establish Sovereign Architecture & Documentation CI Gate

- **Status:** Active
- **Goal:** Keep 5-zone Joyzoning compliance, \`docs:check\`, and \`docs:roll:check\` green on every commit.
`,

  "docs/roadmap/next.md": `<!-- [LAYER: UI] -->
# Roadmap — Next

### 1. Expand Automated Acceptance & Regression Coverage

- **Status:** Ready
- **Goal:** Cover critical domain state transitions and failure paths with deterministic tests.
`,

  "docs/roadmap/later.md": `<!-- [LAYER: UI] -->
# Roadmap — Later

### 1. Production Telemetry & Capacity Calibration

- **Status:** Planned
- **Goal:** Reconcile measured latency and throughput against architectural targets.
`,

  "docs/roadmap/discovery.md": `<!-- [LAYER: UI] -->
# Roadmap — Discovery

### 1. Edge-Case Failure Mode Exploration

- **Status:** Investigating
- **Question:** Which boundary conditions or concurrency storms pose the highest reliability risk?
`,

  "docs/roadmap/decision-log.md": `<!-- [LAYER: UI] -->
# Strategic Decision Log

### ${today} — Adopt GALXAI Strict Documentation Sovereignty

- **Decision:** Enforce Diátaxis workspace topology, autorolling \`ROADMAP.md\`/\`CHANGELOG.md\`, and fail-closed \`bin/docs-check.mjs\` in CI.
- **Reason:** Guarantees deterministic, zero-drift documentation across every project.
- **Impact:** All documentation and links are verified automatically on every change.
`,

  "docs/roadmap/archive.md": `<!-- [LAYER: UI] -->
# Roadmap Archive

Completed milestones rolled from active horizons.
`,

  "docs/changelog/README.md": `<!-- [LAYER: UI] -->
# ${projectName} Changelog Directory

Modular autorolling changelog slices projected into [\`CHANGELOG.md\`](../../CHANGELOG.md).

<!-- AUTOROLL:START -->
<!-- AUTOROLL:END -->
`,

  "docs/changelog/unreleased.md": `<!-- [LAYER: UI] -->
# Changelog — Unreleased

### Security & Hardening

### Added

- **GALXAI Documentation Sovereignty Scaffold** ([\`docs-check.mjs\`](../../bin/docs-check.mjs), [\`docs-roll.mjs\`](../../bin/docs-roll.mjs)):
  - Initialized deterministic Diátaxis documentation topology, autorolling roadmap/changelog, and CI verification gates.

### Changed

### Fixed
`,

  "spec/00-CANONICAL-MODEL.md": `<!-- [LAYER: DOMAIN] -->\n# Spec 00: Canonical 5-Zone Model\n\nDefines \`DOMAIN\`, \`CORE\`, \`INFRASTRUCTURE\`, \`PLUMBING\`, and \`UI\`.\n`,
  "spec/01-SYNTAX-AND-HEADERS.md": `<!-- [LAYER: DOMAIN] -->\n# Spec 01: Syntax and Layer Headers\n\nEvery file begins with its zone \`[LAYER: <ZONE>]\` tag.\n`,
  "spec/02-DEPENDENCY-AND-COMPOSITION-RULES.md": `<!-- [LAYER: DOMAIN] -->\n# Spec 02: Dependency and Composition Rules\n\nEnforces unidirectional dependencies and \`I<Name>\` contractual sovereignty.\n`,
  "spec/03-METRICS-AND-FORENSICS.md": `<!-- [LAYER: DOMAIN] -->\n# Spec 03: Metrics and Forensics\n\nDefines module size caps (<800 lines) and stability scoring.\n`,
  "spec/04-ENFORCEMENT-AND-LENIENCY.md": `<!-- [LAYER: DOMAIN] -->\n# Spec 04: Enforcement and Leniency\n\nFail-closed CI verification with zero silent bypasses.\n`,
  "spec/05-PORTABLE-SUBSTRATE-TRINITY.md": `<!-- [LAYER: DOMAIN] -->\n# Spec 05: Portable Substrate Trinity\n\nDefines durable queues, buffered tables, and specialist swarms.\n`,
  "workflows/01-TRIAD-AUDIT-PLANNING.md": `<!-- [LAYER: CORE] -->\n# Workflow 01: Triad Audit Planning\n\nRun Architect, Critic, and SRE checks before mutating code.\n`,
  "workflows/02-TACTICAL-REFACTORING-SOPS.md": `<!-- [LAYER: CORE] -->\n# Workflow 02: Tactical Refactoring SOPs\n\nExecute producer-before-consumer refactoring.\n`,
  "workflows/03-ADOPTING-IN-EXISTING-PROJECTS.md": `<!-- [LAYER: CORE] -->\n# Workflow 03: Adopting in Existing Projects\n\nIncrementally apply layer tags and documentation maps without breaking tests.\n`,
  "workflows/04-EXECUTION-ERGONOMICS-JOYRIDE.md": `<!-- [LAYER: CORE] -->\n# Workflow 04: Execution Ergonomics\n\nKeep verification loops fast, deterministic, and token-bounded.\n`,
  "workflows/05-PORTABLE-SPIDER-AND-BROCCOLIDB.md": `<!-- [LAYER: CORE] -->\n# Workflow 05: Portable Spider and BroccoliDB\n\nOperate durable local primitives without external infrastructure.\n`,
};

// Copy the executable reference scripts from this skill into the target project's bin/
for (const scriptName of ["docs-check.mjs", "docs-roll.mjs"]) {
  const srcPath = path.join(scriptDir, scriptName);
  if (fs.existsSync(srcPath)) {
    files[`bin/${scriptName}`] = fs.readFileSync(srcPath, "utf8");
  }
}

files[".github/workflows/ci.yml"] = `# [LAYER: INFRASTRUCTURE]
name: CI
on:
  push:
    branches: [main]
  pull_request:
permissions:
  contents: read
jobs:
  verify-docs:
    name: GALXAI Documentation & Autoroll Gate
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: "22"
      - name: Run full verification gate
        run: npm run verify
`;

files["bin/docs-check.test.mjs"] = `/** [LAYER: UI] */
import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";

test("docs:roll:check reports zero drift", () => {
  const out = JSON.parse(execFileSync(process.execPath, ["bin/docs-roll.mjs", "--check", "--json"], { encoding: "utf8" }));
  assert.equal(out.ok, true);
  assert.deepEqual(out.invariantErrors, []);
});

test("docs:check passes with zero errors and emits valid SARIF 2.1.0", () => {
  const out = JSON.parse(execFileSync(process.execPath, ["bin/docs-check.mjs", ".", "--json"], { encoding: "utf8" }));
  assert.equal(out.passed, true);
  assert.equal(out.errorCount, 0);
  const sarif = JSON.parse(execFileSync(process.execPath, ["bin/docs-check.mjs", ".", "--sarif"], { encoding: "utf8" }));
  assert.equal(sarif.version, "2.1.0");
  assert.equal(sarif.runs[0].results.length, 0);
});
`;

const created = [];
const skipped = [];

for (const [rel, content] of Object.entries(files)) {
  const full = path.join(targetRoot, rel);
  if (fs.existsSync(full) && !force) {
    skipped.push(rel);
    continue;
  }
  created.push(rel);
  if (!dryRun) {
    fs.mkdirSync(path.dirname(full), { recursive: true });
    const tmp = `${full}.scaftmp-${process.pid}-${Date.now()}`;
    fs.writeFileSync(tmp, content, "utf8");
    fs.renameSync(tmp, full);
  }
}

// Safely create or merge package.json scripts without clobbering existing project scripts
const pkgPath = path.join(targetRoot, "package.json");
const pkgExisted = fs.existsSync(pkgPath);
const pkg = pkgExisted
  ? JSON.parse(fs.readFileSync(pkgPath, "utf8"))
  : { name: projectName.toLowerCase().replace(/[^a-z0-9-]+/g, "-"), version: "0.1.0", private: true, type: "module", scripts: {} };
pkg.scripts = pkg.scripts || {};
const desiredScripts = {
  "docs:check": "node bin/docs-check.mjs .",
  "docs:roll": "node bin/docs-roll.mjs",
  "docs:roll:check": "node bin/docs-roll.mjs --check && node --test bin/docs-check.test.mjs",
};
let pkgChanged = !pkgExisted;
for (const [k, v] of Object.entries(desiredScripts)) {
  if (!pkg.scripts[k]) {
    pkg.scripts[k] = v;
    pkgChanged = true;
  }
}
if (!pkg.scripts.verify) {
  pkg.scripts.verify = "npm run docs:roll:check && npm run docs:check";
  pkgChanged = true;
} else if (!pkg.scripts.verify.includes("docs:check")) {
  pkg.scripts.verify = `${pkg.scripts.verify} && npm run docs:roll:check && npm run docs:check`;
  pkgChanged = true;
}
if (pkgChanged) {
  created.push("package.json");
  if (!dryRun) {
    const tmp = `${pkgPath}.scaftmp-${process.pid}-${Date.now()}`;
    fs.writeFileSync(tmp, `${JSON.stringify(pkg, null, 2)}\n`, "utf8");
    fs.renameSync(tmp, pkgPath);
  }
} else {
  skipped.push("package.json");
}

let rollVerified = false;
let checkVerified = false;
let testVerified = false;
if (!dryRun) {
  execFileSync(process.execPath, [path.join(targetRoot, "bin/docs-roll.mjs"), "--root", targetRoot], { stdio: wantJson ? "pipe" : "inherit" });
  execFileSync(process.execPath, [path.join(targetRoot, "bin/docs-roll.mjs"), "--check", "--root", targetRoot], { stdio: wantJson ? "pipe" : "inherit" });
  rollVerified = true;
  execFileSync(process.execPath, [path.join(targetRoot, "bin/docs-check.mjs"), targetRoot], { stdio: wantJson ? "pipe" : "inherit" });
  checkVerified = true;
  execFileSync(process.execPath, ["--test", "bin/docs-check.test.mjs"], { cwd: targetRoot, stdio: wantJson ? "pipe" : "inherit" });
  testVerified = true;
}

if (wantJson) {
  console.log(JSON.stringify({ ok: true, dryRun, projectName, targetRoot, createdCount: created.length, skippedCount: skipped.length, rollVerified, checkVerified, testVerified, created, skipped }, null, 2));
} else {
  console.log(`scaffold-docs${dryRun ? " (dry-run)" : ""} OK — project='${projectName}', created=${created.length}, skipped=${skipped.length}, rollVerified=${rollVerified}, checkVerified=${checkVerified}, testVerified=${testVerified}`);
}
