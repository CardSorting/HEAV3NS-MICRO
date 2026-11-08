<!-- [LAYER: UI] -->
# Symptom & Diagnostic Code Resolution Matrix

<p align="center">
  <a href="README.md">KB Hub</a> ·
  <a href="../playbooks/README.md">Playbooks Hub</a> ·
  <a href="../api/errors.md">Error Catalog</a> ·
  <a href="../OPERATIONS.md">Operations</a>
</p>

Master lookup table mapping runtime symptoms, CLI error codes, and audit failures directly to their root causes, one-line fix commands, and governing Playbooks/ADRs.

---

## Master Symptom Resolution Table

| Symptom / Error Code | Root Cause | Immediate One-Line Fix Command | Reference Link |
| :--- | :--- | :--- | :--- |
| `SPI-208: Missing Layer Header` | File missing `// [LAYER: <ZONE>]` on line 1 | Add `// [LAYER: <ZONE>]` to line 1 | [PB-01](../playbooks/architectural-violation-remediation.md) |
| `SPI-206: Domain I/O Impurity` | `src/domain/` module directly imports I/O | Move I/O adapter to `src/infrastructure/` | [ADR-0001](../adr/0001-five-canonical-zones-and-geographic-alignment.md) |
| `SPI-201: Circular Dependency` | Mutual import loop across source modules | Extract shared types to `domain/types/` | [PB-01](../playbooks/architectural-violation-remediation.md) |
| `SPI-205: Mutable State in Plumbing` | Global or module-level mutable variable | Remove mutable state; make functions pure | [CONSTITUTION](../CONSTITUTION.md) |
| `ArenaOom: Out of Memory` | 16-MiB arena capacity exhausted by turns | Run `/compact` to summarize history | [PB-02](../playbooks/arena-exhaustion-and-oom-triage.md) |
| `FT-01 Failure: File Discovery` | File scanner cannot locate test files | Verify workspace root directory path | [ADR-0006](../adr/0006-stdio-jsonl-and-flight-test-harness.md) |
| `FT-03 Failure: Parser Desync` | JSON bracket mismatch or unescaped quote | Check `src/plumbing/json_repair.rs` | [PB-03](../playbooks/flight-test-failures-and-parser-desync.md) |
| `VfsBaselineMismatch` | External disk edit occurred while staging | Re-read target file with `fs_read` | [PB-04](../playbooks/vfs-commit-conflict-and-journal-rollback.md) |
| `SensitivePathBlocked` | Tool attempted to access protected system path | Confine path to `workspace_root` | [PB-05](../playbooks/secret-leakage-and-security-interdiction.md) |
| `AuthorityDenied` | Tool requires higher permission level | Launch engine with `--authority FullSovereign` | [api/cli.md](../api/cli.md) |
| `Broken Markdown Link` | Relative Markdown link does not resolve on disk | Correct link target or run `npm run docs:check` | [ADR-0009](../adr/0009-diataxis-documentation-and-knowledge-governance.md) |
| `Exit Code 1 on Flight Test` | Assertion failure in flight scenario | Run `cargo run -- --flight` for detailed trace | [ADR-0006](../adr/0006-stdio-jsonl-and-flight-test-harness.md) |
