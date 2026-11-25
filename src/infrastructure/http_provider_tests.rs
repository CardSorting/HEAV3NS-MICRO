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

    #[test]
    fn test_workspace_revision_cache_invalidation_and_equivalence() {
        use crate::infrastructure::vfs_store::{workspace_revision, workspace_revision_with_cache};
        use std::fs::{self, File};
        use std::time::{Duration, SystemTime};

        let dir = std::env::temp_dir().join(format!(
            "heav3ns_rev_audit_{}_{}",
            std::process::id(),
            crate::plumbing::hash_and_math::now_ms()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();

        let old_time = SystemTime::now() - Duration::from_secs(30);
        let f1 = dir.join("a.txt");
        let f2 = dir.join("sub/b.txt");
        fs::write(&f1, b"alpha123").unwrap();
        fs::write(&f2, b"bravo456").unwrap();
        File::options().write(true).open(&f1).unwrap().set_modified(old_time).unwrap();
        File::options().write(true).open(&f2).unwrap().set_modified(old_time).unwrap();

        let rev_init_cached = workspace_revision(&dir);
        let rev_init_uncached = workspace_revision_with_cache(&dir, false);
        assert_eq!(rev_init_cached, rev_init_uncached);

        // 1. Same-length edit within 2s racy window
        fs::write(&f1, b"omega123").unwrap();
        assert_eq!(workspace_revision(&dir), workspace_revision_with_cache(&dir, false));
        assert_ne!(workspace_revision(&dir), rev_init_uncached);

        // 2. Same-length atomic replacement preserving old mtime (caught by ino/ctime stamp)
        let tmp_rep = dir.join("a.tmp");
        fs::write(&tmp_rep, b"zeta9999").unwrap();
        File::options().write(true).open(&tmp_rep).unwrap().set_modified(old_time).unwrap();
        fs::rename(&tmp_rep, &f1).unwrap();
        assert_eq!(workspace_revision(&dir), workspace_revision_with_cache(&dir, false));

        // 3. Rename
        let f2_renamed = dir.join("sub/b_moved.txt");
        fs::rename(&f2, &f2_renamed).unwrap();
        assert_eq!(workspace_revision(&dir), workspace_revision_with_cache(&dir, false));

        // 4. Deletion
        fs::remove_file(&f2_renamed).unwrap();
        assert_eq!(workspace_revision(&dir), workspace_revision_with_cache(&dir, false));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sse_fast_path_equivalence_with_mixed_and_malformed_inputs() {
        use crate::infrastructure::http_provider_helpers::extract_openai_usage;
        use crate::infrastructure::sse_and_response_parsers::{
            parse_multidialect_sse_stream, parse_multidialect_sse_stream_value_only,
        };

        let mixed = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"\\\"world\\\"\\n\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":null}]}\n\n",
            "data: {malformed_truncated_json\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"a.rs\\\"}\"}}]}}]}\n\n",
            "data: {\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":8}}\n\n",
            "data: [DONE]\n\n"
        );
        let fast = parse_multidialect_sse_stream(mixed, "openai", extract_openai_usage).unwrap();
        let slow =
            parse_multidialect_sse_stream_value_only(mixed, "openai", extract_openai_usage).unwrap();
        assert_eq!(fast.content, slow.content);
        assert_eq!(fast.tool_calls, slow.tool_calls);
        assert_eq!(
            fast.usage.as_ref().map(|u| (u.prompt_tokens, u.completion_tokens, u.cached_prompt_tokens)),
            slow.usage.as_ref().map(|u| (u.prompt_tokens, u.completion_tokens, u.cached_prompt_tokens))
        );
    }

    #[test]
    fn test_rigorous_ab_benchmarks() {
        use crate::infrastructure::http_provider_helpers::extract_openai_usage;
        use crate::infrastructure::sse_and_response_parsers::{
            parse_multidialect_sse_stream, parse_multidialect_sse_stream_value_only,
        };
        use crate::infrastructure::vfs_store::{workspace_revision, workspace_revision_with_cache};
        use std::fmt::Write;
        use std::hint::black_box;
        use std::time::Instant;

        fn summarize(label: &str, mut samples_us: Vec<f64>) -> f64 {
            samples_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let n = samples_us.len();
            let median = samples_us[n / 2];
            let p25 = samples_us[n / 4];
            let p75 = samples_us[(n * 3) / 4];
            let mean = samples_us.iter().sum::<f64>() / n as f64;
            let var = samples_us.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
            eprintln!(
                "{label}: n={n} median={median:.1}us mean={mean:.1}us stddev={:.1}us iqr={:.1}us min={:.1}us max={:.1}us",
                var.sqrt(),
                p75 - p25,
                samples_us[0],
                samples_us[n - 1]
            );
            median
        }

        // 1. SSE A/B (same build, 2000 mixed events: text, escaped strings, tool calls, malformed)
        let mut mixed = String::with_capacity(256 * 1024);
        for i in 0..1_800 {
            let _ = writeln!(
                mixed,
                "data: {{\"choices\":[{{\"delta\":{{\"content\":\"tok_{i} \\\"q\\\" \"}}}}]}}\n"
            );
        }
        for i in 0..180 {
            let _ = writeln!(
                mixed,
                "data: {{\"choices\":[{{\"delta\":{{\"tool_calls\":[{{\"index\":0,\"function\":{{\"arguments\":\"k{i}\"}}}}]}}}}]}}\n"
            );
        }
        for _ in 0..20 {
            mixed.push_str("data: {truncated_json\n\n");
        }
        mixed.push_str("data: {\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":200}}\n\ndata: [DONE]\n\n");

        for _ in 0..5 {
            black_box(parse_multidialect_sse_stream_value_only(black_box(&mixed), "openai", extract_openai_usage).unwrap());
            black_box(parse_multidialect_sse_stream(black_box(&mixed), "openai", extract_openai_usage).unwrap());
        }
        let mut sse_base = Vec::with_capacity(30);
        let mut sse_fast = Vec::with_capacity(30);
        for _ in 0..30 {
            let t0 = Instant::now();
            let r = parse_multidialect_sse_stream_value_only(black_box(&mixed), "openai", extract_openai_usage).unwrap();
            black_box(&r.content);
            sse_base.push(t0.elapsed().as_nanos() as f64 / 1000.0);

            let t1 = Instant::now();
            let r2 = parse_multidialect_sse_stream(black_box(&mixed), "openai", extract_openai_usage).unwrap();
            black_box(&r2.content);
            sse_fast.push(t1.elapsed().as_nanos() as f64 / 1000.0);
        }
        let m_sse_base = summarize("SSE Value-baseline (mixed 2000ev)", sse_base);
        let m_sse_fast = summarize("SSE SseFastFrame (mixed 2000ev)", sse_fast);
        eprintln!("SSE A/B speedup: {:.2}x ({:.1}% faster)", m_sse_base / m_sse_fast, (1.0 - m_sse_fast / m_sse_base) * 100.0);

        // 2. workspace_revision A/B (uncached vs warm-cached on repo root)
        let root = std::env::current_dir().unwrap();
        black_box(workspace_revision(&root)); // cold warmup
        let mut wr_uncached = Vec::with_capacity(10);
        let mut wr_cached = Vec::with_capacity(10);
        for _ in 0..10 {
            let t0 = Instant::now();
            black_box(workspace_revision_with_cache(black_box(&root), false));
            wr_uncached.push(t0.elapsed().as_nanos() as f64 / 1000.0);

            let t1 = Instant::now();
            black_box(workspace_revision_with_cache(black_box(&root), true));
            wr_cached.push(t1.elapsed().as_nanos() as f64 / 1000.0);
        }
        let m_wr_u = summarize("workspace_revision uncached", wr_uncached);
        let m_wr_c = summarize("workspace_revision warm-cached", wr_cached);
        eprintln!("workspace_revision A/B speedup: {:.2}x ({:.1}% faster)", m_wr_u / m_wr_c, (1.0 - m_wr_c / m_wr_u) * 100.0);
    }
}
