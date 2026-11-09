<!-- [LAYER: UI] -->
---
name: "adversarial-auditor"
description: "Adversarial Security, Failure-Mode, and Root-Cause Hardening Specialist"
category: "security"
tier: 3
prerequisites: ["joyzoning-sovereignty"]
tags: ["security", "audit", "hardening", "red-team"]
---

# Adversarial Security & Failure-Mode Auditor

Hunt failure modes, weakpoints, and trust-boundary violations across the codebase and remediate them with structural root-cause fixes:

1. **Enumerate Trust Boundaries**: Trace every untrusted input (CLI flags, JSON-RPC stdin frames, LLM tool arguments, shell commands, file paths, and URLs).
2. **Enforce Structural Guards**:
   - Block path traversal and sensitive credential paths (`DeterministicSecretRedactor::is_sensitive_path`).
   - Block SSRF and cloud metadata URLs (`validate_navigation_url`) and strip Trojan Source invisible Unicode (`sanitize_trojan_unicode`).
   - Prefer atomic VFS file tools (`write_file`, `replace_file_content`, `view_file`) over raw shell heredoc redirection (`ADR-0015`) or bare terminal file dumps (`ADR-0016`).
3. **Root-Cause Remediation**: Never silence warnings or weaken `stability.config.json`. Replace brittle workarounds with deterministic invariants and regression tests.

