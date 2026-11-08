<!-- [LAYER: UI] -->
# ADR-0006: Headless Stdio JSONL Worker Protocol & 5-Scenario Flight Test

- **Status**: Accepted
- **Date**: 2026-02-06
- **Deciders**: HEAV3NS Core Architecture Team
- **Technical Story**: [`src/core/flight_runner.rs`](../../src/core/flight_runner.rs) · [`src/ui/cli_presenter.rs`](../../src/ui/cli_presenter.rs)

---

## Status

Accepted.

## Context

To be broadly adopted, an agent runtime must integrate cleanly with diverse external host environments: terminal CLIs, VS Code extensions, web supervisors, and headless CI test harnesses. Additionally, validating end-to-end multi-turn runtime behavior against live LLM APIs in CI is non-deterministic, expensive, and fragile.

### Decision drivers

- Standardized machine-readable inter-process communication protocol.
- Deterministic, zero-network multi-turn flight benchmark testing the full agent loop.

### Considered options

1. **HTTP/gRPC Microservice**: Heavyweight network stack, port contention, and firewall hurdles.
2. **Ad-Hoc Text Pipe**: Fragile framing and poor support for streaming progress events.
3. **Newline-Delimited JSON RPC over Stdio (`--stdio`) + Offline 5-Scenario Flight Test Suite (`--flight`)**: Clean JSONL framing on standard streams coupled with a deterministic scripted flight runner.

## Decision

We chose **Option 3: Stdio JSONL Worker Protocol and Embedded Flight Test Suite**:
- `--stdio` processes incoming requests and streams progress events line-by-line using standard JSON RPC envelopes.
- `--flight` executes five canonical end-to-end benchmark scenarios (`FT-01` through `FT-05`) verifying discovery, VFS staging, parser repair, wave scheduling, and security redaction against an offline mock provider ([`ScriptedProvider`](../../src/infrastructure/scripted_provider.rs)).

## Consequences

### Positive

- Trivial integration with any language or IDE capable of spawning child processes.
- Instant, deterministic offline regression testing across all 19 flight assertions in CI.

### Negative / trade-offs

- Debug logs must be directed to stderr to prevent corrupting stdout JSONL message framing.

## Verification

```bash
cargo run -- --flight
cargo test test_agent_flight_suite_passes_100_percent
```
