<!-- [LAYER: UI] -->
# ADR-0015: Auto-Refreshing 2026 Frontier Model Catalog, Dynamic Capability Taxonomy & Zero-Cost Billing Integrity

## Status

**Accepted**

## Context

Prior to this architecture decision, model pricing, token limits, and capability tags in **HEAV3NS-MICRO** were stored in hardcoded lookup tables (`src/domain/pricing_and_taxonomy.rs`). While functional for static testing, this design suffered from three critical production vulnerabilities:

1. **Frontier Model Staleness & Phantom Defaults**: 2024/2025 default models (e.g. `gpt-4o`, `claude-3-5-sonnet`, `gemini-1.5-pro`) quickly became legacy compared to 2026 frontier releases (`gpt-6.1-terra`, `gpt-6.1-terra-pro`, `gpt-5.6-terra`, `claude-sonnet-5.5`, `claude-opus-5.5`, `gemini-4-carbon`, `deepseek-v4.1-flash`, `grok-4.7`, `qwen3.8-max-prime`). Hardcoded models require manual engine recompilation whenever providers adjust rates, release new model variants, or update context windows.
2. **Deceptive `$0.0000` Billing & Cost Hallucination**: Querying an unpriced or third-party model previously evaluated to `$0.0000` total cost, masking API expenses in operator logs and user receipts. Conversely, local models (e.g., Ollama, Apple MLX) were not cleanly distinguished from remote pay-per-token APIs (such as cloud-hosted Qwen endpoints).
3. **Provider Failover & Tier Desynchronization**: Automatic model fallback and intelligent tier routing (`economy`, `standard`, `frontier`, `codex`) could not discover newly deployed models at runtime without restarting the process.

## Decision

We establish an auto-refreshing model catalog and taxonomy architecture spanning `DOMAIN`, `INFRASTRUCTURE`, `CORE`, and `UI`:

1. **Domain Representation (`src/domain/types/model_catalog.rs`)**:
   - Define pure domain contracts `IModelCatalog` and `IModelTierRecommender`.
   - Implement `ModelPricingCatalog` (`heav3ns.model_catalog/v1`) with an embedded 2026 frontier baseline containing exact pricing and context ceilings for OpenAI, Anthropic, Google Gemini, DeepSeek, xAI Grok, Qwen, and Moonshot Kimi.
   - Implement `ModelPricingCatalog::parse_openrouter_models_json(raw_json, fetched_at_ms)` to ingest over 460+ live models dynamically from OpenRouter (`https://openrouter.ai/api/v1/models`). The parser ignores batch/free sentinels, handles negative price markers, indexes models by both `vendor/slug` and bare `slug`, and keeps the newest snapshot by `created_unix`.
2. **Infrastructure Caching & Background Refresh (`src/infrastructure/pricing_updater.rs`)**:
   - Define `IModelPricingRegistry` in `src/domain/interfaces/mod.rs` and implement `ModelPricingRegistry` with atomic `0600` disk persistence at `~/.heav3ns/pricing_catalog.json`.
   - Implement a configurable 12-hour TTL (`HEAV3NS_PRICING_TTL_SECS`, default `43200`), offline opt-out (`HEAV3NS_OFFLINE_PRICING=1`), custom endpoint override (`HEAV3NS_MODEL_CATALOG_URL`), and thread-safe deduplicated background refreshing (`ensure_fresh_background`).
3. **Honest Billing & Dynamic Taxonomy (`src/domain/pricing_and_taxonomy.rs`)**:
   - Refactor `evaluate_with_catalog` to categorize costs into `Metered`, `LocalNoBill`, `Subscription`, or `Unknown` (`not_recorded: true`). Cloud-hosted Qwen models are metered accurately, while true local endpoints (`ollama`, `apple*`, `127.0.0.1`, `:local`) are classified as `LocalNoBill`.
   - Dynamically route compatible fallback models and tier recommendations via catalog metadata.
4. **Ergonomic CLI, REPL & Stdio RPC Surface (`src/ui/`, `src/core/`)**:
   - Provide `heav3ns-micro models [--refresh]` CLI subcommand and `/models` interactive slash command displaying live models, context windows, and input/output rates.
   - Provide `models.discover` and `models.refresh` JSON-RPC methods for desktop client inspection.

## Consequences

- **Positive**: Zero recompilation required to support newly released models; cache persists securely with `0600` permissions; background updates never block turn execution; and operator receipts never report false `$0.0000` costs.
- **Trade-offs**: First boot without an existing disk cache makes a single non-blocking HTTP GET to the OpenRouter catalog; if network is unavailable or `HEAV3NS_OFFLINE_PRICING=1` is set, the system falls back seamlessly to the embedded 2026 frontier catalog.

## Verification

```bash
cargo test core::codewhale_zenith_tests
cargo run -- models --refresh
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```

