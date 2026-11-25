// [LAYER: CORE]
#[cfg(test)]
mod tests {
    use crate::core::engine::AgentEngine;
    use crate::core::flight_runner::TempWorkspace;
    use crate::core::tool_specs::builtin_tool_specs;
    use crate::domain::interfaces::ILlmProvider;
    use crate::domain::pricing_and_taxonomy::{BillingMetering, ProviderTokenUsage};
    use crate::domain::types::messages::{Role, SessionMessage, ToolCall, ToolCallFunction};
    use crate::domain::types::provider_registry::{
        canonicalize_provider_id, lookup_provider_descriptor, ProviderChain, WireDialect,
    };
    use crate::domain::types::runtime::{AgentConfig, EngineTickInput, EngineTickOutcome};
    use crate::domain::types::tick_result::{ProviderError, ProviderRequest};
    use crate::infrastructure::http_provider::{parse_sse_stream, HttpProvider};
    
    
    use crate::infrastructure::wire_dialects::{
        build_anthropic_messages_payload, build_chat_completions_payload,
        build_google_gemini_payload, build_openai_responses_payload,
    };
    use crate::plumbing::schema_and_sse::sanitize_tool_schema_for_gemini;
    use serde_json::{json, Value};
    
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn test_provider_chain_failover_and_honest_billing_across_provider_classes() {
        let mut chain = ProviderChain::new(
            "openai",
            &[
                "codex".into(),
                "anthropic".into(),
                "deepseek".into(),
                "anthropic".into(),
            ],
        );
        assert_eq!(
            chain.providers(),
            &["openai", "openai-codex", "anthropic", "deepseek"]
        );
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
        let playground_bill = BillingMetering::evaluate(
            "apple-playground",
            "apple-foundation-model",
            "http://127.0.0.1:11434/v1",
            &usage,
        );
        assert_eq!(playground_bill, BillingMetering::LocalNoBill);

        let codex_bill = BillingMetering::evaluate(
            "openai-codex",
            "gpt-5.4",
            "https://api.openai.com/v1/responses",
            &usage,
        );
        assert_eq!(codex_bill, BillingMetering::Subscription);

        let mistral_bill = BillingMetering::evaluate(
            "mistral",
            "mistral-large-latest",
            "https://api.mistral.ai/v1",
            &usage,
        );
        assert_eq!(mistral_bill.kind_slug(), "metered");

        let unknown_bill = BillingMetering::evaluate(
            "custom",
            "proprietary-internal-v9",
            "https://llm.internal/v1",
            &usage,
        );
        assert_eq!(unknown_bill, BillingMetering::Unknown);

        assert!(lookup_provider_descriptor("nvidia-nim").is_some());
        assert_eq!(
            canonicalize_provider_id("deepseekAnthropic"),
            "deepseek-anthropic"
        );
    }

