<!-- [LAYER: UI] -->
# ADR-0008: Deterministic Secret Redaction & Sensitive Path Interdiction

- **Status**: Accepted
- **Date**: 2026-02-08
- **Deciders**: HEAV3NS Core Architecture Team
- **Technical Story**: [`src/plumbing/secret_redactor.rs`](../../src/plumbing/secret_redactor.rs) · [`docs/SECURITY_BEST_PRACTICES.md`](../SECURITY_BEST_PRACTICES.md)

---

## Status

Accepted.

## Context

Autonomous coding agents routinely process environment variables, repository configurations, command outputs, and shell executions. If an agent inspects a configuration file containing an API key or SSH private key, these credentials can leak into progress telemetry streams, session logs, or subsequent LLM prompt contexts.

### Decision drivers

- Zero credential leakage across logs and event streams.
- Zero external allocation during redaction filtering.
- Deterministic path interdiction preventing unauthorized reads to sensitive operating system paths.

### Considered options

1. **Ad-Hoc String Matching in Presenters**: Vulnerable to omissions and difficult to maintain uniformly.
2. **Third-Party Sanitization Libraries**: Adds heavy dependencies and potential runtime panic vectors.
3. **Stateless Deterministic Secret Redactor in `src/plumbing/`**: Zero-allocation regular expression engine with static patterns matching known key prefixes and sensitive paths.

## Decision

We chose **Option 3: Deterministic Secret Redactor** implemented in [`src/plumbing/secret_redactor.rs`](../../src/plumbing/secret_redactor.rs):
- Interdicts sensitive filesystem paths (`~/.ssh`, `~/.aws`, `~/.gnupg`, `/etc/shadow`, `/etc/passwd`).
- Redacts high-entropy API tokens (`sk-`, `ghp_`, `AKIA`, generic Bearer authorization headers).
- Intercepts both outbound tool inputs and streaming progress events.

## Consequences

### Positive

- Complete protection against accidental credential exposure.
- Fully verified in `test_secret_redactor_and_sensitive_path_guard`.

### Negative / trade-offs

- Legitimate strings matching key patterns (e.g. documentation examples) may be masked as `[REDACTED_API_KEY]`.

## Verification

```bash
cargo test test_secret_redactor_and_sensitive_path_guard
```
