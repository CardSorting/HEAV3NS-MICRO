<!-- [LAYER: UI] -->
# PM-0000: Postmortem Incident Title

- **Incident Date**: YYYY-MM-DD
- **Severity**: SEV-1 / SEV-2 / SEV-3
- **Subsystem**: Affected subsystem
- **Related ADR**: [ADR-0000](../adr/0000-adr-template.md)
- **Resolved By**: PR / Commit Link

---

## Summary

Executive summary of what failed, the immediate impact on runtime execution, duration of the failure, and how the incident was detected.

---

## Root cause

Detailed technical explanation tracing the failure via the 5 Whys. Explain the exact mechanism in memory, parsing, or filesystem state that allowed the incident to occur.

---

## Action items

1. **Immediate Mitigation**: Actions taken to unblock execution.
2. **Permanent Code Guard**: Invariant or defense added to `src/`.
3. **Automated Regression Test**: Test case added in `src/core/substrate_tests.rs`.
4. **Documentation & KB Update**: Relevant playbook and symptom row created.

---

## Lessons learned

Key engineering insights gained from this failure regarding spatial sovereignty, memory bounding, or parser robustness.
