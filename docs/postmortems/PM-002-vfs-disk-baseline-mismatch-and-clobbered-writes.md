<!-- [LAYER: UI] -->
# PM-002: VFS Disk Baseline Mismatch & Clobbered External Writes

- **Incident Date**: 2026-02-12
- **Severity**: SEV-1
- **Subsystem**: `SessionVfs` (`src/infrastructure/vfs_store.rs`)
- **Related ADR**: [ADR-0004](../adr/0004-two-phase-vfs-staging-and-transaction-journal.md)
- **Resolved By**: FNV-1a disk baseline verification before commit

---

## Summary

A developer concurrently edited a critical domain interface file in their editor while an agent session was generating a multi-file refactoring patch. When the agent committed its staged overlay, it clobbered the human's newly saved edits.

---

## Root cause

1. **Why was human work clobbered?** The agent's `commit_staged` operation blindly overwrote the target file on disk.
2. **Why didn't the runtime detect external changes?** The VFS overlay only recorded the new proposed content, not the baseline state of the file when the turn began.
3. **Why was disk parity unchecked?** The VFS lacked an optimistic concurrency verification step comparing disk state at commit time with read time.

---

## Action items

1. **Add `VfsDiskBaseline`**: Store file size and FNV-1a 32-bit hash at staging time.
2. **Enforce baseline check on commit**: In `SessionVfs::commit_staged`, compare current disk hash against baseline; abort with `VfsBaselineMismatch` if different.
3. **Add regression test**: Add `test_vfs_baseline_verification_and_snapshot_rewind`.
4. **Document Playbook**: Create [PB-04: VFS Commit Conflict & Journal Rollback](../playbooks/vfs-commit-conflict-and-journal-rollback.md).

---

## Lessons learned

Agentic filesystem mutations must never operate on unverified assumptions of exclusive disk access. Optimistic concurrency with lightweight cryptographic baselines is mandatory for safe human-agent pair programming.
