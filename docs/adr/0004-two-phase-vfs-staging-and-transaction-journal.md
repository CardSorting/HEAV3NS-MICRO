<!-- [LAYER: UI] -->
# ADR-0004: Two-Phase Staged VFS with Disk Baseline & Rollback Journal

- **Status**: Accepted
- **Date**: 2026-02-04
- **Deciders**: HEAV3NS Core Architecture Team
- **Technical Story**: [`src/infrastructure/vfs_store.rs`](../../src/infrastructure/vfs_store.rs) · [`src/infrastructure/transaction_journal.rs`](../../src/infrastructure/transaction_journal.rs)

---

## Status

Accepted.

## Context

When autonomous agents execute code-editing tools (`fs_write`, `patch_apply`), writing directly to physical disk presents major hazards:
- Speculative edits based on stale file states can overwrite changes made concurrently by a developer.
- If an agent generates broken diffs across multiple files, partial writes leave the git working tree in an uncompilable state.
- Without an atomic undo mechanism, recovery requires manual git stashing or checking out prior commits.

### Decision drivers

- Optimistic concurrency control detecting external file modifications.
- Atomic commit across multi-file modifications.
- Deterministic, one-click rollback of any committed file action.

### Considered options

1. **Direct Filesystem Mutation**: High risk of file corruption and clobbered developer edits.
2. **Git Branch Isolation**: Spawns heavy external git processes and requires resolving merge conflicts.
3. **Two-Phase In-Memory Staging (`SessionVfs`) + Baseline Hashing + Transaction Journal (`TransactionJournal`)**: Staged in-memory overlays verified against disk FNV-1a hashes before commit, with reverse patch logging for instant undo.

## Decision

We chose **Option 3: Two-Phase Staged VFS with Transaction Journaling**:
- Tools write into [`SessionVfs`](../../src/infrastructure/vfs_store.rs) overlays.
- Every overlay captures a [`VfsDiskBaseline`](../../src/domain/types/vfs_types.rs) (byte length and 32-bit FNV-1a hash).
- On commit (`commit_staged`), if the physical disk content differs from the baseline, the commit aborts with `VfsBaselineMismatch`.
- Successful commits record reverse diffs in [`TransactionJournal`](../../src/infrastructure/transaction_journal.rs).

## Consequences

### Positive

- Complete protection against clobbered disk edits.
- Instant rollback via `/rewind` or `session_rollback` tool.
- Clear separation between agent intent (staged) and repository truth (committed).

### Negative / trade-offs

- The agent or user must explicitly trigger `commit` to finalize file modifications.

## Verification

```bash
cargo test test_vfs_baseline_verification_and_snapshot_rewind
```
