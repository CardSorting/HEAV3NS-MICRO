// [LAYER: CORE]
#[cfg(test)]
mod tests {
    use crate::core::engine::AgentEngine;
    use crate::core::flight_runner::TempWorkspace;
    use crate::core::tool_specs::builtin_tool_specs;
    use crate::domain::interfaces::ILlmProvider;
    use crate::domain::pricing_and_taxonomy::{BillingMetering, ProviderTokenUsage};
    use crate::domain::types::messages::{Role, SessionMessage, ToolCall, ToolCallFunction};
    use crate::domain::types::provider_registry::{canonicalize_provider_id, lookup_provider_descriptor, resolve_provider_route, ProviderBillingClass, ProviderChain, ReasoningEffortLevel, WireDialect, PROVIDER_DESCRIPTORS};
    use crate::domain::types::runtime::{AgentConfig, EngineTickInput, EngineTickOutcome};
    use crate::domain::types::tick_result::{ProviderError, ProviderRequest};
    use crate::infrastructure::http_provider::{parse_sse_stream, HttpProvider};
    use crate::infrastructure::scripted_provider::ScriptedProvider;
    use crate::infrastructure::sse_and_response_parsers::parse_responses_json;
    use crate::infrastructure::wire_dialects::{build_anthropic_messages_payload, build_chat_completions_payload, build_google_gemini_payload, build_openai_responses_payload};
    use crate::plumbing::schema_and_sse::{parse_sse_events, redact_endpoint_for_display, sanitize_tool_schema, sanitize_tool_schema_for_gemini, sanitize_tool_schema_for_responses, MAX_SSE_EVENT_DATA_BYTES};
    use serde_json::{json, Value};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn test_provider_registry_43_descriptors_aliases_and_secret_slot_isolation() {
        assert!(PROVIDER_DESCRIPTORS.len() >= 43, "Expected >= 43 canonical providers");
        let mut seen_ids = BTreeSet::new();
        for desc in PROVIDER_DESCRIPTORS {
            assert!(seen_ids.insert(desc.id), "Duplicate provider id: {}", desc.id);
            assert!(!desc.display_name.is_empty());
            assert!(!desc.secret_slot.is_empty());
            assert!(!desc.default_endpoint.is_empty());
        }

        assert_eq!(canonicalize_provider_id("codex"), "openai-codex");
        assert_eq!(canonicalize_provider_id("claude"), "anthropic");
        assert_eq!(canonicalize_provider_id("claude-code"), "claude-subscription-directsdk-experimental");
        assert_eq!(canonicalize_provider_id("google"), "gemini");
        assert_eq!(canonicalize_provider_id("grok"), "xai");
        assert_eq!(canonicalize_provider_id("dashscope"), "qwen");
        assert_eq!(canonicalize_provider_id("kimi"), "moonshot");
        assert_eq!(canonicalize_provider_id("glm"), "zai");
        assert_eq!(canonicalize_provider_id("ark"), "volcengine");
        assert_eq!(canonicalize_provider_id("nim"), "nvidia-nim");
        assert_eq!(canonicalize_provider_id("hermes"), "nous");
        assert_eq!(canonicalize_provider_id("lms"), "lmstudio");
        assert_eq!(canonicalize_provider_id("clinePass"), "cline-pass");
        assert_eq!(canonicalize_provider_id("apple"), "apple-playground");

        let empty_keys = BTreeMap::new();
        let empty_eps = BTreeMap::new();

        // Cross-provider switch MUST NEVER leak inherited OpenAI key to Anthropic, DeepSeek, Gemini, or Ollama
        for target in ["anthropic", "deepseek", "gemini", "xai", "openrouter", "ollama"] {
            let route = resolve_provider_route(
                target,
                None,
                None,
                None,
                None,
                Some("openai"),
                Some("sk-openai-super-secret"),
                &empty_keys,
                &empty_eps,
                |_| None,
            );
            assert_eq!(route.api_key, None, "Leaked OpenAI key into {target}!");
        }

        // Same secret_slot family switch (openai -> openai-codex) preserves credential and upgrades wire to /v1/responses
        let codex_route = resolve_provider_route(
            "openai-codex",
            None,
            None,
            None,
            None,
            Some("openai"),
            Some("sk-openai-shared-slot"),
            &empty_keys,
            &empty_eps,
            |_| None,
        );
        assert_eq!(codex_route.api_key.as_deref(), Some("sk-openai-shared-slot"));
        assert_eq!(codex_route.wire_dialect, WireDialect::OpenAiResponses);
        assert_eq!(codex_route.api_endpoint, "https://api.openai.com/v1");

        // Vault per-slot isolation across OpenAI-compatible and Anthropic-compatible sibling endpoints
        let mut vault = BTreeMap::new();
        vault.insert("deepseek".to_string(), "sk-ds-vault".to_string());
        vault.insert("anthropic".to_string(), "sk-ant-vault".to_string());
        let ds_anth = resolve_provider_route("deepseek-anthropic", None, None, None, None, None, None, &vault, &empty_eps, |_| None);
        assert_eq!(ds_anth.api_key.as_deref(), Some("sk-ds-vault"));
        assert_eq!(ds_anth.wire_dialect, WireDialect::AnthropicMessages);

        // Apple Playground loopback override via HEAV3NS_LOCAL_MODEL_URL & HEAV3NS_LOCAL_MODEL_TOKEN
        let mut apple_env = BTreeMap::new();
        apple_env.insert("HEAV3NS_LOCAL_MODEL_URL".to_string(), "http://127.0.0.1:59123/v1".to_string());
        apple_env.insert("HEAV3NS_LOCAL_MODEL_TOKEN".to_string(), "local-bridge-tok".to_string());
        let apple_route = resolve_provider_route("apple-playground", None, None, None, None, None, None, &empty_keys, &empty_eps, |k| apple_env.get(k).cloned());
        assert_eq!(apple_route.api_endpoint, "http://127.0.0.1:59123/v1");
        assert_eq!(apple_route.api_key.as_deref(), Some("local-bridge-tok"));
        assert_eq!(apple_route.billing_class, ProviderBillingClass::LocalNoBill);
    }

