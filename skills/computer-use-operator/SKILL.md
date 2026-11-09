<!-- [LAYER: UI] -->
---
name: "computer-use-operator"
description: "Set-of-Marks (SoM) GUI, Virtual Display, and Zero-Trust Browser DOM Operator"
category: "computer-use"
tier: 2
prerequisites: ["joyzoning-sovereignty"]
tags: ["computer-use", "gui", "som", "browser", "vision"]
---

# Set-of-Marks Computer Use & Browser Operator

Operate desktop windows, interactive GUI controls, and web DOM trees with deterministic precision:

1. **Capture Modes & Consecutive Deduplication**: Call `computer_use` (`action: "capture"`, `mode: "som" | "vision" | "ax"`) or `browser_use` (`action: "snapshot"`). Consecutive unchanged captures automatically set `screenUnchanged: true` (up to streak 2) to save vision tokens while preserving prompt cache prefixes.
2. **Target by `element_token`, `ref`, `som_id`, or `element_id`**: Prefer snapshot-scoped `element_token` (`s{frame_hex}:{som_id}`), legible preview `ref` (`btn-run`, `inp-cmd`), or `som_id` integer badges over raw `(x, y)` coordinates.
3. **Background-First Delivery & Verify -> Escalate Verdict Ladder**: Input actions default to `delivery_mode: "background"` (routing events without raising windows or stealing focus) and return a structured `verdict` (`decision: "done" | "verify_fresh_state" | "escalate"`, `effect: "confirmed" | "unverifiable" | "suspected_noop"`). Escalate to `set_value` or `delivery_mode: "foreground"` when `verdict.decision == "escalate"`.
4. **Human Desktop Control Lease & Epoch Fence**: Respect the epoch-fenced desktop lease (`lease_acquire`, `lease_release`, `lease_status`). When a human viewer holds control (`holder: "human"`), agent input actions fail closed (`human_has_control`).
5. **Delta Preview Driver & Persistent Annotations**: Use `action: "drive_preview"` (`command: "elements" | "click" | "fill"`) to receive compact delta inventories (`added`, `changed`, `removed`) after the initial full snapshot, and `action: "annotate_preview"` (`sub_action: "pin" | "hold" | "remove" | "clear"`) for persistent element annotations.
6. **SSRF, WHATWG Authority Ambiguity & JS Eval Pre-Screen**: Validate URLs via `browser_use`, which blocks cloud metadata (`169.254.169.254`, IPv4-mapped IPv6 `::ffff:169.254.169.254`), WHATWG authority ambiguities (`\`, `@`, `%`, fullwidth dots `。`), embedded API keys, and private URL literals inside `eval` expressions.
7. **Reversible State & Audit Trail**: Every GUI mutation records a checkpoint on the 32-deep undo/redo stack and logs to the queryable action ledger (`action: "search_dsl"`). Use `action: "undo"` immediately if an unexpected transition occurs.

