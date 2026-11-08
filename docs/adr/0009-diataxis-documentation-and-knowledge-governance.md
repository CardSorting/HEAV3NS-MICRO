<!-- [LAYER: UI] -->
# ADR-0009: Diátaxis + Institutional Memory Documentation Architecture

- **Status**: Accepted
- **Date**: 2026-02-09
- **Deciders**: HEAV3NS Core Architecture Team
- **Technical Story**: [`docs/DOCS_GUIDE.md`](../DOCS_GUIDE.md) · [`bin/docs-check.mjs`](../../bin/docs-check.mjs)

---

## Status

Accepted.

## Context

Technical documentation in high-velocity software projects often becomes outdated, fragmented, or detached from codebase reality. When autonomous coding agents interact with documentation, broken links, conflicting instructions, and unverified architectural claims cause hallucinations and incorrect refactoring decisions.

### Decision drivers

- 1-to-1 parity between documentation and source code.
- Clear structural division between tutorials, operational playbooks, reference documentation, and concept explanations.
- Continuous automated verification of documentation links and metrics in CI.

### Considered options

1. **Monolithic README.md**: Overwhelms readers and degrades into an unstructured wall of text.
2. **External Wiki / Notion**: Detached from git version control; changes cannot be reviewed atomically in pull requests.
3. **Diátaxis Framework + Institutional Memory (MADR, SRE Postmortems, Knowledge Base) with Automated Guardrails**: A 4-quadrant Diátaxis layout coupled with ADRs, playbooks, and blameless postmortems, continuously verified by a zero-dependency script.

## Decision

We chose **Option 3: Diátaxis + Institutional Memory Architecture**:
- Structure documentation across `docs/adr/`, `docs/playbooks/`, `docs/wiki/`, `docs/postmortems/`, and `docs/kb/`.
- Every markdown document carries a valid `<!-- [LAYER: UI] -->` header tag.
- Implement an automated verifier (`bin/docs-check.mjs`) ensuring zero broken links, complete taxonomy hub indexing, and live metric parity with the codebase.

## Consequences

### Positive

- Complete, self-healing documentation that never drifts from code reality.
- Instant automated feedback on broken links or missing sections during CI runs.

### Negative / trade-offs

- Document authoring requires strict adherence to taxonomy templates and cross-linking rules.

## Verification

```bash
node bin/docs-check.mjs
```
