<!-- [LAYER: UI] -->
# Playbook PB-05: Secret Leakage & Security Interdiction

<p align="center">
  <a href="README.md">Playbooks Hub</a> ·
  <a href="../README.md">Docs Hub</a> ·
  <a href="../adr/0008-secret-redaction-and-security-policy.md">ADR-0008</a> ·
  <a href="../SECURITY_BEST_PRACTICES.md">Security Best Practices</a>
</p>

## Triage at a glance

| Field | Detail |
| :--- | :--- |
| **Playbook ID** | `PB-05` |
| **Severity** | `SEV-1` (Potential credential leak / security violation) |
| **Trigger Codes** | `SensitivePathBlocked`, `SecretDetected`, unmasked API token |
| **Time to Mitigate** | `1–10 minutes` |
| **First Command** | `cargo test test_secret_redactor_and_sensitive_path_guard` |
| **Rollback Safety** | Security filters operate in-memory prior to output emission |

---

## Remediation steps

### Case 1: Sensitive File Path Blocked (`SensitivePathBlocked`)
1. An agent tool call attempted to access a sensitive system path (`~/.ssh`, `~/.aws`, `/etc/shadow`, etc.).
2. The runtime correctly blocked the execution with a security denial.
3. **Action**:
   - Instruct the agent to confine file operations strictly within the configured workspace directory (`workspace_root`).
   - If a valid task legitimately requires accessing external files, verify that the requested path is not within the interdicted security list.

### Case 2: Credential Exposure in Output Stream
1. If an unredacted API key or secret token was observed in output:
2. **Immediate Remediation**:
   - Invalidate and rotate the exposed credential immediately.
   - Inspect the regex patterns in `src/plumbing/secret_redactor.rs` to understand why the token format was not matched.
   - Add the new token pattern or entropy threshold to [`DeterministicSecretRedactor`](../../src/plumbing/secret_redactor.rs).
3. **Add regression test**:
   - Add a test case in `src/core/substrate_tests.rs` verifying that the new credential pattern is replaced with `[REDACTED_API_KEY]`.

---

## Verification

```bash
cargo test test_secret_redactor_and_sensitive_path_guard
```
