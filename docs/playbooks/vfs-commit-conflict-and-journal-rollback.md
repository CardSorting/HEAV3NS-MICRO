<!-- [LAYER: UI] -->
# Playbook PB-04: VFS Commit Conflict & Journal Rollback

<p align="center">
  <a href="README.md">Playbooks Hub</a> ·
  <a href="../README.md">Docs Hub</a> ·
  <a href="../adr/0004-two-phase-vfs-staging-and-transaction-journal.md">ADR-0004</a> ·
  <a href="../PORTABLE_TRINITY.md">Substrate Primitives</a>
</p>

## Triage at a glance

| Field | Detail |
| :--- | :--- |
| **Playbook ID** | `PB-04` |
| **Severity** | `SEV-2` (Commit aborted / disk write conflict) |
| **Trigger Codes** | `VfsBaselineMismatch`, `StagingConflict`, `JournalRollbackFailed` |
| **Time to Mitigate** | `2–5 minutes` |
| **First Command** | `cargo run -- -p "/vfs"` |
| **Rollback Safety** | `/rewind` restores prior disk state deterministically |

---

## Remediation steps

### Case 1: `VfsBaselineMismatch` on Commit
1. **Identify the conflicting file**: The error message indicates which file path failed the FNV-1a hash baseline check.
2. **Inspect external changes**:
   - Run `git diff <path>` or inspect the file on disk to see what changed outside the agent session.
3. **Reconcile and re-stage**:
   - Re-read the file into the agent session (`fs_read`).
   - Re-apply the desired modifications onto the updated baseline.
   - Run `/commit` to finalize.

### Case 2: Accidental or Broken Tool Modification
1. If a committed file modification breaks compilation or unit tests:
2. **Execute instant rollback**:
   ```text
   /rewind
   ```
   Or invoke the `session_rollback` tool.
3. The [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs) will apply the inverse diff and restore the file to its previous state.
4. Verify disk restoration via `git status`.

---

## Verification

```bash
cargo test test_vfs_baseline_verification_and_snapshot_rewind
```
