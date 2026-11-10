// [LAYER: CORE]
use crate::core::session_store::SessionCompactor;
use crate::core::tool_registry::ToolRegistry;
use crate::domain::arena::ArenaAllocator;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::spatial_mediator::SpatialActionMediator;
use crate::domain::tool_parser::ToolCallArgParser;
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::spatial_types::{SpatialActionContract, SpatialScene, SpatialTarget};
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::infrastructure::search_tools::extract_workspace_symbols;
use crate::infrastructure::shell_runner::execute_shell_command;
use crate::plumbing::fuzzy_patch::DeterministicPatchEngine;
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::secret_redactor::DeterministicSecretRedactor;
use crate::plumbing::vision_and_dom::snapshot_html_dom;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyMetrics {
    #[serde(rename = "count")]
    pub count: usize,
    #[serde(rename = "totalDurationMs")]
    pub total_duration_ms: f64,
    #[serde(rename = "throughputOpsPerSec")]
    pub throughput_ops_per_sec: f64,
    #[serde(rename = "avgLatencyUs")]
    pub avg_latency_us: f64,
    #[serde(rename = "p50LatencyUs")]
    pub p50_latency_us: f64,
    #[serde(rename = "p95LatencyUs")]
    pub p95_latency_us: f64,
    #[serde(rename = "p99LatencyUs")]
    pub p99_latency_us: f64,
}