    fn assert_strict_schema_node(node: &Value, forbid_gemini_keys: bool) {
        match node {
            Value::Object(map) => {
                if forbid_gemini_keys {
                    assert!(
                        !map.contains_key("$schema"),
                        "Gemini schema leaked $schema: {node}"
                    );
                    assert!(
                        !map.contains_key("additionalProperties"),
                        "Gemini schema leaked additionalProperties: {node}"
                    );
                }
                if map.get("type").and_then(Value::as_str) == Some("array") {
                    assert!(
                        map.get("items").is_some_and(Value::is_object),
                        "Array schema missing items: {node}"
                    );
                }
                if map.get("type").and_then(Value::as_str) == Some("object") {
                    let has_props = map.contains_key("properties")
                        || map
                            .get("additionalProperties")
                            .is_some_and(Value::is_object);
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
        let anth_pl =
            build_anthropic_messages_payload("anthropic", "https://api.anthropic.com/v1", &req);
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
        for d in gem_pl["tools"][0]["functionDeclarations"]
            .as_array()
            .unwrap()
        {
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
                function: ToolCallFunction {
                    name: "read_file".into(),
                    arguments: r#"{"path":"a.txt"}"#.into(),
                },
                thought_signature: None,
            },
            ToolCall {
                id: "call_dangling".into(),
                r#type: "function".into(),
                function: ToolCallFunction {
                    name: "list_dir".into(),
                    arguments: r#"{}"#.into(),
                },
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
        assert_ne!(
            chat_msgs[1]["role"], "tool",
            "Orphaned Role::Tool must be projected to user context"
        );
        let tool_ids: Vec<&str> = chat_msgs
            .iter()
            .filter(|m| m["role"] == "tool")
            .filter_map(|m| m["tool_call_id"].as_str())
            .collect();
        assert_eq!(tool_ids, vec!["call_ok", "call_dangling"]);

        let resp_pl = build_openai_responses_payload("openai-codex", &req);
        let resp_items = resp_pl["input"].as_array().unwrap();
        let fco_ids: Vec<&str> = resp_items
            .iter()
            .filter(|i| i["type"] == "function_call_output")
            .filter_map(|i| i["call_id"].as_str())
            .collect();
        assert_eq!(fco_ids, vec!["call_ok", "call_dangling"]);

        let anth_pl =
            build_anthropic_messages_payload("anthropic", "https://api.anthropic.com/v1", &req);
        let anth_msgs = anth_pl["messages"].as_array().unwrap();
        for pair in anth_msgs.windows(2) {
            assert_ne!(
                pair[0]["role"], pair[1]["role"],
                "Anthropic roles must strictly alternate: {anth_msgs:?}"
            );
        }
        let tool_user_blocks = anth_msgs[2]["content"].as_array().unwrap();
        let first_text_pos = tool_user_blocks
            .iter()
            .position(|b| b["type"] == "text")
            .unwrap();
        let last_tr_pos = tool_user_blocks
            .iter()
            .rposition(|b| b["type"] == "tool_result")
            .unwrap();
        assert!(
            last_tr_pos < first_text_pos,
            "Anthropic tool_result blocks must precede text blocks"
        );

        // SSE in-stream errors must surface as typed HttpStatus errors, never empty 200 OK completions
        let anth_err_sse = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n";
        match parse_sse_stream(anth_err_sse, "anthropic").unwrap_err() {
            ProviderError::HttpStatus {
                status, retryable, ..
            } => {
                assert_eq!(status, 529);
                assert!(retryable);
            }
            other => panic!("Expected HttpStatus 529, got {other:?}"),
        }

        let resp_err_sse = "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"insufficient_quota\",\"message\":\"Out of credits\"}}}\n\n";
        match parse_sse_stream(resp_err_sse, "openai-codex").unwrap_err() {
            ProviderError::HttpStatus {
                status, retryable, ..
            } => {
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
        assert_eq!(
            done_resp.tool_calls[0].function.arguments,
            r#"{"path":"."}"#
        );
    }

    async fn read_http_request_head_and_body(sock: &mut tokio::net::TcpStream) -> String {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let mut header_end = None;
        let mut content_len = 0usize;
        loop {
            let Ok(n) = sock.read(&mut tmp).await else {
                break;
            };
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
                let body =
                    r#"{"error":{"code":"insufficient_quota","message":"Codex quota exhausted"}}"#;
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
                assert!(
                    raw_req.starts_with("POST /v1/messages "),
                    "Fallback must use Anthropic /v1/messages: {raw_req}"
                );
                assert!(
                    raw_req
                        .to_ascii_lowercase()
                        .contains("x-api-key: sk-ant-vault-999"),
                    "Missing vault x-api-key header"
                );
                let sse_body = if round_idx == 0 {
                    concat!(
                        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":500,\"output_tokens\":10}}}\n\n",
                        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_r0\",\"name\":\"read_file\"}}\n\n",
                        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"hello.txt\\\"}\"}}\n\n",
                        "event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":45}}\n\n"
                    )
                } else {
                    assert!(
                        raw_req.contains("world-42"),
                        "Round 1 must carry tool_result to Anthropic fallback"
                    );
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
        cfg.provider_keys
            .insert("anthropic".to_string(), "sk-ant-vault-999".to_string());
        cfg.provider_endpoints
            .insert("anthropic".to_string(), format!("http://{fb_addr}/v1"));

        let primary_http = Arc::new(
            HttpProvider::new(
                "openai-codex",
                cfg.api_endpoint.clone(),
                cfg.api_key.clone(),
                5_000,
            )
            .with_wire_dialect(WireDialect::OpenAiResponses),
        );
        let engine = AgentEngine::new(&ws.path, cfg, primary_http);
        let res = engine
            .tick(EngineTickInput::new("Read hello.txt and report its value."))
            .await;

        assert_eq!(res.outcome, EngineTickOutcome::Completed);
        assert!(res.is_fallback_model);
        assert_eq!(res.active_model, "claude-sonnet-5.5");
        assert!(res.response.contains("world-42"));
        assert_eq!(
            primary_hits.load(Ordering::SeqCst),
            1,
            "Primary 402 must fail over immediately and stay sticky on Round 1"
        );
        assert_eq!(
            fb_hits.load(Ordering::SeqCst),
            2,
            "Anthropic fallback must serve both Round 0 and Round 1"
        );

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

        let provider = HttpProvider::new(
            "openai",
            format!("http://{addr}/v1"),
            Some("sk-test".into()),
            10_000,
        );
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
            ProviderError::Transport(msg) => assert!(
                msg.contains("safety cap"),
                "Unexpected transport message: {msg}"
            ),
            other => panic!("Expected safety cap Transport error, got {other:?}"),
        }
    }
}
