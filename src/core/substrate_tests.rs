// [LAYER: CORE]
#[cfg(test)]
mod tests {
    use crate::core::engine::AgentEngine;
    use crate::core::flight_runner::{
        format_flight_test_report, run_agent_flight_test, test_config, TempWorkspace,
    };
    
    use crate::core::session_store::SessionStore;
    use crate::core::tool_registry::ToolRegistry;
    use crate::domain::arena::ArenaAllocator;
    use crate::domain::memory::MemoryStore;
    
    
    use crate::domain::swarm::SwarmAssignment;
    use crate::domain::tool_parser::ToolCallArgParser;
    use crate::domain::types::messages::{extract_text_tool_calls, Role, SessionMessage};
    use crate::domain::types::runtime::{EngineTickOutcome, ExecutionAuthorityLevel};
    use crate::domain::types::tool_schedule::ScheduledToolCall;
    use crate::domain::types::vfs_types::VfsDiskBaseline;
    use crate::infrastructure::http_provider::parse_sse_stream;
    use crate::infrastructure::scripted_provider::{ProviderStep, ScriptedProvider};
    use crate::infrastructure::vfs_store::{SessionVfs, SkillsIngestor};
    
    use crate::plumbing::json_repair::{
        accumulate_streaming_tool_name, compute_streaming_arguments_delta,
        normalize_completed_tool_call_arguments,
    };
    use crate::plumbing::secret_redactor::{DeterministicSecretRedactor, SafetyPolicy};
    
    use crate::plumbing::vision_and_dom::{
        calculate_aspect_ratio, decode_image_metadata,
        generate_synthetic_bmp, generate_synthetic_svg, redact_cdp_url, sanitize_trojan_unicode,
        snapshot_html_dom, validate_navigation_url,
    };
    use serde_json::{json, Value};
    use std::fs;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn test_agent_flight_suite_passes_100_percent() {
        let report = run_agent_flight_test().await;
        assert_eq!(
            report.summary.passed,
            report.summary.total,
            "Flight test failed:\n{}",
            format_flight_test_report(&report)
        );
    }

    #[test]
    fn test_arena_allocator_alignment_and_o1_rewind() {
        let mut arena = ArenaAllocator::new(4096);
        let (off1, len1) = arena.allocate_str("hello").unwrap();
        assert_eq!(off1 % 4, 0);
        assert_eq!(arena.read_str(off1, len1).unwrap(), "hello");

        let snap = arena.snapshot();
        let (off2, len2) = arena.allocate_str("world_extended").unwrap();
        assert_eq!(off2 % 4, 0);
        assert_eq!(arena.read_str(off2, len2).unwrap(), "world_extended");
        assert!(arena.allocated_bytes() > snap.allocated_bytes);

        arena.set_offset(snap.offset_words).unwrap();
        assert_eq!(arena.allocated_bytes(), snap.allocated_bytes);
        assert_eq!(arena.read_str(off1, len1).unwrap(), "hello");
    }

