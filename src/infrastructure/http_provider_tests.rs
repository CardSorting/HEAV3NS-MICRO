// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::{ILlmProvider, ProgressCallback};
use crate::domain::types::messages::Role;
use crate::domain::types::progress::{EngineProgressEvent, EngineProgressPhase, EngineProgressStatus};
use crate::domain::types::runtime::CancellationToken;
use crate::domain::types::tick_result::{ProviderError, ProviderRequest};
use crate::infrastructure::http_provider::HttpProvider;
use crate::infrastructure::http_provider_helpers::{emit_stream_progress, format_stream_bytes};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_openai_payload_modern_and_reasoning_models_use_max_completion_tokens() {
        let provider = HttpProvider::new("openai", "https://api.openai.com/v1", None, 30_000);
        let req = ProviderRequest {
            model: "gpt-5.6-terra".to_string(),
            messages: vec![crate::domain::types::messages::SessionMessage::new(
                Role::User,
                "Hello",
            )],
            tools: Vec::new(),
            max_tokens: 4096,
            temperature: 0.0,
            reasoning_effort: None,
        };
        let payload = provider.build_openai_payload(&req);
        assert_eq!(payload["max_completion_tokens"], 4096);
        assert!(payload.get("max_tokens").is_none());
        assert_eq!(payload["temperature"], 0.0);

        let req_with_tools = ProviderRequest {
            model: "gpt-5.6-terra".to_string(),
            messages: vec![crate::domain::types::messages::SessionMessage::new(
                Role::User,
                "List files",
            )],
            tools: vec![serde_json::json!({
                "type": "function",
                "function": { "name": "list_files", "parameters": {} }
            })],
            max_tokens: 4096,
            temperature: 0.0,
            reasoning_effort: None,
        };
        let payload_tools = provider.build_openai_payload(&req_with_tools);
        assert_eq!(payload_tools["reasoning_effort"], "none");

        for reasoning_model in ["o1-mini", "o3-mini", "o4-mini"] {
            let req_o = ProviderRequest {
                model: reasoning_model.to_string(),
                messages: vec![crate::domain::types::messages::SessionMessage::new(
                    Role::User,
                    "Hello",
                )],
                tools: Vec::new(),
                max_tokens: 2048,
                temperature: 1.0,
                reasoning_effort: None,
            };
            let payload_o = provider.build_openai_payload(&req_o);
            assert_eq!(payload_o["max_completion_tokens"], 2048);
            assert!(payload_o.get("max_tokens").is_none());
            assert!(payload_o.get("temperature").is_none());
        }
    }

    #[test]
    fn test_build_openai_payload_legacy_models_use_max_tokens() {
        let provider = HttpProvider::new("openai", "https://api.openai.com/v1", None, 30_000);
        let req = ProviderRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![crate::domain::types::messages::SessionMessage::new(
                Role::User,
                "Hello",
            )],
            tools: Vec::new(),
            max_tokens: 1024,
            temperature: 0.0,
            reasoning_effort: None,
        };
        let payload = provider.build_openai_payload(&req);
        assert_eq!(payload["max_tokens"], 1024);
        assert!(payload.get("max_completion_tokens").is_none());
        assert_eq!(payload["temperature"], 0.0);
    }

    #[test]
    fn test_emit_stream_progress_emits_turn_scope_and_formats_bytes() {
        use std::sync::{Arc, Mutex};
        let events: Arc<Mutex<Vec<EngineProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&events);
        let cb: ProgressCallback = Arc::new(move |ev| {
            captured.lock().unwrap().push(ev);
        });
        emit_stream_progress(
            Some(&cb),
            EngineProgressPhase::Responding,
            format!("Receiving model response ({})…", format_stream_bytes(2048)),
            None,
            2,
        );
        let recorded = events.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].phase, EngineProgressPhase::Responding);
        assert_eq!(recorded[0].status, EngineProgressStatus::InProgress);
        assert_eq!(recorded[0].message, "Receiving model response (2 KB)…");
        assert_eq!(
            recorded[0].metadata.as_ref().and_then(|m| m.scope.as_deref()),
            Some("turn")
        );
        assert_eq!(format_stream_bytes(512), "512 B");
    }

    #[tokio::test]
    async fn test_complete_with_progress_honors_cancellation_and_timeout() {
        let provider = HttpProvider::new("openai", "http://127.0.0.1:9/v1", Some("sk-test".to_string()), 5_000);
        let req = ProviderRequest {
            model: "gpt-4o".to_string(),
            messages: vec![crate::domain::types::messages::SessionMessage::new(
                Role::User,
                "ping",
            )],
            tools: Vec::new(),
            max_tokens: 64,
            temperature: 0.0,
            reasoning_effort: None,
        };
        let token = CancellationToken::new();
        token.cancel();
        let err = provider
            .complete_with_progress(req, Some(token), None)
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Cancelled));
    }

    #[test]
    fn test_incremental_sse_stream_throughput_benchmark() {
        use crate::infrastructure::http_provider::parse_sse_stream;
        use std::fmt::Write;
        use std::time::Instant;

        let mut payload = String::with_capacity(512 * 1024);
        for i in 0..5_000 {
            let _ = writeln!(
                payload,
                "data: {{\"choices\":[{{\"delta\":{{\"content\":\"tok{i} \"}}}}]}}\n"
            );
        }
        payload.push_str("data: [DONE]\n\n");

        let started = Instant::now();
        let resp = parse_sse_stream(&payload, "openai").unwrap();
        let elapsed_us = started.elapsed().as_micros();
        assert!(resp.content.as_deref().unwrap_or("").contains("tok4999"));
        eprintln!(
            "SSE throughput benchmark: 5000 events ({} bytes) parsed in {} us",
            payload.len(),
            elapsed_us
        );
    }
}
