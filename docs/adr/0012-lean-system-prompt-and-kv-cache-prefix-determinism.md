<!-- [LAYER: UI] -->
# ADR-0012: Lean System Prompt Specification, KV-Cache Prefix Determinism & Sticky Dynamic Tool Routing

- **Status**: Accepted
- **Date**: 2026-11-10
- **Deciders**: HEAV3NS Architecture Working Group
- **Technical Story**: Zenith Pass — Lean System Prompt Parity & Token Governance

---

## Status

Accepted

## Context

Legacy agent runtimes like `LUMI` transmit 15,000–20,000 tokens of redundant prompt instructions on every turn across 60+ fragmented components. Conversely, prior versions of `heav3ns-micro` transmitted an over-compressed 35-token role string but dumped all 29 built-in tool schemas (~4,278 tokens) unconditionally on every turn, and dynamic workspace facts were rendered ahead of instructions, invalidating provider KV prompt caches across sessions.

### Decision drivers

- Maximize provider KV prompt-cache reuse across turns and sessions via byte-deterministic static prefixing.
- Provide full imperative execution discipline (`INSPECT -> ACT -> OBSERVE -> ADAPT -> VERIFY`) without conversational sycophancy or bloat.
- Reduce per-turn schema overhead from ~4,278 tokens to ~1,510 tokens while preventing amnesia on multi-step specialist workflows.
- Prevent runaway recursive delegation in `The Legion` multi-agent swarm specialists.

### Considered options

1. **Option 1**: Monolithic static prompt (rigid, workspace-unaware, bloated schemas).
2. **Option 2**: Stateless dynamic schema filtering (drops specialist tools like `browser_use` on follow-up turns).
3. **Option 3 (Chosen)**: Split `SystemPromptSpec` with byte-deterministic KV-cache static prefixing (~400t, FNV-1a fingerprint), sticky on-demand tool routing (15 active core + 14 on-demand), and bounded handoffs.

## Decision

We implemented:
1. **`SystemPromptSpec` (`src/domain/prompt_spec.rs`)**:
   - `static_prefix`: An immutable, workspace-independent constitutional prefix (~400 tokens) with an 8-character FNV-1a hex fingerprint (`cache_prefix_fingerprint`), ensuring 100% byte identity across sessions.
   - `dynamic_suffix`: Dynamically appends workspace facts (`cwd`, `os`, `shell`, `manifests`), Joyzoning architecture summaries, active `SoulProfile` axioms, and sanitized governance rules (`SOUL.md`, `.cursorrules`).
   - `PromptProfile`: `Standard`, `Compact` (<260t for local models), `SwarmSpecialist` (with exclusive file reservations and no recursive delegation), and `SideQuestion` (read-only 0-tool mode).
   - `ModelPromptFamily`: Tailored execution hints for OpenAI reasoning, Gemini, Anthropic, and compact local models.
2. **Sticky Tool Schema Routing (`src/core/tool_registry.rs`)**:
   - 15 core active tools (~1,510t) are always active.
   - 14 specialist tools unlock on-demand and remain sticky for the task duration, saving 2,768 tokens (64.7%) per coding turn.
   - Child engines in `The Legion` explicitly forbid `engage_swarm` via `AgentEngine::without_swarm_delegation`.

## Consequences

### Positive

- **Massive Token Savings**: Saves ~2,768 tokens of schema overhead per turn and cuts base system prompt bloat by over 85% compared to `LUMI`.
- **KV Prompt Cache Optimization**: Byte-deterministic `static_prefix` hits 100% provider prompt-cache reuse.
- **Swarm Safety**: Recursive delegation loops are structurally eliminated at both schema and dispatch levels.
- **Observability**: Exposes `/context` and `/prompt` inspector HUDs, CLI `heav3ns-micro context`, and `"context.inspect"` RPC telemetry.

### Negative / trade-offs

- Specialist tools must be mentioned or triggered by intent before their full parameter schemas are loaded into the LLM context window.

## Verification

```bash
# 1. Verify prompt cache prefix determinism and token budget assertions
cargo test core::zenith_tests::tests::test_prompt_cache_prefix_determinism_and_model_families
cargo test core::substrate_tests::tests::test_lean_system_prompt_parity_budgets_and_sanitization

# 2. Verify sticky tool schema routing and token savings
cargo test core::substrate_tests::tests::test_sticky_dynamic_tool_schema_routing

# 3. Verify visual context HUD and RPC telemetry
cargo run -- context
```