impl LatencyMetrics {
    pub fn compute(mut samples_us: Vec<f64>, total_duration_ms: f64) -> Self {
        if samples_us.is_empty() {
            return Self {
                count: 0,
                total_duration_ms,
                throughput_ops_per_sec: 0.0,
                avg_latency_us: 0.0,
                p50_latency_us: 0.0,
                p95_latency_us: 0.0,
                p99_latency_us: 0.0,
            };
        }
        samples_us.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = samples_us.len();
        let sum: f64 = samples_us.iter().sum();
        let avg = sum / count as f64;
        let p50 = samples_us[(count as f64 * 0.50).min((count - 1) as f64) as usize];
        let p95 = samples_us[(count as f64 * 0.95).min((count - 1) as f64) as usize];
        let p99 = samples_us[(count as f64 * 0.99).min((count - 1) as f64) as usize];
        let throughput = if total_duration_ms > 0.0 {
            (count as f64 / total_duration_ms) * 1000.0
        } else {
            0.0
        };

        Self {
            count,
            total_duration_ms,
            throughput_ops_per_sec: (throughput * 10.0).round() / 10.0,
            avg_latency_us: (avg * 100.0).round() / 100.0,
            p50_latency_us: (p50 * 100.0).round() / 100.0,
            p95_latency_us: (p95 * 100.0).round() / 100.0,
            p99_latency_us: (p99 * 100.0).round() / 100.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTaskBenchmarkResult {
    pub name: String,
    pub project: String,
    pub language: String,
    pub passed: bool,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    pub steps: Vec<String>,
    pub metrics: Value,
}

pub fn run_stress_benchmarks() -> Value {
    // 1. Arena Allocator Churn Stress (500k allocs across 50 resets)
    let t_arena_start = Instant::now();
    let mut arena = ArenaAllocator::new(16 * 1024 * 1024);
    let mut arena_samples = Vec::with_capacity(50);
    let mut total_allocs = 0usize;
    for _ in 0..50 {
        let t_sub = Instant::now();
        for j in 0..10_000u32 {
            let _ = arena.allocate_node(j, j ^ 0xCAFE);
        }
        total_allocs += 10_000;
        arena_samples.push(t_sub.elapsed().as_micros() as f64);
        arena.reset();
    }
    let arena_metrics = LatencyMetrics::compute(arena_samples, t_arena_start.elapsed().as_secs_f64() * 1000.0);

    // 2. Self-Healing Tool Call Parser Stress (25k malformed repairs)
    let t_parser_start = Instant::now();
    let parser = ToolCallArgParser::new();
    let mut parser_samples = Vec::with_capacity(250);
    let malformed_payloads = [
        "```json\n{'file_path': 'src/main.rs', 'start_line': '10', trailing: True,}\n```",
        "{\"command\": \"cargo test\", \"timeout\": 5000, /* comment */ }",
        "```\n{\"query\": \"fn main\", \"case_sensitive\": false,}\n```",
        "{action: 'click', coordinates: [100, 200], target_id: 'btn-submit',}",
        "{\"patch\": \"--- a/file\\n+++ b/file\\n@@ -1,3 +1,3 @@\\n-old\\n+new\\n\", \"dry_run\": false}",
    ];
    for chunk in 0..250 {
        let t_sub = Instant::now();
        for i in 0..100 {
            let payload = malformed_payloads[(chunk * 100 + i) % malformed_payloads.len()];
            let _ = parser.parse_raw_arguments(payload);
        }
        parser_samples.push((t_sub.elapsed().as_micros() as f64) / 100.0);
    }
    let parser_metrics = LatencyMetrics::compute(parser_samples, t_parser_start.elapsed().as_secs_f64() * 1000.0);

    // 3. Deterministic Secret Redactor Stress (25k buffer scans)
    let t_redactor_start = Instant::now();
    let mut redactor_samples = Vec::with_capacity(250);
    let secret_payloads = [
        "export OPENAI_API_KEY=sk-proj-1234567890abcdefghijklmnop AWS=AKIAIOSFODNN7EXAMPLE",
        "GITHUB_TOKEN=ghp_ABC123xyz789000000000000000000000000 and auth=Bearer eyJhbGciOi",
        "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0\n-----END RSA PRIVATE KEY-----",
        "Safe code string with let x = 42; println!(\"{}\", x); and no credentials here.",
        "curl -H 'Authorization: Bearer sk-ant-api03-abcdefg' http://internal.corp/secret",
    ];
    for chunk in 0..250 {
        let t_sub = Instant::now();
        for i in 0..100 {
            let payload = secret_payloads[(chunk * 100 + i) % secret_payloads.len()];
            let _ = DeterministicSecretRedactor::redact(payload);
        }
        redactor_samples.push((t_sub.elapsed().as_micros() as f64) / 100.0);
    }
    let redactor_metrics = LatencyMetrics::compute(redactor_samples, t_redactor_start.elapsed().as_secs_f64() * 1000.0);

    // 4. Spatial Action Grounding & Resolution Stress (25k spatial groundings)
    let t_spatial_start = Instant::now();
    let mut med = SpatialActionMediator::new(1920, 1080, "stress-win");
    let mut scene = SpatialScene::new(1920, 1080, "stress-win");
    for i in 0..100 {
        scene.targets.push(SpatialTarget::new(
            format!("el-{}", i),
            Some(i as u32),
            if i % 2 == 0 { "button" } else { "input" },
            format!("Interactive Element {}", i),
            [(i % 10) * 180, (i / 10) * 80, 150, 40],
        ));
    }
    med.update_scene(scene);
    let mut spatial_samples = Vec::with_capacity(250);
    for chunk in 0..250 {
        let t_sub = Instant::now();
        for i in 0..100 {
            let target_idx = (chunk * 100 + i) % 100;
            let contract = SpatialActionContract {
                action_name: "click".into(),
                target_selector: Some(format!("el-{}", target_idx)),
                ..Default::default()
            };
            let _ = med.ground_action(&contract);
        }
        spatial_samples.push((t_sub.elapsed().as_micros() as f64) / 100.0);
    }
    let spatial_metrics = LatencyMetrics::compute(spatial_samples, t_spatial_start.elapsed().as_secs_f64() * 1000.0);

    // 5. Session Compactor Scalability Stress (1,000 compaction cycles)
    let t_compact_start = Instant::now();
    let compactor = SessionCompactor::default();
    let mut msgs = vec![SessionMessage::new(Role::System, "System Prompt with architecture rules")];
    for i in 0..40 {
        msgs.push(SessionMessage::new(Role::User, format!("User command {i} with payload")));
        msgs.push(SessionMessage::new(Role::Assistant, format!("Assistant executed step {i} with status ok")));
    }
    let policy = CompactionPolicy {
        max_messages: Some(15),
        preserve_recent_turns: 4,
        ..Default::default()
    };
    let mut compact_samples = Vec::with_capacity(100);
    for _ in 0..100 {
        let t_sub = Instant::now();
        for _ in 0..10 {
            let _ = compactor.compact(&msgs, &policy, &msgs);
        }
        compact_samples.push((t_sub.elapsed().as_micros() as f64) / 10.0);
    }
    let compact_metrics = LatencyMetrics::compute(compact_samples, t_compact_start.elapsed().as_secs_f64() * 1000.0);

    // 6. Wave Scheduler DAG Partitioning Stress (1,000 partition evaluations)
    let t_wave_start = Instant::now();
    let mem_store = std::sync::Arc::new(std::sync::Mutex::new(crate::domain::memory::MemoryStore::new()));
    let reg = ToolRegistry::new(mem_store);
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let calls: Vec<ScheduledToolCall> = (0..30)
        .map(|i| ScheduledToolCall {
            id: format!("call-{}", i),
            name: if i % 3 == 0 { "read_file".into() } else if i % 3 == 1 { "edit_file".into() } else { "run_command".into() },
            args: json!({ "path": format!("src/file_{}.rs", i % 10) }),
        })
        .collect();
    let mut wave_samples = Vec::with_capacity(100);
    for _ in 0..100 {
        let t_sub = Instant::now();
        for _ in 0..10 {
            let _ = reg.partition_waves(&calls, &cwd);
        }
        wave_samples.push((t_sub.elapsed().as_micros() as f64) / 10.0);
    }
    let wave_metrics = LatencyMetrics::compute(wave_samples, t_wave_start.elapsed().as_secs_f64() * 1000.0);

    json!({
        "arenaAllocationStress": {
            "totalAllocations": total_allocs,
            "slabResets": 50,
            "metrics": arena_metrics
        },
        "toolParserRepairStress": {
            "totalRepairs": 25000,
            "metrics": parser_metrics
        },
        "secretRedactorStress": {
            "totalScans": 25000,
            "metrics": redactor_metrics
        },
        "spatialGroundingStress": {
            "totalGroundings": 25000,
            "metrics": spatial_metrics
        },
        "sessionCompactionStress": {
            "totalCompactions": 1000,
            "metrics": compact_metrics
        },
        "waveSchedulerPartitionStress": {
            "totalPartitions": 1000,
            "metrics": wave_metrics
        }
    })
}

pub async fn run_workspace_benchmarks(tests_dir: &Path) -> Vec<WorkspaceTaskBenchmarkResult> {
    let mut results = Vec::new();

    // -------------------------------------------------------------
    // Task 1: python-calc (Code inspection, AST, test verification)
    // -------------------------------------------------------------
    let py_dir = tests_dir.join("python-calc");
    if py_dir.exists() {
        let t_start = Instant::now();
        let mut steps = Vec::new();

        // Step 1: Read and inspect
        let core_content = fs::read_to_string(py_dir.join("calc/core.py")).unwrap_or_default();
        let has_calc_class = core_content.contains("class Calculator");
        steps.push(format!("Inspect AST/Class Calculator: {}", if has_calc_class { "FOUND" } else { "MISSING" }));

        // Step 2: Run unit tests via shell runner
        let test_res = execute_shell_command("python3 run_tests.py", &py_dir, 10_000, None).await;
        let (passed, output, exit_code, test_duration_ms) = match test_res {
            Ok((payload, _)) => {
                let code = payload.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1);
                let out = payload.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
                let err = payload.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
                let combined = format!("{out}\n{err}");
                let success = code == 0 && (combined.contains("OK") || combined.contains("Ran"));
                (success, combined, code, 0u64)
            }
            Err(e) => (false, e, -1, 0u64),
        };
        steps.push(format!("Run python3 run_tests.py: exit={exit_code}"));

        // Step 3: Run stress benchmarks if available
        let bench_script = py_dir.join("calc/benchmark.py");
        let mut bench_metrics = json!({});
        if bench_script.exists() {
            if let Ok((payload, _)) = execute_shell_command("python3 calc/benchmark.py", &py_dir, 10_000, None).await {
                let out = payload.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
                if let Ok(parsed) = serde_json::from_str::<Value>(out) {
                    bench_metrics = parsed;
                }
            }
            steps.push("Run python3 calc/benchmark.py: stress benchmarks captured".into());
        }

        let duration_ms = t_start.elapsed().as_millis() as u64;
        results.push(WorkspaceTaskBenchmarkResult {
            name: "Python Engine Inspection & Test Validation".into(),
            project: "python-calc".into(),
            language: "python".into(),
            passed,
            duration_ms,
            steps,
            metrics: json!({
                "sourceLines": core_content.lines().count(),
                "testOutput": output.trim(),
                "testDurationMs": test_duration_ms,
                "stressBenchmarks": bench_metrics
            }),
        });
    }

    // -------------------------------------------------------------
    // Task 2: rust-cli (Symbol indexing, cargo test, hashing benchmark)
    // -------------------------------------------------------------
    let rust_dir = tests_dir.join("rust-cli");
    if rust_dir.exists() {
        let t_start = Instant::now();
        let mut steps = Vec::new();

        // Step 1: Symbol indexing
        let t_sym = Instant::now();
        let target_file = rust_dir.join("src/lib.rs");
        let symbols = extract_workspace_symbols(&target_file, &rust_dir, "", None, 100);
        let sym_us = t_sym.elapsed().as_micros() as u64;
        steps.push(format!("Indexed {} symbols in {}us", symbols.len(), sym_us));

        // Step 2: Patch validation
        let original = fs::read_to_string(&target_file).unwrap_or_default();
        let patch_hunk = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -25,2 +25,7 @@\n+pub fn crc32_dummy(data: &[u8]) -> u32 {\n+    data.len() as u32\n+}\n".to_string();
        let ops = DeterministicPatchEngine::parse(&patch_hunk);
        let patch_applied = if let Some(op) = ops.first() {
            DeterministicPatchEngine::apply_hunks(&original, &op.hunks).is_ok()
        } else {
            false
        };
        steps.push(format!("Fuzzy patch engine simulation: {}", if patch_applied { "APPLIED" } else { "SKIPPED" }));

        // Step 3: Run cargo test
        let test_res = execute_shell_command("cargo test", &rust_dir, 15_000, None).await;
        let (passed, exit_code) = match test_res {
            Ok((payload, _)) => {
                let code = payload.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1);
                let out = payload.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
                let success = code == 0 && (out.contains("test result: ok") || out.contains("passed"));
                (success, code)
            }
            Err(_) => (false, -1),
        };
        steps.push(format!("Run cargo test: exit={exit_code}"));

        // Step 4: Substrate cryptographic throughput verification
        let t_hash = Instant::now();
        let payload = vec![0x5Au8; 1024 * 1024];
        let h = crate::plumbing::hash_and_math::fnv1a_32(&payload);
        let hash_us = t_hash.elapsed().as_micros();
        steps.push(format!("Substrate FNV-1a 1MB buffer hashed in {}us (checksum=0x{:08x})", hash_us, h));

        let duration_ms = t_start.elapsed().as_millis() as u64;
        results.push(WorkspaceTaskBenchmarkResult {
            name: "Rust Substrate Symbol Indexing & Compilation".into(),
            project: "rust-cli".into(),
            language: "rust".into(),
            passed,
            duration_ms,
            steps,
            metrics: json!({
                "symbolsCount": symbols.len(),
                "symbolIndexLatencyUs": sym_us,
                "cargoTestPass": passed,
                "fnv1a1MbHashUs": hash_us
            }),
        });
    }

    // -------------------------------------------------------------
    // Task 3: node-web (REST API route verification, compute benchmarks)
    // -------------------------------------------------------------
    let node_dir = tests_dir.join("node-web");
    if node_dir.exists() {
        let t_start = Instant::now();
        let mut steps = Vec::new();

        // Step 1: Read and verify package.json
        let pkg_content = fs::read_to_string(node_dir.join("package.json")).unwrap_or_default();
        let has_node_web = pkg_content.contains("node-web");
        steps.push(format!("Read package.json: {}", if has_node_web { "OK" } else { "FAIL" }));

        // Step 2: Run npm test
        let test_res = execute_shell_command("npm test", &node_dir, 10_000, None).await;
        let (passed, exit_code, stdout) = match test_res {
            Ok((payload, _)) => {
                let code = payload.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1);
                let out = payload.get("stdout").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let success = code == 0 && (out.contains("pass ") || out.contains("tests ")) && (out.contains("fail 0") || !out.contains("fail "));
                (success, code, out)
            }
            Err(e) => (false, -1, e),
        };
        steps.push(format!("Run npm test: exit={exit_code}"));

        let duration_ms = t_start.elapsed().as_millis() as u64;
        results.push(WorkspaceTaskBenchmarkResult {
            name: "Node.js REST Service Validation & Endpoint Testing".into(),
            project: "node-web".into(),
            language: "javascript".into(),
            passed,
            duration_ms,
            steps,
            metrics: json!({
                "testOutputSnippet": stdout.lines().take(4).collect::<Vec<_>>().join(" | ")
            }),
        });
    }

