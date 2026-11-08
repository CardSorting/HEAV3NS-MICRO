---
description: Plan and run a specialist swarm for a task using the joy-swarm skill
argument-hint: "<goal>"
allowed-tools: Bash(npx -y heav3ns-joy@1.0.0 trinity:*), Read, Edit, Write
---
<!-- [LAYER: UI] -->

Use the `joy-swarm` skill for: $ARGUMENTS

Plan with `npx -y heav3ns-joy@1.0.0 trinity swarm plan --task "$ARGUMENTS" --cwd . --id swarm-$(date +%s)`, then loop `next` → do each claimed specialist's work → `report` until `done`, and finish with `finish`. Summarize the final handoff.
