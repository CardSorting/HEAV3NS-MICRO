---
name: joyzoning
description: Keep code architecturally sovereign: place files in the right Joyzoning zone, run scoped pre/post-edit checks, and fix violations. Use whenever creating, moving, or editing source files, or when asked about architecture, layering, or imports.
---
<!-- [LAYER: UI] -->

# Joyzoning

Five zones: DOMAIN `src/domain`, CORE `src/core`, INFRASTRUCTURE `src/infrastructure`, PLUMBING `src/plumbing`, UI `src/ui`.
Each file carries one `[LAYER: <ZONE>]` header that matches its folder.

## Workflow
1. **Scope** before editing: `npx -y heav3ns-joy@1.0.0 check . --phase pre-edit --scope <file> --neighborhood 1 --format compact`
2. **Read cheaply**: `npx -y heav3ns-joy@1.0.0 check . --skeleton <file> --keep-symbol <name>` prints signatures only.
3. **Edit** while obeying the invariants below.
4. **Verify**: `npx -y heav3ns-joy@1.0.0 check . --phase post-edit --scope <file> --format compact`; fix every `error` line, then re-run.
5. **Explain** unknown codes: `npx -y heav3ns-joy@1.0.0 check --explain SPI-206` (rationale plus before/after fix).

## Invariants
- DOMAIN: no I/O (`fs`, `http`, `net`, `child_process`, `fetch`, DB drivers); never imports infrastructure or ui.
- PLUMBING: stateless (no mutable top-level `let`/`var`); imports no other zone.
- UI never imports infrastructure; CORE/DOMAIN/INFRASTRUCTURE never import ui.
- Relative imports: at most three `../`. No runtime cycles. Core talks to infrastructure through `I<Name>` in `src/domain/interfaces/`.
- Implement producers (plumbing → domain → infrastructure) before consumers (core → ui).

## Exit codes
`0` pass · `1` blocking violations (fix them) · `2` bad invocation (fix the command). After 3 failed attempts on one finding, stop and report it.
Never weaken `stability.config.json` to get a pass.
