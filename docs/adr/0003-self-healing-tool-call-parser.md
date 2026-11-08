<!-- [LAYER: UI] -->
# ADR-0003: Streaming Self-Healing JSON Repair & Deduplication Engine

- **Status**: Accepted
- **Date**: 2026-02-03
- **Deciders**: HEAV3NS Core Architecture Team
- **Technical Story**: [`src/plumbing/json_repair.rs`](../../src/plumbing/json_repair.rs)

---

## Status

Accepted.

## Context

When LLMs stream tool call arguments, network packets or output token budget cutoffs frequently deliver malformed payloads:
- Unbalanced curly braces or brackets (`{"path": "src/main.rs"`)
- Literal markdown fences wrapping JSON blocks (` ```json ... ``` `)
- Escaped quotes and control characters inside patch hunks
- Duplicate streaming chunks repeating previous argument tokens

Standard strict JSON deserializers (`serde_json::from_str`) immediately return fatal parse errors on these inputs, prematurely terminating autonomous agent workflows.

### Decision drivers

- 100% resilience against common LLM formatting hallucinations.
- Deduplication of streaming argument token deltas.
- Zero-copy extraction of completed tool call arguments.

### Considered options

1. **Strict JSON Deserialization with Retry Prompts**: High token consumption and multi-second latency for formatting errors.
2. **Ad-Hoc Regex Extraction**: Incomplete handling of nested JSON structures and escaped quotes.
3. **Multi-Stage Self-Healing Repair Pipeline**: Incremental bracket depth balancing, markdown delimiter stripping, and character normalization before deserialization.

## Decision

We chose **Option 3: Multi-Stage Self-Healing JSON Repair** implemented in [`src/plumbing/json_repair.rs`](../../src/plumbing/json_repair.rs):
- `repair_json_string()` balances trailing braces and unescapes invalid sequences.
- `extract_json_roots()` locates root-level JSON objects even when surrounded by prose.
- `compute_streaming_arguments_delta()` detects repeated string prefixes and outputs clean incremental argument deltas.

## Consequences

### Positive

- Zero fatal agent aborts caused by truncated braces or markdown fences.
- Clean tool dispatch under token-limited streaming conditions.

### Negative / trade-offs

- Severely corrupted payloads (e.g. truncated mid-identifier) fall back to safe error records rather than executing hallucinated commands.

## Verification

```bash
cargo test test_self_healing_parser_and_streaming_dedup
```
