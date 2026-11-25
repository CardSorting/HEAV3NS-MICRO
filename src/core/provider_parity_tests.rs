// [LAYER: CORE]
#[cfg(test)]
mod tests {
    use crate::core::engine::AgentEngine;
    use crate::core::flight_runner::TempWorkspace;
    
    
    
    use crate::domain::types::messages::{Role, SessionMessage, ToolCall, ToolCallFunction};
    use crate::domain::types::provider_registry::{
        canonicalize_provider_id, resolve_provider_route,
        ProviderBillingClass, ReasoningEffortLevel, WireDialect,
        PROVIDER_DESCRIPTORS,
    };
    use crate::domain::types::runtime::AgentConfig;
    use crate::domain::types::tick_result::ProviderRequest;
    use crate::infrastructure::http_provider::parse_sse_stream;
    use crate::infrastructure::scripted_provider::ScriptedProvider;
    use crate::infrastructure::sse_and_response_parsers::parse_responses_json;
    use crate::infrastructure::wire_dialects::{
        build_anthropic_messages_payload, build_chat_completions_payload,
        build_google_gemini_payload, build_openai_responses_payload,
    };
    use crate::plumbing::schema_and_sse::{
        parse_sse_events, redact_endpoint_for_display, sanitize_tool_schema, sanitize_tool_schema_for_responses,
        MAX_SSE_EVENT_DATA_BYTES,
    };
    use serde_json::{json, Value};
    use std::collections::{BTreeMap, BTreeSet};
    
    
    
    

