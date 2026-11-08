<!-- [LAYER: UI] -->
# Glossary

<p align="center">
  <a href="README.md">Docs Hub</a> ·
  <a href="CONSTITUTION.md">Constitution</a> ·
  <a href="CODEBASE_STANDARDS.md">Codebase Standards</a> ·
  <a href="api/public-api.md">Public API</a>
</p>

Definitions of core terms, architectural concepts, and code abbreviations used across **HEAV3NS-MICRO**.

---

| Term | Definition |
| :--- | :--- |
| **Joyzoning** | The architectural discipline dividing code into five sovereign zones (`DOMAIN`, `CORE`, `INFRASTRUCTURE`, `PLUMBING`, `UI`) tagged with top-of-file headers. |
| **Zone** | One of the 5 canonical layers governing module responsibilities and dependency direction. |
| **PGA** (Principle of Geographic Alignment) | The rule requiring a file's physical directory path (`src/<zone>/`) to match its declared `[LAYER: <ZONE>]` header tag. |
| **Contractual Sovereignty** | The rule requiring `CORE` orchestrators to depend exclusively on abstract `I<Name>` traits in `src/domain/interfaces/`, never directly on concrete infrastructure structs. |
| **`ArenaAllocator`** | High-performance contiguous memory slab pre-allocating a 16-MiB buffer with linear bump-pointer allocation and $O(1)$ tick unwinding ([ADR-0002](adr/0002-arena-allocator-and-o1-tick-rewind.md)). |
| **$O(1)$ Tick Rewind** | Resetting the arena allocation watermark to a prior tick checkpoint in constant time, instantly recycling scratch memory without heap fragmentation. |
| **Compaction** | Semantic summarization of historical dialogue turns when token depth approaches context window thresholds (`CompactionEngine`). |
| **`SessionVfs`** | In-memory Virtual File System layer that stages modifications and verifies disk baselines before applying mutations to physical disk ([ADR-0004](adr/0004-two-phase-vfs-staging-and-transaction-journal.md)). |
| **`VfsDiskBaseline`** | Pre-modification snapshot consisting of file size and FNV-1a 32-bit hash used to detect concurrent external modifications. |
| **`TransactionJournal`** | Append-only ledger recording every committed filesystem modification with full reverse patch data for deterministic undo. |
| **`WaveScheduler`** | Concurrency engine that partitions emitted tool calls into parallel read wavefronts and serialized write barriers ([ADR-0005](adr/0005-dag-wave-scheduler-concurrency.md)). |
| **Execution Authority Level** | Permission tier (`ReadOnly`, `WorkspaceWrite`, `ShellExecute`, `FullSovereign`) governing allowed tool invocations. |
| **Self-Healing Parser** | Streaming JSON parser capable of repairing unclosed braces, escaped characters, markdown fences, and fragmented tool names ([ADR-0003](adr/0003-self-healing-tool-call-parser.md)). |
| **`DeterministicSecretRedactor`** | Zero-allocation security filter matching and redacting API keys, bearer tokens, private keys, and sensitive filesystem paths. |
| **Agent Flight Test (`--flight`)** | Comprehensive 5-scenario benchmark (`FT-01..FT-05`) verifying runtime memory, VFS, parser, scheduler, and security integrity. |
| **Stdio Protocol (`--stdio`)** | Newline-delimited JSONL stream over standard input and output for integration with IDEs and external supervisors. |
| **ADR** (Architecture Decision Record) | Immutable MADR-format document in [`docs/adr/`](adr/README.md) recording architectural context, options, decision, consequences, and verification. |
| **Playbook (`PB-xx`)** | Actionable operational runbook in [`docs/playbooks/`](playbooks/README.md) designed for incident response and failure triage. |
| **Postmortem (`PM-xxx`)** | Blameless retrospective in [`docs/postmortems/`](postmortems/README.md) analyzing a failure and closing the loop with code invariants and regression tests. |
| **Knowledge Base (`docs/kb/`)** | 3-tier heuristic and documentation repository mapping symptoms to root causes and fixes. |
| **Tier 1 Optimal** | Module complexity standard requiring all files to stay under 800 lines of code (`SPI-203`). |
