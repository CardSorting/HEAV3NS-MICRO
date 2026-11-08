<!-- [LAYER: UI] -->
# Playbook PB-01: Architectural Violation Remediation

<p align="center">
  <a href="README.md">Playbooks Hub</a> ·
  <a href="../README.md">Docs Hub</a> ·
  <a href="../adr/0001-five-canonical-zones-and-geographic-alignment.md">ADR-0001</a> ·
  <a href="../CONSTITUTION.md">Constitution</a>
</p>

## Triage at a glance

| Field | Detail |
| :--- | :--- |
| **Playbook ID** | `PB-01` |
| **Severity** | `SEV-2` (Blocks CI / pre-commit gate) |
| **Trigger Codes** | `SPI-201`, `SPI-205`, `SPI-206`, `SPI-208`, `SPI-209` |
| **Time to Mitigate** | `1–5 minutes` |
| **First Command** | `npm run joy-check` |
| **Rollback Safety** | Read-only static analysis audit |

---

## Remediation steps

### Case 1: `SPI-208` — Missing or Misaligned `[LAYER: <ZONE>]` Header
1. **Identify the violating file**: Look at the output of `npm run joy-check`.
2. **Add or align the header**:
   - For Rust files (`.rs`): Add `// [LAYER: <ZONE>]` on line 1.
   - For Markdown files (`.md`): Add `<!-- [LAYER: <ZONE>] -->` on line 1.
3. Ensure the declared zone matches the physical folder (`src/domain/` → `DOMAIN`, `src/core/` → `CORE`, `src/infrastructure/` → `INFRASTRUCTURE`, `src/plumbing/` → `PLUMBING`, `src/ui/` → `UI`).

### Case 2: `SPI-206` — Layer Leakage or Domain I/O Impurity
1. Check if a file in `src/domain/` is importing filesystem, network, or process modules (`std::fs`, `std::net`, `reqwest`, `tokio::fs`).
2. Move the concrete I/O implementation to `src/infrastructure/`.
3. Declare an abstract `pub trait I<Name>` in `src/domain/interfaces/mod.rs` and depend on that trait from `src/core/`.

### Case 3: `SPI-201` — Circular Dependency
1. Identify the cycle chain in the audit report (`A -> B -> A`).
2. Break the cycle by extracting shared data types into `src/domain/types/` or stateless utility functions into `src/plumbing/`.

### Case 4: `SPI-209` — Relative Import Depth Exceeded (`> 3` `../`)
1. In Rust, avoid deep relative `super::super::super::...` paths; prefer crate-relative paths such as `crate::domain::...`.

---

## Verification

```bash
npm run joy-check
```

Confirm that the output reports `Blocking Violations (0): None. 100% Joyzoning Conformance.` with an exit code of `0`.
