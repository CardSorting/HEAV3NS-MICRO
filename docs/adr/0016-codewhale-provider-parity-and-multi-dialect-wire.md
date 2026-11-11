<!-- [LAYER: UI] -->
# ADR-0016: Codewhale Provider Parity, 4-Dialect Wire Engine, Secret-Slot Isolation, and Full-Stack AppKit/CLI Synchronization

## Status

**Accepted** — 2026-11-10

## Context

Prior to this ADR, **HEAV3NS-MICRO** supported a small subset of LLM routes (`openai`, `anthropic`, `gemini`, `ollama`, and four gateway proxies in the AppKit desktop client), whereas **Codewhale** (`provider_descriptors.json`, `openai_responses.rs`, `provider_chain.rs`, `schema_sanitizer.rs`) supported 40+ canonical providers, native OpenAI `/v1/responses` wire semantics, multi-provider failover chains, and strict tool schema sanitization.

Auditing the existing **HEAV3NS-MICRO** provider stack also exposed five critical reliability and security ceilings:

1. **Cross-Provider Secret & Wire Leakage**: Hot-switching providers via `AgentEngine::switch_http_provider` or `/provider` could retain an incompatible `wire_dialect` override or reuse a single global `api_key` across unrelated provider families unless strictly isolated by credential slot.
2. **Missing OpenAI `/v1/responses` & Multi-Dialect SSE Parity**: Codex/Responses models (`openai-codex`, `concentrate`) require `store: false`, `instructions`, `function_call` / `function_call_output` items, flat tool definitions, and streaming `response.output_text.delta` / `response.function_call_arguments.delta` / `response.completed` SSE parsing.
3. **Anthropic `/v1/messages` Dangling `tool_use` & Mid-Transcript `System` Hazards**: Cancelled or compacted turns leaving an unmatched `tool_use` block caused Anthropic `/v1/messages` to reject subsequent turns with HTTP `400`, and mid-transcript `Role::System` notes risked mutating the top-level `system` KV-cache prefix.
4. **Strict Tool JSON Schema Rejections**: Providers such as DeepSeek, OpenAI Responses, xAI, Mistral, and Gemini reject nullable `anyOf: [T, {"type": "null"}]` unions, bare `"type": "object"` schemas without `"properties"`, root `oneOf`/`anyOf` tool unions, or dangling `"required"` entries.
5. **AppKit ↔ Stdio Worker Catalog Desynchronization**: `WorkspaceModelSelection.providers` in `ModelPickerState.swift` only accepted 7 hardcoded IDs and capped `WorkspaceModelCatalog` at `16` providers, silently dropping `gemini`, `ollama`, `deepseek`, `anthropic`, `xai`, `groq`, `mistral`, and `qwen`.

## Decision

We implement full **Codewhale Provider Parity** across all five Joyzoning zones without external SDK bloat:

1. **`PLUMBING` (`src/plumbing/schema_and_sse.rs`)**:
   - Implemented `parse_sse_events` supporting W3C multi-line `data:` frames, compact blank-line-free OpenAI/vLLM `data:` streams (via zero-allocation `serde::de::IgnoredAny` boundary detection), CRLF line endings, `: comment` heartbeats, `[DONE]` sentinels, and an 8 MiB `MAX_SSE_EVENT_DATA_BYTES` OOM guard.
   - Implemented `sanitize_tool_schema` and `sanitize_tool_schema_for_responses` (nullable union collapse, bare object `"properties": {}` injection, dangling `"required"` pruning, and root union flattening) plus `redact_endpoint_for_display`.
2. **`DOMAIN` (`src/domain/types/provider_registry.rs`)**:
   - Distilled `PROVIDER_DESCRIPTORS` (`43` canonical providers + aliases across OpenAI, OpenAI Codex, Claude Code, Anthropic, Gemini, DeepSeek, OpenRouter, xAI, Groq, Mistral, Qwen, Moonshot/Kimi, Z.ai GLM, MiniMax, Volcengine Ark, NVIDIA NIM, Together, Fireworks, DeepInfra, SiliconFlow, Cerebras, Baseten, Novita, Xiaomi MiMo, StepFun, Hugging Face, Perplexity, Cohere, Concentrate, OpenCode, Cline Pass, Nous, Ollama, vLLM, SGLang, LM Studio, Apple Playground, and Custom Gateways).
   - Implemented zero-I/O `resolve_provider_route` with strict `secret_slot` isolation, `WireDialect` (`ChatCompletions`, `AnthropicMessages`, `OpenAiResponses`, `GoogleGemini`), `ReasoningEffortLevel`, and `ProviderChain` failover state.
   - Extended `RequestManifest` (`src/domain/prefix_and_manifest.rs`) with `provider`, `wireDialect`, `endpointDisplay`, and `reasoningEffort` while preserving exact canonical byte-sum identity, and wired `ProviderBillingClass` into `BillingMetering` (`src/domain/pricing_and_taxonomy.rs`).
3. **`INFRASTRUCTURE` (`src/infrastructure/wire_dialects.rs`, `sse_and_response_parsers.rs`, `http_provider.rs`)**:
   - Implemented all four outbound wire builders (`build_chat_completions_payload`, `build_openai_responses_payload`, `build_anthropic_messages_payload` with automatic `repair_dangling_anthropic_tool_uses` and adaptive `thinking` + `output_config.effort`, and `build_google_gemini_payload`) and unified multi-dialect SSE/JSON response parsers (`parse_multidialect_sse_stream`, `parse_responses_json`).
4. **`CORE` (`src/core/turn_orchestrator.rs`, `context_inspector.rs`, `slash_and_swarm.rs`)**:
   - Hardened `switch_http_provider_with_vault` to reset stale cross-family `fallback_model` and `wire_dialect` overrides, prevent cross-provider API key carryover, execute pre-side-effect `ProviderChain` failover in `complete_round_with_backoff`, and expose `/provider`, `/providers`, and `/reasoning` slash commands.
5. **`UI` (`src/ui/*`, `src/main.rs`, `native/Sources/Heav3nsCode/ModelPicker*.swift`)**:
   - Upgraded `Heav3nsUserConfig` with multi-slot `provider_keys` & `provider_endpoints` (`0600` permissions), added `heav3ns-micro providers`, `--reasoning`, `--wire`, `providers.list` RPC, universal `<provider>.{status,saveKey,clearKey}` credential vault RPCs, and synchronized `ModelPickerState.swift` + `ModelPicker.swift` across all canonical providers (`<= 64` catalog cap).

## Consequences

- **Positive**: Any of the 43 canonical providers (plus arbitrary custom OpenAI/Anthropic/Responses gateways) works out-of-the-box across CLI, REPL, headless stdio JSON-RPC, and the native macOS AppKit `ModelPicker` (`F3` / `⇧⌘M`) with zero cross-provider credential leakage and full KV-cache prefix stability.
- **Positive**: Interrupted turns no longer brick Anthropic sessions with dangling `tool_use` 400 errors, and strict schema validators on `/v1/responses`, DeepSeek, xAI, and Gemini accept all 31 built-in tools cleanly.
- **Trade-off**: Maintaining four wire dialects in pure Rust requires explicit unit coverage across dialect payload shapes and SSE frame variants (`src/core/provider_parity_tests.rs`).

## Verification

```bash
cargo test core::provider_parity_tests
cargo test ui::rpc_dispatch::tests
cd native && swift test --filter ModelPickerTests && cd ..
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```