    #[test]
    fn test_engine_switch_http_provider_and_slash_commands_zero_key_leakage() {
        let ws = TempWorkspace::new("provider-parity-engine");
        let mut cfg = AgentConfig::default();
        cfg.provider = "openai".to_string();
        cfg.model_name = "gpt-5.4".to_string();
        cfg.api_key = Some("sk-openai-do-not-leak".to_string());
        cfg.wire_dialect = Some("openai-responses".to_string());

        let engine = AgentEngine::new(&ws.path, cfg, ScriptedProvider::new(Vec::new()));
        // Cross-family switch ("openai" -> "xai") must clear both OpenAI API key AND stale `openai-responses` wire override
        let empty = BTreeMap::new();
        engine.switch_http_provider_with_vault("xai", "grok-4", None, None, &empty, &empty);
        let snap = engine.config_snapshot();
        assert_eq!(snap.provider, "xai");
        assert_eq!(snap.model_name, "grok-4");
        assert_ne!(snap.api_key.as_deref(), Some("sk-openai-do-not-leak"), "Cross-provider switch must never carry OpenAI key");
        assert_eq!(snap.wire_dialect, None, "Cross-family switch must reset stale wire_dialect override");
        assert_eq!(engine.build_request_manifest("Check").wire_dialect.as_deref(), Some("chat_completions"));

        let list_out = engine.handle_slash_command("/providers").unwrap();
        assert!(list_out.contains("openai-codex"));
        assert!(list_out.contains("deepseek"));

        let sw_out = engine.handle_slash_command("/provider anthropic claude-sonnet-4-6").unwrap();
        assert!(sw_out.contains("anthropic"));
        let r_out = engine.handle_slash_command("/reasoning high").unwrap();
        assert!(r_out.contains("high"));

        let manifest = engine.build_request_manifest("Inspect workspace");
        assert_eq!(manifest.provider.as_deref(), Some("anthropic"));
        assert_eq!(manifest.model, "claude-sonnet-4-6");
        assert_eq!(manifest.wire_dialect.as_deref(), Some("anthropic_messages"));
        assert_eq!(manifest.endpoint_display.as_deref(), Some("https://api.anthropic.com"));
        assert_eq!(manifest.reasoning_effort.as_deref(), Some("high"));
        let sum_bytes = manifest.system_bytes
            + manifest.tool_schema_bytes
            + manifest.history_bytes
            + manifest.json_framing_bytes;
        assert_eq!(manifest.canonical_body_bytes, sum_bytes);
    }