    // -------------------------------------------------------------
    // Task 4: frontend-kanban (DOM Spatial Grounding & Card State Lifecycle)
    // -------------------------------------------------------------
    let kanban_dir = tests_dir.join("frontend-kanban");
    if kanban_dir.exists() {
        let t_start = Instant::now();
        let mut steps = Vec::new();

        // Step 1: Snapshot HTML DOM & Extract UI targets
        let html_content = fs::read_to_string(kanban_dir.join("index.html")).unwrap_or_default();
        let dom_snap = snapshot_html_dom(&html_content, "file://index.html", 100);
        let interactive_elements = dom_snap.get("interactiveElements").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        steps.push(format!("DOM snapshot extracted {} interactive elements", interactive_elements.len()));

        // Step 2: Spatial Grounding of Add Card and Column Targets
        let mut mediator = SpatialActionMediator::new(1440, 900, "kanban-window");
        let mut scene = SpatialScene::new(1440, 900, "kanban-window");
        for (idx, elem) in interactive_elements.iter().enumerate() {
            let target_id = elem.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let tag = elem.get("tag").and_then(|v| v.as_str()).unwrap_or("div");
            let label = elem.get("label").and_then(|v| v.as_str()).unwrap_or("");
            scene.targets.push(SpatialTarget::new(
                if target_id.is_empty() { format!("elem-{}", idx) } else { target_id },
                Some(idx as u32),
                tag,
                label,
                [100 + (idx as i32 % 4) * 300, 150 + (idx as i32 / 4) * 80, 240, 60],
            ));
        }
        mediator.update_scene(scene);

        let contract_add = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-add-card".into()),
            ..Default::default()
        };
        let ground_res = mediator.ground_action(&contract_add);
        let grounded = ground_res.success;
        steps.push(format!("Spatial grounding 'btn-add-card': {}", if grounded { "GROUNDED" } else { "NOT FOUND" }));

