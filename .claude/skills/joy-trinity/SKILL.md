---
name: joy-trinity
description: Operate durable jobs and buffered tables from any shell: enqueue and drain a persistent job queue, push and select rows, check the environment. Use when work must survive across turns or processes, needs idempotency, or needs a lightweight local database.
---
<!-- [LAYER: UI] -->

# joy-trinity: durable queue and tables

Everything is a shell command that prints **one JSON document** and exits `0` (ok), `1` (failure), or `2` (usage error, read the `hint`).
State persists under `--root` (default `./.joy-state`); commands lock the root automatically, so parallel calls are safe.

```bash
npx -y heav3ns-joy@1.0.0 trinity selftest        # prove it works here
npx -y heav3ns-joy@1.0.0 trinity doctor          # diagnose; every problem has a fix
npx -y heav3ns-joy@1.0.0 trinity queue add email --data '{"to":"ada"}' --key welcome:ada   # idempotent enqueue
npx -y heav3ns-joy@1.0.0 trinity queue list --state failed
npx -y heav3ns-joy@1.0.0 trinity queue drain email --handler ./handler.mjs                 # export default async (job) => result
npx -y heav3ns-joy@1.0.0 trinity db push docs --data '{"id":"1","title":"hi"}'
npx -y heav3ns-joy@1.0.0 trinity db select docs --where id=1 --order title:desc --limit 10
```

Rules: queue delivery is at-least-once, so make handlers idempotent and pass `--key`; `db push --type update|delete` needs `--where`;
use `--format text` for tables when a human is reading. `npx -y heav3ns-joy@1.0.0 trinity <command> --help` documents every option.
