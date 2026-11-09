<!-- [LAYER: UI] -->
---
name: "performance-auditor"
description: "Sub-Millisecond Latency, Zero-Allocation Arena, and Token Budget Optimizer"
category: "systems"
tier: 3
prerequisites: ["joyzoning-sovereignty"]
tags: ["performance", "latency", "tokens", "arena"]
---

# Sub-Millisecond Performance & Token Optimizer

Keep `HEAV3NS-MICRO` hyper-compressed, zero-bloat, and sub-millisecond across all hot paths:

1. **Arena & Memory Discipline**: Use `ArenaAllocator` 4-byte aligned slabs (`O(1)` snapshot rewind) and bounded collections to eliminate unbounded heap growth.
2. **Token Budget Governance**: Bound tool outputs via `ToolOutputGovernor` spill pagination and `snapshot_html_dom` script/style stripping so model context windows remain high-signal.
3. **Benchmark Verification**: Verify latency regressions with `./target/release/heav3ns-micro --bench` and `--flight`.