    #[test]
    fn test_provider_registry_43_descriptors_aliases_and_secret_slot_isolation() {
        assert!(
            PROVIDER_DESCRIPTORS.len() >= 43,
            "Expected >= 43 canonical providers"
        );
        let mut seen_ids = BTreeSet::new();
        for desc in PROVIDER_DESCRIPTORS {
            assert!(
                seen_ids.insert(desc.id),
                "Duplicate provider id: {}",
                desc.id
            );
            assert!(!desc.display_name.is_empty());
            assert!(!desc.secret_slot.is_empty());
            assert!(!desc.default_endpoint.is_empty());
        }

        assert_eq!(canonicalize_provider_id("codex"), "openai-codex");
        assert_eq!(canonicalize_provider_id("claude"), "anthropic");
        assert_eq!(
            canonicalize_provider_id("claude-code"),
            "claude-subscription-directsdk-experimental"
        );
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
        for target in [
            "anthropic",
            "deepseek",
            "gemini",
            "xai",
            "openrouter",
            "ollama",
        ] {
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
        assert_eq!(
            codex_route.api_key.as_deref(),
            Some("sk-openai-shared-slot")
        );
        assert_eq!(codex_route.wire_dialect, WireDialect::OpenAiResponses);
        assert_eq!(codex_route.api_endpoint, "https://api.openai.com/v1");

        // Vault per-slot isolation across OpenAI-compatible and Anthropic-compatible sibling endpoints
        let mut vault = BTreeMap::new();
        vault.insert("deepseek".to_string(), "sk-ds-vault".to_string());
        vault.insert("anthropic".to_string(), "sk-ant-vault".to_string());
        let ds_anth = resolve_provider_route(
            "deepseek-anthropic",
            None,
            None,
            None,
            None,
            None,
            None,
            &vault,
            &empty_eps,
            |_| None,
        );
        assert_eq!(ds_anth.api_key.as_deref(), Some("sk-ds-vault"));
        assert_eq!(ds_anth.wire_dialect, WireDialect::AnthropicMessages);

        // Apple Playground loopback override via HEAV3NS_LOCAL_MODEL_URL & HEAV3NS_LOCAL_MODEL_TOKEN
        let mut apple_env = BTreeMap::new();
        apple_env.insert(
            "HEAV3NS_LOCAL_MODEL_URL".to_string(),
            "http://127.0.0.1:59123/v1".to_string(),
        );
        apple_env.insert(
            "HEAV3NS_LOCAL_MODEL_TOKEN".to_string(),
            "local-bridge-tok".to_string(),
        );
        let apple_route = resolve_provider_route(
            "apple-playground",
            None,
            None,
            None,
            None,
            None,
            None,
            &empty_keys,
            &empty_eps,
            |k| apple_env.get(k).cloned(),
        );
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
        assert_ne!(
            snap.api_key.as_deref(),
            Some("sk-openai-do-not-leak"),
            "Cross-provider switch must never carry OpenAI key"
        );
        assert_eq!(
            snap.wire_dialect, None,
            "Cross-family switch must reset stale wire_dialect override"
        );
        assert_eq!(
            engine
                .build_request_manifest("Check")
                .wire_dialect
                .as_deref(),
            Some("chat_completions")
        );

        let list_out = engine.handle_slash_command("/providers").unwrap();
        assert!(list_out.contains("openai-codex"));
        assert!(list_out.contains("deepseek"));

        let sw_out = engine
            .handle_slash_command("/provider anthropic claude-sonnet-4-6")
            .unwrap();
        assert!(sw_out.contains("anthropic"));
        let r_out = engine.handle_slash_command("/reasoning high").unwrap();
        assert!(r_out.contains("high"));

        let manifest = engine.build_request_manifest("Inspect workspace");
        assert_eq!(manifest.provider.as_deref(), Some("anthropic"));
        assert_eq!(manifest.model, "claude-sonnet-4-6");
        assert_eq!(manifest.wire_dialect.as_deref(), Some("anthropic_messages"));
        assert_eq!(
            manifest.endpoint_display.as_deref(),
            Some("https://api.anthropic.com")
        );
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
                SessionMessage::tool_result(
                    "call_resp_1",
                    "read_file",
                    "[package]\nname = \"heav3ns\"",
                ),
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
        assert_eq!(
            codex_payload["reasoning"],
            json!({"effort": "high", "summary": "auto"})
        );
        assert!(codex_payload.get("temperature").is_none());

        let input_items = codex_payload["input"].as_array().unwrap();
        assert!(input_items
            .iter()
            .any(|i| i["type"] == "function_call" && i["call_id"] == "call_resp_1"));
        assert!(input_items
            .iter()
            .any(|i| i["type"] == "function_call_output" && i["call_id"] == "call_resp_1"));

        let tool0 = &codex_payload["tools"][0];
        assert_eq!(tool0["type"], "function");
        assert_eq!(tool0["name"], "read_file");
        assert_eq!(tool0["parameters"]["properties"]["path"]["type"], "string");
        assert_eq!(
            tool0["parameters"]["properties"]["opts"]["properties"],
            json!({})
        );
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
        assert_eq!(
            parsed.tool_calls[0].function.arguments,
            r#"{"path":"src/lib.rs"}"#
        );
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
                function: ToolCallFunction {
                    name: "read_file".into(),
                    arguments: r#"{"path":"a.rs"}"#.into(),
                },
                thought_signature: None,
            },
            ToolCall {
                id: "tu_dangling".into(),
                r#type: "function".into(),
                function: ToolCallFunction {
                    name: "read_file".into(),
                    arguments: r#"{"path":"b.rs"}"#.into(),
                },
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

        let payload = build_anthropic_messages_payload(
            "anthropic",
            "https://api.anthropic.com/v1/messages",
            &req,
        );
        let sys_arr = payload["system"].as_array().unwrap();
        assert_eq!(
            sys_arr.len(),
            1,
            "Mid-transcript system messages must not pollute top-level system prefix"
        );
        assert_eq!(sys_arr[0]["text"], "Immutable Sovereign Prompt");
        assert_eq!(payload["thinking"], json!({"type": "adaptive"}));
        assert_eq!(payload["output_config"], json!({"effort": "high"}));
        assert!(
            payload.get("temperature").is_none(),
            "Anthropic thinking must omit temperature"
        );

        // Verify dangling `tu_dangling` received a synthesized error tool_result right alongside `tu_ok`,
        // AND mid-transcript system note was coalesced into the user message instead of polluting top-level `system`
        let msgs = payload["messages"].as_array().unwrap();
        let tool_res_msg = &msgs[2];
        assert_eq!(tool_res_msg["role"], "user");
        let blocks = tool_res_msg["content"].as_array().unwrap();
        assert!(blocks
            .iter()
            .any(|b| b["type"] == "tool_result" && b["tool_use_id"] == "tu_ok"));
        assert!(
            blocks.iter().any(|b| b["type"] == "tool_result"
                && b["tool_use_id"] == "tu_dangling"
                && b["is_error"] == true),
            "Missing synthesized error tool_result for dangling tool_use: {blocks:?}"
        );
        assert!(blocks.iter().any(|b| b["text"]
            .as_str()
            .unwrap_or("")
            .contains("<system_note>\nMid-turn coordinator note\n</system_note>")));

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
        assert_eq!(
            parsed.tool_calls[0].function.arguments,
            r#"{"path":"src/main.rs"}"#
        );
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
        assert_eq!(
            gemini_payload["generationConfig"]["thinkingConfig"],
            json!({"thinkingLevel": "HIGH"})
        );

        // SSE heartbeats, CRLF, multiline JSON, compact lines, and 8 MiB frame cap
        let mixed_sse = ": OPENROUTER PROCESSING\r\n\r\ndata: {\r\ndata:   \"ok\": true\r\ndata: }\r\n\r\ndata: {\"second\":2}\ndata: [DONE]\n";
        let frames = parse_sse_events(mixed_sse);
        assert_eq!(frames.len(), 2);
        assert_eq!(
            serde_json::from_str::<Value>(&frames[0].data).unwrap()["ok"],
            json!(true)
        );
        assert_eq!(
            serde_json::from_str::<Value>(&frames[1].data).unwrap()["second"],
            json!(2)
        );

        let huge_chunk = "x".repeat(MAX_SSE_EVENT_DATA_BYTES + 64);
        let overflow_sse =
            format!("data: {{\"huge\":\"{huge_chunk}\"}}\n\ndata: {{\"safe\":1}}\n\n");
        let safe_frames = parse_sse_events(&overflow_sse);
        assert_eq!(safe_frames.len(), 1);
        assert_eq!(safe_frames[0].data, r#"{"safe":1}"#);

        // Endpoint redaction strips secrets in userinfo, path, and query string
        assert_eq!(
            redact_endpoint_for_display(
                "https://user:secret@proxy.example.com:8443/v1/secret-key/chat?api_key=123#frag"
            ),
            "https://proxy.example.com:8443"
        );
        assert_eq!(
            ReasoningEffortLevel::parse_flexible("max"),
            Ok(Some(ReasoningEffortLevel::XHigh))
        );
        assert_eq!(
            ReasoningEffortLevel::parse_flexible("minimal"),
            Ok(Some(ReasoningEffortLevel::Low))
        );
        assert_eq!(
            ReasoningEffortLevel::parse_flexible("off"),
            Ok(Some(ReasoningEffortLevel::None))
        );
        assert_eq!(ReasoningEffortLevel::parse_flexible("default"), Ok(None));
    }

}
