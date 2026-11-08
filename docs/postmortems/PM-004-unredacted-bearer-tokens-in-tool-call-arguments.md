<!-- [LAYER: UI] -->
# PM-004: Unredacted Bearer Tokens in Tool Call Arguments

- **Incident Date**: 2026-02-18
- **Severity**: SEV-1
- **Subsystem**: `secret_redactor` (`src/plumbing/secret_redactor.rs`)
- **Related ADR**: [ADR-0008](../adr/0008-secret-redaction-and-security-policy.md)
- **Resolved By**: Deterministic secret redaction filter applied to tool inputs and events

---

## Summary

During an automated debugging session, an agent read a `.env` file containing an API key and passed the unmasked string as an argument to `shell_execute`, broadcasting the secret to progress event listeners.

---

## Root cause

1. **Why was the key exposed?** The progress event listener emitted the raw tool argument string.
2. **Why wasn't the secret filtered?** Redaction was only applied to final assistant responses, not intermediate progress events or tool execution records.
3. **Why did the agent read the key?** The agent tool lacked a security interdiction check blocking sensitive credentials.

---

## Action items

1. **Integrate redactor at plumbing boundary**: Pass all progress events and tool arguments through [`DeterministicSecretRedactor`](../../src/plumbing/secret_redactor.rs).
2. **Add unit test**: Add `test_secret_redactor_and_sensitive_path_guard`.
3. **Document Playbook**: Create [PB-05: Secret Leakage & Security Interdiction](../playbooks/secret-leakage-and-security-interdiction.md).

---

## Lessons learned

Security sanitization must occur at the lowest plumbing layer before any event is dispatched to listeners or written to disk. Applying filters only at the UI layer invites leakages through background channels.
