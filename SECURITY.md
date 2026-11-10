<!-- [LAYER: UI] -->
# Security Policy

## Architectural Security Controls in `HEAV3NS-MICRO`

1. **Workspace Path Confinement ([`resolve_workspace_path`](src/infrastructure/search_tools.rs))**:
   - All filesystem tool operations resolve relative to the workspace root and reject `..` path traversal escapes outside the workspace boundary.
2. **Parameterized macOS Seatbelt Sandboxing ([`seatbelt_profile.rs`](src/plumbing/seatbelt_profile.rs))**:
   - Subprocess executions (`execute_shell_command_sandboxed`) generate parameterized SBPL profiles for `/usr/bin/sandbox-exec`, enforcing file write write-deny policies on `.git/hooks`, `.git/config`, `~/.ssh`, `~/.aws`, `~/.gnupg`, and system directories outside the allowed workspace scope.
3. **Dual-Scope Credential Denylist & Secret Redaction ([`DeterministicSecretRedactor`](src/plumbing/secret_redactor.rs))**:
   - Enforces `CredentialDenyScope`: blocks `.env*`, `.ssh/id_rsa`, `.aws/credentials`, `.kube/config`, `.gnupg`, `.netrc`, `.git-credentials` in model context tools while explicitly allowing non-sensitive templates like `.env.example` and `.env.template`. Automatically scrubs 8 secret families (OpenAI, Anthropic, AWS, GitHub, Slack, Google API, Bearer, and PEM) from model context and logs.
4. **Destructive Shell Command Guard & Pipeline Defenses ([`SafetyPolicy`](src/plumbing/secret_redactor.rs), [`shell_guard.rs`](src/plumbing/shell_guard.rs))**:
   - Blocks destructive shell commands (`rm -rf /`, `mkfs`, `dd if=`, fork bombs, `git reset --hard`) before execution.
   - Enforces heredoc file-write interdiction (`ADR-0015`) and compound terminal file-read anti-starvation guards (`ADR-0016`).
   - Uses Unix `.process_group(0)` process group termination (`SIGKILL` to `-pgid`) and concurrent pipe draining to prevent subprocess deadlocks and dangling orphan processes.
5. **Fail-Closed Repository Governance ([`RepoConstitution`](src/domain/types/posture_types.rs))**:
   - `.heav3ns/constitution.json` is strictly validated. Any malformed, unparseable, or tampered policy fails closed (`AuthorizationVerdict::AskUser`) rather than degrading into permissive execution.
6. **Pure-Rust CAS Undo & Atomic Rollback Journal ([`WorkspaceSnapshotCas`](src/infrastructure/workspace_cas.rs), [`TransactionJournal`](src/infrastructure/transaction_journal.rs))**:
   - Captures pre-mutation file snapshots prior to every mutating tool call (`TransactionJournal`) and SHA-256 pre/post-turn CAS trees (`WorkspaceSnapshotCas`).
   - Protects post-turn undo with `WouldBlock` verification: if a file has been edited externally after turn completion, rollback refuses to clobber user work.
7. **Secure Cache & Credential Permissions**:
   - Local configuration files and cached data in `~/.heav3ns/` (including `pricing_catalog.json` and session tokens) are created with atomic Unix permissions `0700` (directory) and `0600` (files).

