<!-- [LAYER: UI] -->
# Security Policy

## Architectural Security Controls in `HEAV3NS-MICRO`

1. **Workspace Path Confinement ([`resolve_workspace_path`](src/infrastructure/search_tools.rs))**:
   - All filesystem tool operations resolve relative to the workspace root and reject `..` path traversal escapes outside the workspace boundary.
2. **Sensitive Credential Denylist & Secret Redaction ([`DeterministicSecretRedactor`](src/plumbing/secret_redactor.rs))**:
   - Blocks reads and writes to sensitive credential paths (`.ssh/id_rsa`, `.aws/credentials`, `.kube/config`, `.gnupg`, `.netrc`, `.git-credentials`) and automatically scrubs OpenAI, Anthropic, AWS, GitHub, Slack, Google API, Bearer, and PEM secrets from tool outputs and session logs.
3. **Destructive Shell Command Guard ([`SafetyPolicy`](src/plumbing/secret_redactor.rs))**:
   - Blocks destructive shell commands (`rm -rf /`, `mkfs`, `dd if=`, fork bombs, `git reset --hard`) before execution.
4. **Atomic Rollback Journal ([`TransactionJournal`](src/infrastructure/transaction_journal.rs))**:
   - Captures pre-mutation file snapshots prior to every mutating tool call and supports deterministic rollback (`rollback_last_mutation`).