        // Step 3: Run Kanban unit & stress tests
        let test_res = execute_shell_command("npm test", &kanban_dir, 10_000, None).await;
        let (npm_passed, exit_code) = match test_res {
            Ok((payload, _)) => {
                let code = payload.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1);
                let out = payload.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
                let success = code == 0 && (out.contains("pass ") || out.contains("tests ")) && (out.contains("fail 0") || !out.contains("fail "));
                (success, code)
            }
            Err(_) => (false, -1),
        };
        let passed = grounded && npm_passed;
        steps.push(format!("Run npm test: exit={exit_code}"));

        let duration_ms = t_start.elapsed().as_millis() as u64;
        results.push(WorkspaceTaskBenchmarkResult {
            name: "Frontend Spatial DOM Inspection & State Lifecycle".into(),
            project: "frontend-kanban".into(),
            language: "html/javascript".into(),
            passed,
            duration_ms,
            steps,
            metrics: json!({
                "domElementsCount": interactive_elements.len(),
                "spatialGroundingSuccess": grounded
            }),
        });
    }

    results
}

pub async fn run_full_benchmark_suite(tests_dir: Option<&Path>) -> Value {
    let t_total = Instant::now();
    let stress = run_stress_benchmarks();

    let resolved_tests_dir = tests_dir
        .map(|p| p.to_path_buf())
        .or_else(|| {
            std::env::var("HEAV3NS_BENCH_TESTS_DIR")
                .ok()
                .map(PathBuf::from)
                .filter(|p| p.exists())
        });

    let workspace_tasks = if let Some(ref dir) = resolved_tests_dir {
        run_workspace_benchmarks(dir).await
    } else {
        Vec::new()
    };

    let total_duration_ms = t_total.elapsed().as_millis() as u64;
    let tasks_passed = workspace_tasks.iter().filter(|t| t.passed).count();
    let tasks_total = workspace_tasks.len();

    json!({
        "runtime": "heav3ns-micro",
        "benchmarkVersion": "1.0.0",
        "timestamp": now_ms(),
        "totalDurationMs": total_duration_ms,
        "stressBenchmarks": stress,
        "workspaceTasks": {
            "total": tasks_total,
            "passed": tasks_passed,
            "passRate": if tasks_total > 0 { tasks_passed as f64 / tasks_total as f64 } else { 1.0 },
            "scenarios": workspace_tasks
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_metrics_computation() {
        let samples = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let metrics = LatencyMetrics::compute(samples, 100.0);
        assert_eq!(metrics.count, 5);
        assert_eq!(metrics.avg_latency_us, 30.0);
        assert_eq!(metrics.p50_latency_us, 30.0);
        assert!(metrics.throughput_ops_per_sec > 0.0);
    }

    #[test]
    fn test_stress_benchmarks_run() {
        let res = run_stress_benchmarks();
        assert!(res.get("arenaAllocationStress").is_some());
        assert!(res.get("toolParserRepairStress").is_some());
        assert!(res.get("secretRedactorStress").is_some());
        assert!(res.get("spatialGroundingStress").is_some());
        assert!(res.get("sessionCompactionStress").is_some());
        assert!(res.get("waveSchedulerPartitionStress").is_some());
    }

    #[tokio::test]
    async fn test_full_benchmark_suite_execution() {
        let suite = run_full_benchmark_suite(None).await;
        assert_eq!(suite["runtime"], "heav3ns-micro");
        assert!(suite["stressBenchmarks"].is_object());
    }
}