    #[test]
    fn test_self_healing_parser_and_streaming_dedup() {
        let parser = ToolCallArgParser::new();

        let dup = r#"{"path":"src/lib.rs"}{"path":"src/lib.rs"}"#;
        let normalized = normalize_completed_tool_call_arguments(dup, "test").unwrap();
        assert_eq!(normalized, r#"{"path":"src/lib.rs"}"#);

        let (d1, acc1) = compute_streaming_arguments_delta("", r#"{"path":"#);
        let (d2, acc2) = compute_streaming_arguments_delta(&acc1, r#"{"path":"src/lib.rs"}"#);
        assert_eq!(format!("{d1}{d2}"), r#"{"path":"src/lib.rs"}"#);
        assert_eq!(acc2, r#"{"path":"src/lib.rs"}"#);

        let n1 = accumulate_streaming_tool_name("", "read_file");
        let n2 = accumulate_streaming_tool_name(&n1, "read_file");
        assert_eq!(n2, "read_file");

        let broken =
            "```json\n{'file_path': 'src/main.rs', 'active': True, 'nothing': None, 'line': '42',";
        let (parsed, repaired) = parser.parse_raw_arguments(broken);
        assert!(repaired);
        assert_eq!(
            parsed.get("file_path").and_then(|v| v.as_str()),
            Some("src/main.rs")
        );
        assert_eq!(parsed.get("active").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(parsed.get("nothing"), Some(&Value::Null));

        let xml_text =
            "I will read the file now.\n<tool_call>{\"name\": \"read_file\", \"arguments\": {\"path\": \"Cargo.toml\"}}</tool_call>";
        let extracted = extract_text_tool_calls(xml_text);
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].function.name, "read_file");

        let sse = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\"}}]}}]}\n\
                   data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\\\"a.rs\\\"}\"}}]}}]}\n\
                   data: [DONE]\n";
        let resp = parse_sse_stream(sse, "openai").unwrap();
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].function.name, "read_file");
        assert_eq!(resp.tool_calls[0].function.arguments, r#"{"path":"a.rs"}"#);
    }

    #[test]
    fn test_secret_redactor_and_sensitive_path_guard() {
        let raw = "Key1=sk-ant-api03-1234567890abcdefghijklmnop Key2=AKIAIOSFODNN7EXAMPLE Key3=ghp_1234567890abcdefghijklmnopqrstuv123456";
        let redacted = DeterministicSecretRedactor::redact(raw);
        assert!(
            redacted.contains("[REDACTED:anthropic_key]"),
            "got: {redacted}"
        );
        assert!(redacted.contains("[REDACTED:aws_key]"), "got: {redacted}");
        assert!(
            redacted.contains("[REDACTED:github_token]"),
            "got: {redacted}"
        );
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));

        assert!(DeterministicSecretRedactor::is_sensitive_path(
            "/Users/test/.ssh/id_rsa"
        ));
        assert!(DeterministicSecretRedactor::is_sensitive_path(
            "/app/.aws/credentials"
        ));
        assert!(!DeterministicSecretRedactor::is_sensitive_path(
            "/app/src/main.rs"
        ));
    }

    #[test]
    fn test_vfs_baseline_verification_and_snapshot_rewind() {
        let ws = TempWorkspace::new("vfs-test");
        let file_path = ws.path.join("config.json");
        fs::write(&file_path, "{\"v\":1}\n").unwrap();

        let baseline = VfsDiskBaseline::capture(&file_path);
        let mut vfs = SessionVfs::new();
        let mut mem = MemoryStore::new();
        let mut store = SessionStore::new("s1", &ws.path, Some(65536));

        store.add_message(SessionMessage::new(Role::User, "Initial user turn"));
        mem.save_memory("k1", "v1", "fact");
        vfs.stage_write(file_path.to_str().unwrap(), "{\"v\":2}\n", Some(baseline));

        let snap = store.create_snapshot(&vfs, &mem);

        store.add_message(SessionMessage::new(Role::Assistant, "Second message"));
        mem.save_memory("k2", "v2", "rule");
        vfs.clear();

        store.rewind_to_snapshot(&snap, &mut vfs, &mut mem);
        assert_eq!(store.messages().len(), 1);
        assert_eq!(mem.list_memories().len(), 1);
        assert!(vfs.has_staged(file_path.to_str().unwrap()));

        fs::write(&file_path, "{\"v\":999}\n").unwrap();
        assert!(vfs.commit_all().is_err());

        fs::write(&file_path, "{\"v\":1}\n").unwrap();
        assert!(vfs.commit_all().is_ok());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "{\"v\":2}\n");
    }

