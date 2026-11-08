---
description: Audit the project (or one file) against Joyzoning and fix blocking violations
argument-hint: "[file]"
allowed-tools: Bash(npx -y heav3ns-joy@1.0.0 check:*), Read, Edit
---
<!-- [LAYER: UI] -->

Run `npx -y heav3ns-joy@1.0.0 check . --phase post-edit --scope $ARGUMENTS --format compact` (omit `--scope` when no file is given), then fix each blocking finding.
Use `npx -y heav3ns-joy@1.0.0 check --explain <CODE>` for rationale. Re-run until it exits 0. Report anything you could not fix; never weaken the configuration.