    #[test]
    fn test_openai_responses_wire_payload_and_multidialect_sse_parser() {
        let mut tool_call_msg = SessionMessage::new(Role::Assistant, "Let me read the file.");
        tool_call_msg.tool_calls = Some(vec![ToolCall {
            id: "call_resp_1".into(),
            r#type: "function".into(),
            function: ToolCallFunction {
                name: "read_file".into(),
                arguments: r#"{"path":"Cargo.toml"}"#.into(),
            },
            thought_signature: None,
        }]);

        let req = ProviderRequest {
            model: "gpt-5.4".into(),
            messages: vec![
                SessionMessage::new(Role::System, "Core instructions."),
                SessionMessage::new(Role::User, "Check Cargo.toml"),
                tool_call_msg,
                SessionMessage::tool_result("call_resp_1", "read_file", "[package]\nname = \"heav3ns\""),
            ],
            tools: vec![json!({
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description": "Read file",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": { "anyOf": [{"type": "string"}, {"type": "null"}] },
                            "opts": { "type": "object" }
                        },
                        "required": ["path", "nonexistent_ghost"]
                    }
                }
            })],
            temperature: 0.2,
            max_tokens: 4096,
            reasoning_effort: Some("high".into()),
        };

        let codex_payload = build_openai_responses_payload("openai-codex", &req);
        assert_eq!(codex_payload["store"], json!(false));
        assert_eq!(codex_payload["instructions"], json!("Core instructions."));
        assert_eq!(codex_payload["max_output_tokens"], json!(4096));
        assert_eq!(codex_payload["reasoning"], json!({"effort": "high", "summary": "auto"}));
        assert!(codex_payload.get("temperature").is_none());

        let input_items = codex_payload["input"].as_array().unwrap();
        assert!(input_items.iter().any(|i| i["type"] == "function_call" && i["call_id"] == "call_resp_1"));
        assert!(input_items.iter().any(|i| i["type"] == "function_call_output" && i["call_id"] == "call_resp_1"));

        let tool0 = &codex_payload["tools"][0];
        assert_eq!(tool0["type"], "function");
        assert_eq!(tool0["name"], "read_file");
        assert_eq!(tool0["parameters"]["properties"]["path"]["type"], "string");
        assert_eq!(tool0["parameters"]["properties"]["opts"]["properties"], json!({}));
        assert_eq!(tool0["parameters"]["required"], json!(["path"]));

        // Concentrate Responses profile omits `summary: "auto"`
        let conc_payload = build_openai_responses_payload("concentrate", &req);
        assert_eq!(conc_payload["reasoning"], json!({"effort": "high"}));

        // Streaming /v1/responses SSE
        let responses_sse = "\
event: response.output_text.delta\n\
data: {\"type\":\"response.output_text.delta\",\"delta\":\"Checking \"}\n\n\
event: response.output_text.delta\n\
data: {\"type\":\"response.output_text.delta\",\"delta\":\"now.\"}\n\n\
event: response.output_item.added\n\
data: {\"type\":\"response.output_item.added\",\"output_index\":1,\"item\":{\"type\":\"function_call\",\"call_id\":\"call_r99\",\"name\":\"read_file\",\"arguments\":\"\"}}\n\n\
event: response.function_call_arguments.delta\n\
data: {\"type\":\"response.function_call_arguments.delta\",\"output_index\":1,\"delta\":\"{\\\"path\\\":\\\"src/\"}\n\n\
event: response.function_call_arguments.delta\n\
data: {\"type\":\"response.function_call_arguments.delta\",\"output_index\":1,\"delta\":\"lib.rs\\\"}\"}\n\n\
event: response.completed\n\
data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":500,\"output_tokens\":80,\"input_tokens_details\":{\"cached_tokens\":320}}}}\n\n";

        let parsed = parse_sse_stream(responses_sse, "openai-codex").unwrap();
        assert_eq!(parsed.content.as_deref(), Some("Checking now."));
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].id, "call_r99");
        assert_eq!(parsed.tool_calls[0].function.name, "read_file");
        assert_eq!(parsed.tool_calls[0].function.arguments, r#"{"path":"src/lib.rs"}"#);
        let usage = parsed.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 500);
        assert_eq!(usage.completion_tokens, 80);
        assert_eq!(usage.cached_prompt_tokens, 320);

        // Non-streaming /v1/responses JSON
        let non_stream = json!({
            "status": "completed",
            "output": [
                { "type": "message", "content": [{ "type": "output_text", "text": "Done!" }] },
                { "type": "function_call", "call_id": "fc_1", "name": "list_dir", "arguments": "{\"path\":\".\"}" }
            ],
            "usage": { "input_tokens": 100, "output_tokens": 25 }
        });
        let parsed_json = parse_responses_json(&non_stream, "openai-codex").unwrap();
        assert_eq!(parsed_json.content.as_deref(), Some("Done!"));
        assert_eq!(parsed_json.tool_calls.len(), 1);
        assert_eq!(parsed_json.tool_calls[0].id, "fc_1");
    }

    #[test]
    fn test_anthropic_messages_dangling_tool_use_repair_mid_system_and_sse() {
        let mut assistant_msg = SessionMessage::new(Role::Assistant, "Running two tools.");
        assistant_msg.tool_calls = Some(vec![
            ToolCall {
                id: "tu_ok".into(),
                r#type: "function".into(),
                function: ToolCallFunction { name: "read_file".into(), arguments: r#"{"path":"a.rs"}"#.into() },
                thought_signature: None,
            },
            ToolCall {
                id: "tu_dangling".into(),
                r#type: "function".into(),
                function: ToolCallFunction { name: "read_file".into(), arguments: r#"{"path":"b.rs"}"#.into() },
                thought_signature: None,
            },
        ]);

        let req = ProviderRequest {
            model: "claude-sonnet-4.6".into(),
            messages: vec![
                SessionMessage::new(Role::System, "Immutable Sovereign Prompt"),
                SessionMessage::new(Role::User, "Read a.rs and b.rs"),
                assistant_msg,
                SessionMessage::tool_result("tu_ok", "read_file", "fn a() {}"),
                SessionMessage::new(Role::System, "Mid-turn coordinator note"),
                SessionMessage::new(Role::User, "Proceed"),
            ],
            tools: Vec::new(),
            temperature: 0.4,
            max_tokens: 2048,
            reasoning_effort: Some("high".into()),
        };

        let payload = build_anthropic_messages_payload("anthropic", "https://api.anthropic.com/v1/messages", &req);
        let sys_arr = payload["system"].as_array().unwrap();
        assert_eq!(sys_arr.len(), 1, "Mid-transcript system messages must not pollute top-level system prefix");
        assert_eq!(sys_arr[0]["text"], "Immutable Sovereign Prompt");
        assert_eq!(payload["thinking"], json!({"type": "adaptive"}));
        assert_eq!(payload["output_config"], json!({"effort": "high"}));
        assert!(payload.get("temperature").is_none(), "Anthropic thinking must omit temperature");

        // Verify dangling `tu_dangling` received a synthesized error tool_result right alongside `tu_ok`,
        // AND mid-transcript system note was coalesced into the user message instead of polluting top-level `system`
        let msgs = payload["messages"].as_array().unwrap();
        let tool_res_msg = &msgs[2];
        assert_eq!(tool_res_msg["role"], "user");
        let blocks = tool_res_msg["content"].as_array().unwrap();
        assert!(blocks.iter().any(|b| b["type"] == "tool_result" && b["tool_use_id"] == "tu_ok"));
        assert!(
            blocks.iter().any(|b| b["type"] == "tool_result" && b["tool_use_id"] == "tu_dangling" && b["is_error"] == true),
            "Missing synthesized error tool_result for dangling tool_use: {blocks:?}"
        );
        assert!(blocks.iter().any(|b| b["text"].as_str().unwrap_or("").contains("<system_note>\nMid-turn coordinator note\n</system_note>")));

        // Verify Anthropic SSE streaming with input_json_delta + cache_read_input_tokens
        let anthropic_sse = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":120,\"cache_read_input_tokens\":880,\"output_tokens\":1}}}\n\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Let's patch.\"}}\n\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_99\",\"name\":\"read_file\"}}\n\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\n\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"src/main.rs\\\"}\"}}\n\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":42}}\n\n";

        let parsed = parse_sse_stream(anthropic_sse, "anthropic").unwrap();
        assert_eq!(parsed.content.as_deref(), Some("Let's patch."));
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].id, "toolu_99");
        assert_eq!(parsed.tool_calls[0].function.name, "read_file");
        assert_eq!(parsed.tool_calls[0].function.arguments, r#"{"path":"src/main.rs"}"#);
        let u = parsed.usage.unwrap();
        assert_eq!(u.prompt_tokens, 1000);
        assert_eq!(u.cached_prompt_tokens, 880);
        assert_eq!(u.completion_tokens, 42);
    }

    #[test]
    fn test_schema_sanitizer_dialects_and_sse_hardening_edge_cases() {
        let mut union_root = json!({
            "oneOf": [
                {
                    "type": "object",
                    "properties": {
                        "query": { "anyOf": [{"type": "string"}, {"type": "null"}] }
                    },
                    "required": ["query", "stale_field"]
                },
                {
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer" }
                    },
                    "required": ["limit"]
                }
            ]
        });
        let mut chat_copy = union_root.clone();
        sanitize_tool_schema(&mut chat_copy);
        assert!(chat_copy.get("oneOf").is_some());

        sanitize_tool_schema_for_responses(&mut union_root);
        assert_eq!(union_root["type"], "object");
        assert_eq!(union_root["properties"]["query"]["type"], "string");
        assert_eq!(union_root["properties"]["limit"]["type"], "integer");

        // Gemini & Z.ai dialect builders shape thinking configs properly
        let simple_req = ProviderRequest {
            model: "glm-5".into(),
            messages: vec![SessionMessage::new(Role::User, "Hi")],
            tools: Vec::new(),
            temperature: 0.2,
            max_tokens: 512,
            reasoning_effort: Some("high".into()),
        };
        let zai_payload = build_chat_completions_payload("zai", &simple_req);
        assert_eq!(zai_payload["thinking"], json!({"type": "enabled"}));

        let mut gemini_req = simple_req.clone();
        gemini_req.model = "gemini-2.5-pro".into();
        let gemini_payload = build_google_gemini_payload(&gemini_req);
        assert_eq!(gemini_payload["generationConfig"]["thinkingConfig"], json!({"thinkingLevel": "HIGH"}));

        // SSE heartbeats, CRLF, multiline JSON, compact lines, and 8 MiB frame cap
        let mixed_sse = ": OPENROUTER PROCESSING\r\n\r\ndata: {\r\ndata:   \"ok\": true\r\ndata: }\r\n\r\ndata: {\"second\":2}\ndata: [DONE]\n";
        let frames = parse_sse_events(mixed_sse);
        assert_eq!(frames.len(), 2);
        assert_eq!(serde_json::from_str::<Value>(&frames[0].data).unwrap()["ok"], json!(true));
        assert_eq!(serde_json::from_str::<Value>(&frames[1].data).unwrap()["second"], json!(2));

        let huge_chunk = "x".repeat(MAX_SSE_EVENT_DATA_BYTES + 64);
        let overflow_sse = format!("data: {{\"huge\":\"{huge_chunk}\"}}\n\ndata: {{\"safe\":1}}\n\n");
        let safe_frames = parse_sse_events(&overflow_sse);
        assert_eq!(safe_frames.len(), 1);
        assert_eq!(safe_frames[0].data, r#"{"safe":1}"#);

        // Endpoint redaction strips secrets in userinfo, path, and query string
        assert_eq!(
            redact_endpoint_for_display("https://user:secret@proxy.example.com:8443/v1/secret-key/chat?api_key=123#frag"),
            "https://proxy.example.com:8443"
        );
        assert_eq!(ReasoningEffortLevel::parse_flexible("max"), Ok(Some(ReasoningEffortLevel::XHigh)));
        assert_eq!(ReasoningEffortLevel::parse_flexible("minimal"), Ok(Some(ReasoningEffortLevel::Low)));
        assert_eq!(ReasoningEffortLevel::parse_flexible("off"), Ok(Some(ReasoningEffortLevel::None)));
        assert_eq!(ReasoningEffortLevel::parse_flexible("default"), Ok(None));
    }

    #[test]
    fn test_provider_chain_failover_and_honest_billing_across_provider_classes() {
        let mut chain = ProviderChain::new(
            "openai",
            &["codex".into(), "anthropic".into(), "deepseek".into(), "anthropic".into()],
        );
        assert_eq!(chain.providers(), &["openai", "openai-codex", "anthropic", "deepseek"]);
        assert_eq!(chain.advance(), Some("openai-codex"));
        assert!(chain.is_fallback_active());
        assert_eq!(chain.advance(), Some("anthropic"));
        chain.reset();
        assert_eq!(chain.current(), "openai");

        let usage = ProviderTokenUsage {
            prompt_tokens: 10_000,
            completion_tokens: 2_000,
            cached_prompt_tokens: 5_000,
            is_exact: true,
        };
        let playground_bill = BillingMetering::evaluate("apple-playground", "apple-foundation-model", "http://127.0.0.1:11434/v1", &usage);
        assert_eq!(playground_bill, BillingMetering::LocalNoBill);

        let codex_bill = BillingMetering::evaluate("openai-codex", "gpt-5.4", "https://api.openai.com/v1/responses", &usage);
        assert_eq!(codex_bill, BillingMetering::Subscription);

        let mistral_bill = BillingMetering::evaluate("mistral", "mistral-large-latest", "https://api.mistral.ai/v1", &usage);
        assert_eq!(mistral_bill.kind_slug(), "metered");

        let unknown_bill = BillingMetering::evaluate("custom", "proprietary-internal-v9", "https://llm.internal/v1", &usage);
        assert_eq!(unknown_bill, BillingMetering::Unknown);

        assert!(lookup_provider_descriptor("nvidia-nim").is_some());
        assert_eq!(canonicalize_provider_id("deepseekAnthropic"), "deepseek-anthropic");
    }

    fn assert_strict_schema_node(node: &Value, forbid_gemini_keys: bool) {
        match node {
            Value::Object(map) => {
                if forbid_gemini_keys {
                    assert!(!map.contains_key("$schema"), "Gemini schema leaked $schema: {node}");
                    assert!(!map.contains_key("additionalProperties"), "Gemini schema leaked additionalProperties: {node}");
                }
                if map.get("type").and_then(Value::as_str) == Some("array") {
                    assert!(map.get("items").is_some_and(Value::is_object), "Array schema missing items: {node}");
                }
                if map.get("type").and_then(Value::as_str) == Some("object") {
                    let has_props = map.contains_key("properties")
                        || map.get("additionalProperties").is_some_and(Value::is_object);
                    assert!(has_props, "Object schema missing properties: {node}");
                }
                for child in map.values() {
                    assert_strict_schema_node(child, forbid_gemini_keys);
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    assert_strict_schema_node(item, forbid_gemini_keys);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn test_all_31_builtin_tool_schemas_pass_strict_array_items_and_gemini_sanitization() {
        let specs = builtin_tool_specs();
        assert!(specs.len() >= 31);
        let mut tools: Vec<Value> = specs.iter().map(|s| s.to_openai_tool()).collect();
        tools.push(json!({
            "type": "function",
            "function": {
                "name": "mcp_nested_probe",
                "description": "External MCP schema with nullable array, bare array, and nested additionalProperties",
                "parameters": {
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "tags": { "type": "array", "description": "Tag list" },
                        "label": { "type": ["string", "null"] },
                        "meta": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": { "k": { "type": "string" } }
                        }
                    }
                }
            }
        }));

        let req = ProviderRequest {
            model: "gpt-5.4".into(),
            messages: vec![SessionMessage::new(Role::User, "Inspect")],
            tools,
            max_tokens: 1024,
            temperature: 0.0,
            reasoning_effort: None,
        };

        let chat_pl = build_chat_completions_payload("openai", &req);
        let resp_pl = build_openai_responses_payload("openai-codex", &req);
        let anth_pl = build_anthropic_messages_payload("anthropic", "https://api.anthropic.com/v1", &req);
        let gem_pl = build_google_gemini_payload(&req);

        for t in chat_pl["tools"].as_array().unwrap() {
            assert_strict_schema_node(&t["function"]["parameters"], false);
        }
        for t in resp_pl["tools"].as_array().unwrap() {
            assert_strict_schema_node(&t["parameters"], false);
        }
        for t in anth_pl["tools"].as_array().unwrap() {
            assert_strict_schema_node(&t["input_schema"], false);
        }
        for d in gem_pl["tools"][0]["functionDeclarations"].as_array().unwrap() {
            assert_strict_schema_node(&d["parameters"], true);
        }

        let mut direct_gem = json!({"type": ["array", "null"], "additionalProperties": false});
        sanitize_tool_schema_for_gemini(&mut direct_gem);
        assert_strict_schema_node(&direct_gem, true);
    }

    #[test]
    fn test_bidirectional_tool_continuity_and_sse_in_stream_error_classification() {
        let mut assistant_calls = SessionMessage::new(Role::Assistant, "Running two tools");
        assistant_calls.tool_calls = Some(vec![
            ToolCall {
                id: "call_ok".into(),
                r#type: "function".into(),
                function: ToolCallFunction { name: "read_file".into(), arguments: r#"{"path":"a.txt"}"#.into() },
                thought_signature: None,
            },
            ToolCall {
                id: "call_dangling".into(),
                r#type: "function".into(),
                function: ToolCallFunction { name: "list_dir".into(), arguments: r#"{}"#.into() },
                thought_signature: None,
            },
        ]);

        let req = ProviderRequest {
            model: "claude-sonnet-4-6".into(),
            messages: vec![
                SessionMessage::new(Role::System, "Top system"),
                SessionMessage::tool_result("orphan_rewind_99", "read_file", "stale rewind output"),
                SessionMessage::new(Role::User, "Fix build"),
                assistant_calls,
                SessionMessage::new(Role::System, "Mid-round budget advisory"),
                SessionMessage::tool_result("call_ok", "read_file", "contents of a.txt"),
                SessionMessage::new(Role::Assistant, "First summary"),
                SessionMessage::new(Role::Assistant, "Coalesced continuation"),
            ],
            tools: Vec::new(),
            max_tokens: 4096,
            temperature: 0.0,
            reasoning_effort: None,
        };

        let chat_pl = build_chat_completions_payload("openai", &req);
        let chat_msgs = chat_pl["messages"].as_array().unwrap();
        assert_ne!(chat_msgs[1]["role"], "tool", "Orphaned Role::Tool must be projected to user context");
        let tool_ids: Vec<&str> = chat_msgs.iter().filter(|m| m["role"] == "tool").filter_map(|m| m["tool_call_id"].as_str()).collect();
        assert_eq!(tool_ids, vec!["call_ok", "call_dangling"]);

        let resp_pl = build_openai_responses_payload("openai-codex", &req);
        let resp_items = resp_pl["input"].as_array().unwrap();
        let fco_ids: Vec<&str> = resp_items.iter().filter(|i| i["type"] == "function_call_output").filter_map(|i| i["call_id"].as_str()).collect();
        assert_eq!(fco_ids, vec!["call_ok", "call_dangling"]);

        let anth_pl = build_anthropic_messages_payload("anthropic", "https://api.anthropic.com/v1", &req);
        let anth_msgs = anth_pl["messages"].as_array().unwrap();
        for pair in anth_msgs.windows(2) {
            assert_ne!(pair[0]["role"], pair[1]["role"], "Anthropic roles must strictly alternate: {anth_msgs:?}");
        }
        let tool_user_blocks = anth_msgs[2]["content"].as_array().unwrap();
        let first_text_pos = tool_user_blocks.iter().position(|b| b["type"] == "text").unwrap();
        let last_tr_pos = tool_user_blocks.iter().rposition(|b| b["type"] == "tool_result").unwrap();
        assert!(last_tr_pos < first_text_pos, "Anthropic tool_result blocks must precede text blocks");

        // SSE in-stream errors must surface as typed HttpStatus errors, never empty 200 OK completions
        let anth_err_sse = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n";
        match parse_sse_stream(anth_err_sse, "anthropic").unwrap_err() {
            ProviderError::HttpStatus { status, retryable, .. } => {
                assert_eq!(status, 529);
                assert!(retryable);
            }
            other => panic!("Expected HttpStatus 529, got {other:?}"),
        }

        let resp_err_sse = "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"insufficient_quota\",\"message\":\"Out of credits\"}}}\n\n";
        match parse_sse_stream(resp_err_sse, "openai-codex").unwrap_err() {
            ProviderError::HttpStatus { status, retryable, .. } => {
                assert_eq!(status, 402);
                assert!(!retryable);
            }
            other => panic!("Expected HttpStatus 402, got {other:?}"),
        }

        // Gemini thought parts are excluded from visible assistant reply
        let gem_sse = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"internal reasoning\",\"thought\":true},{\"text\":\"Final clean answer\"}]}}]}\n\n";
        let gem_resp = parse_sse_stream(gem_sse, "gemini").unwrap();
        assert_eq!(gem_resp.content.as_deref(), Some("Final clean answer"));

        // Responses function_call_arguments.done populates arguments when delta was omitted
        let done_only_sse = concat!(
            "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"fc_9\",\"call_id\":\"call_9\",\"type\":\"function_call\",\"name\":\"list_dir\"}}\n\n",
            "data: {\"type\":\"response.function_call_arguments.done\",\"output_index\":0,\"arguments\":\"{\\\"path\\\":\\\".\\\"}\"}\n\n"
        );
        let done_resp = parse_sse_stream(done_only_sse, "openai-codex").unwrap();
        assert_eq!(done_resp.tool_calls.len(), 1);
        assert_eq!(done_resp.tool_calls[0].function.arguments, r#"{"path":"."}"#);
    }

    async fn read_http_request_head_and_body(sock: &mut tokio::net::TcpStream) -> String {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let mut header_end = None;
        let mut content_len = 0usize;
        loop {
            let Ok(n) = sock.read(&mut tmp).await else { break };
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if header_end.is_none() {
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let end = pos + 4;
                    header_end = Some(end);
                    let hdr_str = String::from_utf8_lossy(&buf[..end]).to_ascii_lowercase();
                    for line in hdr_str.lines() {
                        if let Some(rest) = line.strip_prefix("content-length:") {
                            content_len = rest.trim().parse::<usize>().unwrap_or(0);
                        }
                    }
                }
            }
            if let Some(end) = header_end {
                if buf.len() >= end + content_len {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    #[tokio::test]
    async fn test_live_tcp_multi_round_mixed_dialect_failover_and_honest_receipt() {
        let primary_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let primary_addr = primary_listener.local_addr().unwrap();
        let primary_hits = Arc::new(AtomicUsize::new(0));
        let primary_hits_srv = Arc::clone(&primary_hits);

        tokio::spawn(async move {
            while let Ok((mut sock, _)) = primary_listener.accept().await {
                let _ = read_http_request_head_and_body(&mut sock).await;
                primary_hits_srv.fetch_add(1, Ordering::SeqCst);
                let body = r#"{"error":{"code":"insufficient_quota","message":"Codex quota exhausted"}}"#;
                let resp = format!(
                    "HTTP/1.1 402 Payment Required\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });

        let fb_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let fb_addr = fb_listener.local_addr().unwrap();
        let fb_hits = Arc::new(AtomicUsize::new(0));
        let fb_hits_srv = Arc::clone(&fb_hits);

        tokio::spawn(async move {
            while let Ok((mut sock, _)) = fb_listener.accept().await {
                let raw_req = read_http_request_head_and_body(&mut sock).await;
                let round_idx = fb_hits_srv.fetch_add(1, Ordering::SeqCst);
                assert!(raw_req.starts_with("POST /v1/messages "), "Fallback must use Anthropic /v1/messages: {raw_req}");
                assert!(raw_req.to_ascii_lowercase().contains("x-api-key: sk-ant-vault-999"), "Missing vault x-api-key header");
                let sse_body = if round_idx == 0 {
                    concat!(
                        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":500,\"output_tokens\":10}}}\n\n",
                        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_r0\",\"name\":\"read_file\"}}\n\n",
                        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"hello.txt\\\"}\"}}\n\n",
                        "event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":45}}\n\n"
                    )
                } else {
                    assert!(raw_req.contains("world-42"), "Round 1 must carry tool_result to Anthropic fallback");
                    concat!(
                        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":700,\"output_tokens\":20}}}\n\n",
                        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Verified fallback read: world-42\"}}\n\n",
                        "event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":30}}\n\n"
                    )
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    sse_body.len(),
                    sse_body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });

        let ws = TempWorkspace::new("live-tcp-failover");
        ws.write_file("hello.txt", "world-42\n");

        let mut cfg = AgentConfig::default();
        cfg.provider = "openai-codex".to_string();
        cfg.model_name = "gpt-5.4".to_string();
        cfg.api_endpoint = format!("http://{primary_addr}/v1");
        cfg.api_key = Some("sk-codex-primary".to_string());
        cfg.fallback_providers = vec!["anthropic:claude-sonnet-5.5".to_string()];
        cfg.provider_keys.insert("anthropic".to_string(), "sk-ant-vault-999".to_string());
        cfg.provider_endpoints.insert("anthropic".to_string(), format!("http://{fb_addr}/v1"));

        let primary_http = Arc::new(
            HttpProvider::new("openai-codex", cfg.api_endpoint.clone(), cfg.api_key.clone(), 5_000)
                .with_wire_dialect(WireDialect::OpenAiResponses),
        );
        let engine = AgentEngine::new(&ws.path, cfg, primary_http);
        let res = engine.tick(EngineTickInput::new("Read hello.txt and report its value.")).await;

        assert_eq!(res.outcome, EngineTickOutcome::Completed);
        assert!(res.is_fallback_model);
        assert_eq!(res.active_model, "claude-sonnet-5.5");
        assert!(res.response.contains("world-42"));
        assert_eq!(primary_hits.load(Ordering::SeqCst), 1, "Primary 402 must fail over immediately and stay sticky on Round 1");
        assert_eq!(fb_hits.load(Ordering::SeqCst), 2, "Anthropic fallback must serve both Round 0 and Round 1");

        let receipt = res.receipt.expect("TurnReceipt must be persisted");
        assert_eq!(receipt.provider, "anthropic");
        assert_eq!(receipt.billing_kind, "metered");
    }

    #[tokio::test]
    async fn test_live_tcp_oversized_streaming_body_aborts_at_16mib_cap() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let _ = read_http_request_head_and_body(&mut sock).await;
                let _ = sock
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n")
                    .await;
                let chunk = vec![b'x'; 64 * 1024];
                // Attempt to stream 20 MiB (> 16 MiB cap); client should abort mid-stream
                for _ in 0..320 {
                    if sock.write_all(&chunk).await.is_err() {
                        break;
                    }
                }
            }
        });

        let provider = HttpProvider::new("openai", format!("http://{addr}/v1"), Some("sk-test".into()), 10_000);
        let req = ProviderRequest {
            model: "gpt-5.4".into(),
            messages: vec![SessionMessage::new(Role::User, "Ping")],
            tools: Vec::new(),
            max_tokens: 128,
            temperature: 0.0,
            reasoning_effort: None,
        };
        let err = provider.complete(req, None).await.unwrap_err();
        match err {
            ProviderError::Transport(msg) => assert!(msg.contains("safety cap"), "Unexpected transport message: {msg}"),
            other => panic!("Expected safety cap Transport error, got {other:?}"),
        }
    }
}