    #[test]
    fn test_wave_scheduler_partitioning() {
        let mem = Arc::new(Mutex::new(MemoryStore::new()));
        let reg = ToolRegistry::new(mem);
        let cwd = std::env::temp_dir();

        let calls = vec![
            ScheduledToolCall {
                id: "1".to_string(),
                name: "view_file".to_string(),
                args: json!({"path": "a.rs"}),
            },
            ScheduledToolCall {
                id: "2".to_string(),
                name: "view_file".to_string(),
                args: json!({"path": "b.rs"}),
            },
            ScheduledToolCall {
                id: "3".to_string(),
                name: "write_file".to_string(),
                args: json!({"path": "a.rs", "content": "x"}),
            },
            ScheduledToolCall {
                id: "4".to_string(),
                name: "view_file".to_string(),
                args: json!({"path": "a.rs"}),
            },
        ];

        let waves = reg.partition_waves(&calls, &cwd);
        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0].len(), 2);
        assert_eq!(waves[1].len(), 1);
        assert_eq!(waves[2].len(), 1);
    }

    #[tokio::test]
    async fn test_deterministic_patch_engine_v4a_fuzzy_heal_and_symbols() {
        let ws = TempWorkspace::new("patch-suite");
        ws.write_file(
            "src/calc.rs",
            "pub struct Calculator;\n\npub fn compute_sum(a: i32, b: i32) -> i32 {\n    let res = a + b;\n    res\n}\n",
        );

        let mem = Arc::new(Mutex::new(MemoryStore::new()));
        let reg = ToolRegistry::new(mem);

        let fuzzy_call = vec![ScheduledToolCall {
            id: "c1".to_string(),
            name: "replace_file_content".to_string(),
            args: json!({
                "path": "src/calc.rs",
                "target": "pub fn compute_sum(a: i32, b: i32) -> i32 {\n  let res = a + b;\n  res\n}",
                "replacement": "pub fn compute_sum(a: i32, b: i32) -> i32 {\n    a + b\n}"
            }),
        }];
        let res1 = reg
            .execute_batch(
                &fuzzy_call,
                &ws.path,
                ExecutionAuthorityLevel::Autonomous,
                None,
            )
            .await;
        assert!(res1[0].success, "Fuzzy heal failed: {:?}", res1[0].error);
        assert!(ws.read_file("src/calc.rs").contains("    a + b\n"));

        let v4a_patch = "*** Begin Patch\n*** Update File: src/calc.rs\n@@\n pub fn compute_sum(a: i32, b: i32) -> i32 {\n-    a + b\n+    a.saturating_add(b)\n }\n*** Add File: src/extra.rs\n+pub fn greet() -> &'static str { \"hi\" }\n*** End Patch";
        let patch_call = vec![ScheduledToolCall {
            id: "c2".to_string(),
            name: "apply_patch".to_string(),
            args: json!({ "patch": v4a_patch }),
        }];
        let res2 = reg
            .execute_batch(
                &patch_call,
                &ws.path,
                ExecutionAuthorityLevel::Autonomous,
                None,
            )
            .await;
        assert!(res2[0].success, "V4A patch failed: {:?}", res2[0].error);
        assert!(ws.read_file("src/calc.rs").contains("saturating_add"));
        assert!(ws.read_file("src/extra.rs").contains("pub fn greet()"));

        let sym_call = vec![ScheduledToolCall {
            id: "c3".to_string(),
            name: "search_symbols".to_string(),
            args: json!({ "query": "compute_sum" }),
        }];
        let res3 = reg
            .execute_batch(
                &sym_call,
                &ws.path,
                ExecutionAuthorityLevel::Autonomous,
                None,
            )
            .await;
        assert!(res3[0].success);
        assert_eq!(res3[0].output["count"], 1);
    }

    #[tokio::test]
    async fn test_skills_ingestor_and_topological_swarm_execution() {
        let ws = TempWorkspace::new("skills-swarm");
        ws.write_file(
            ".agents/skills/rust-opt/SKILL.md",
            "---\nname: rust-opt\ndescription: Rust zero-allocation optimization guide\n---\n# Rust Optimization\nAlways prefer ArenaAllocator slabs.",
        );

        let skills = SkillsIngestor::discover(&ws.path);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "rust-opt");

        let provider = ScriptedProvider::new(vec![
            ProviderStep::text("Spec A analyzed module structure."),
            ProviderStep::text("Spec B verified changes using Spec A findings."),
        ]);
        let engine = AgentEngine::new(&ws.path, test_config(), provider);

        let assignments = vec![
            SwarmAssignment {
                id: "spec-a".to_string(),
                role: "Analyzer".to_string(),
                objective: "Analyze module layout".to_string(),
                depends_on: vec![],
                assigned_files: vec![],
            },
            SwarmAssignment {
                id: "spec-b".to_string(),
                role: "Verifier".to_string(),
                objective: "Verify findings".to_string(),
                depends_on: vec!["spec-a".to_string()],
                assigned_files: vec![],
            },
        ];

        let swarm_res = engine
            .run_swarm(
                "Coordinate analysis and verification",
                assignments,
                None,
                None,
            )
            .await;
        assert_eq!(swarm_res.outcome, EngineTickOutcome::Completed);
        assert_eq!(swarm_res.waves, 2);
        assert_eq!(swarm_res.reports.len(), 2);
        assert_eq!(swarm_res.reports[0].id, "spec-a");
        assert_eq!(swarm_res.reports[1].id, "spec-b");
    }

    #[tokio::test]
    async fn test_computer_use_som_undo_redo_dsl_and_browser_guard() {
        let ws = TempWorkspace::new("comp-use");
        let mem = Arc::new(Mutex::new(MemoryStore::new()));
        let reg = ToolRegistry::new(mem);

        // 1. Click element by Set-of-Marks badge som_id=1 and type into som_id=2
        let calls = vec![
            ScheduledToolCall {
                id: "cu1".into(),
                name: "computer_click".into(),
                args: json!({ "action": "click", "som_id": 1 }),
            },
            ScheduledToolCall {
                id: "cu2".into(),
                name: "computer_type".into(),
                args: json!({ "action": "set_value", "som_id": 2, "text": "/flight --strict" }),
            },
            ScheduledToolCall {
                id: "cu3".into(),
                name: "computer_search_dsl".into(),
                args: json!({ "action": "search_dsl", "query": "action:click status:success duration<10" }),
            },
            ScheduledToolCall {
                id: "cu4".into(),
                name: "computer_undo".into(),
                args: json!({ "action": "undo" }),
            },
            ScheduledToolCall {
                id: "cu5".into(),
                name: "computer_redo".into(),
                args: json!({ "action": "redo" }),
            },
        ];
        let results = reg
            .execute_batch(&calls, &ws.path, ExecutionAuthorityLevel::Autonomous, None)
            .await;
        assert!(
            results.iter().all(|r| r.success),
            "Computer use batch failed: {:?}",
            results
        );
        assert_eq!(results[0].output["resolvedElementId"], "btn-run");
        assert_eq!(results[1].output["newValue"], "/flight --strict");
        assert_eq!(results[2].output["matchCount"], 1);

        // 2. Browser SSRF guard & Set-of-Marks DOM snapshotter
        assert!(validate_navigation_url("http://169.254.169.254/latest/meta-data/").is_err());
        assert!(validate_navigation_url("file:///etc/passwd").is_err());
        assert!(validate_navigation_url("javascript:alert(1)").is_err());
        assert!(validate_navigation_url("https://heav3ns.local/app").is_ok());
        assert_eq!(
            redact_cdp_url("https://example.com/cb?token=secret123&mode=fast"),
            "https://example.com/cb?token=[REDACTED]&mode=fast"
        );

        let dom_snap = snapshot_html_dom(
            "<html><head><title>Test Portal</title><script>evil()</script></head><body><button id=\"login-btn\" aria-label=\"Sign In\">Sign In</button><a href=\"/docs\">Docs</a></body></html>",
            "https://example.com?api_key=xyz",
            32,
        );
        assert_eq!(dom_snap["title"], "Test Portal");
        assert_eq!(dom_snap["interactiveCount"], 2);
        assert_eq!(dom_snap["url"], "https://example.com?api_key=[REDACTED]");
    }

    #[tokio::test]
    async fn test_vision_codec_skill_forge_heredoc_and_legion_classifier() {
        let ws = TempWorkspace::new("vision-forge");
        let mem = Arc::new(Mutex::new(MemoryStore::new()));
        let reg = ToolRegistry::new(mem);

        // 1. Vision binary header decoder (BMP, PNG, GIF, SVG) & GCD aspect ratio
        assert_eq!(calculate_aspect_ratio(1920, 1080), "16:9");
        let bmp = generate_synthetic_bmp(64, 32, 10, 20, 30);
        let bmp_meta = decode_image_metadata(&bmp).unwrap();
        assert_eq!(bmp_meta["format"], "bmp");
        assert_eq!(bmp_meta["width"], 64);
        assert_eq!(bmp_meta["height"], 32);
        assert_eq!(bmp_meta["aspectRatio"], "2:1");

        let svg = generate_synthetic_svg(1280, 720, "Sovereign Display");
        let svg_meta = decode_image_metadata(svg.as_bytes()).unwrap();
        assert_eq!(svg_meta["format"], "svg");
        assert_eq!(svg_meta["aspectRatio"], "16:9");

        // 2. Skill Tree DAG, Custom Forge, Power-Up Pack, Multi-format Ingest & Export
        let calls = vec![
            ScheduledToolCall {
                id: "sf1".into(),
                name: "skill_forge".into(),
                args: json!({ "action": "forge", "prompt": "quantum-kernel-debugger", "category": "systems" }),
            },
            ScheduledToolCall {
                id: "sf2".into(),
                name: "skill_forge".into(),
                args: json!({ "action": "apply_pack", "id": "quantum-kernel-debugger", "pack": "tdd-enforcer" }),
            },
            ScheduledToolCall {
                id: "sf3".into(),
                name: "skill_forge".into(),
                args: json!({ "action": "export", "id": "quantum-kernel-debugger", "format": "claude_xml" }),
            },
            ScheduledToolCall {
                id: "sf4".into(),
                name: "engage_swarm".into(),
                args: json!({ "objective": "Audit architecture, harden security weakpoints, and improve UI window ergonomics" }),
            },
        ];
        let res = reg
            .execute_batch(&calls, &ws.path, ExecutionAuthorityLevel::Autonomous, None)
            .await;
        assert!(
            res.iter().all(|r| r.success),
            "Skill/Legion batch failed: {:?}",
            res
        );
        assert_eq!(res[0].output["status"], "forged");
        assert_eq!(res[1].output["status"], "power_up_applied");
        assert!(res[2].output["content"]
            .as_str()
            .unwrap_or("")
            .contains("<skill id=\"quantum-kernel-debugger\">"));
        assert_eq!(res[3].output["codename"], "THE_LEGION");
        assert!(res[3].output["specialistCount"].as_u64().unwrap_or(0) >= 3);

        // 3. ADR-0015 Heredoc & ADR-0016 Terminal Read Defenses + Trojan Unicode Sanitizer
        assert!(SafetyPolicy::detect_heredoc_file_redirection(
            "cat << 'EOF' > src/lib.rs\nfn a() {}\nEOF"
        )
        .is_some());
        assert!(SafetyPolicy::detect_heredoc_file_redirection("echo $((1 << 4))").is_none());
        assert!(
            SafetyPolicy::detect_heredoc_file_redirection("python3 - << 'PY'\nprint(1)\nPY")
                .is_none()
        );
        assert!(SafetyPolicy::detect_terminal_file_read("cat src/main.rs").is_some());
        assert!(SafetyPolicy::detect_terminal_file_read("cat src/main.rs | rg fn").is_none());
        assert_eq!(
            sanitize_trojan_unicode("safe\u{202E}text\u{200B}"),
            "safetext"
        );
    }

}
