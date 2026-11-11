// [LAYER: CORE]
use crate::core::engine::AgentEngine;
use crate::core::session_store::SessionCompactor;
use crate::domain::arena::ArenaAllocator;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::spatial_mediator::SpatialActionMediator;
use crate::domain::tool_parser::ToolCallArgParser;
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::progress::{EngineProgressEvent, EngineProgressPhase, EngineProgressStatus};
use crate::domain::types::runtime::{AgentConfig, CancellationToken, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel};
use crate::domain::types::spatial_types::{SpatialActionContract, SpatialScene, SpatialTarget};
use crate::infrastructure::scripted_provider::{ProviderStep, ScriptedProvider};
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::secret_redactor::DeterministicSecretRedactor;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightTestCheck {
    pub name: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightTestScenarioResult {
    pub id: String,
    pub title: String,
    pub focus: String,
    pub passed: bool,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    #[serde(rename = "providerRequests")]
    pub provider_requests: usize,
    #[serde(rename = "toolCalls")]
    pub tool_calls: usize,
    pub checks: Vec<FlightTestCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightTestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    #[serde(rename = "passRate")]
    pub pass_rate: f64,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightTestReport {
    pub suite: String,
    #[serde(rename = "startedAt")]
    pub started_at: u64,
    #[serde(rename = "completedAt")]
    pub completed_at: u64,
    pub summary: FlightTestSummary,
    pub scenarios: Vec<FlightTestScenarioResult>,
}

pub(crate) struct TempWorkspace {
    pub(crate) path: PathBuf,
}

impl TempWorkspace {
    pub(crate) fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "heav3ns-micro-{}-{}-{}",
            prefix,
            std::process::id(),
            now_ms()
        ));
        let _ = fs::create_dir_all(&path);
        Self { path }
    }

    pub(crate) fn write_file(&self, rel: &str, content: &str) {
        let target = self.path.join(rel);
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(target, content);
    }

    pub(crate) fn read_file(&self, rel: &str) -> String {
        fs::read_to_string(self.path.join(rel)).unwrap_or_default()
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn check(name: impl Into<String>, condition: bool, detail: impl Into<String>) -> FlightTestCheck {
    FlightTestCheck {
        name: name.into(),
        passed: condition,
        detail: Some(detail.into()),
    }
}

pub(crate) fn test_config() -> AgentConfig {
    AgentConfig {
        provider: "scripted".to_string(),
        model_name: "flight-test-model".to_string(),
        fallback_model: Some("flight-test-fallback".to_string()),
        system_prompt: "You are a concise coding agent under harness test.".to_string(),
        max_turns: 20,
        max_tool_rounds: Some(8),
        context_window_tokens: 32_000,
        max_output_tokens: 2_048,
        temperature: 0.0,
        provider_timeout_ms: 5_000,
        api_endpoint: "http://127.0.0.1:0".to_string(),
        api_key: None,
        reasoning_effort: None,
        wire_dialect: None,
        fallback_providers: Vec::new(),
        ..AgentConfig::default()
    }
}

async fn run_ft01() -> FlightTestScenarioResult {
    let started = Instant::now();
    let ws = TempWorkspace::new("ft01");
    ws.write_file("src/answer.txt", "cobalt\n");

    let provider = ScriptedProvider::new(vec![
        ProviderStep::tool_call(
            "read-answer",
            "read_file",
            json!({ "path": "src/answer.txt" }),
            Some("Reading src/answer.txt.".to_string()),
        ),
        ProviderStep::text("The code word is cobalt."),
    ]);

    let engine = AgentEngine::new(&ws.path, test_config(), provider.clone());
    let mut input = EngineTickInput::new("Read src/answer.txt and tell me the code word.");
    input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
    let result = engine.tick(input).await;

    let tool_names: Vec<String> = result.tool_results.iter().map(|r| r.name.clone()).collect();
    let reply_counts = provider.tool_reply_count_by_request();
    let first_tool_msg = provider
        .tool_messages_by_request()
        .get(1)
        .and_then(|m| m.first())
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let checks = vec![
        check(
            "turn completed",
            result.outcome == EngineTickOutcome::Completed,
            format!("{:?}", result.outcome),
        ),
        check(
            "read_file executed once",
            tool_names == vec!["read_file".to_string()]
                && result.tool_results.first().map(|r| r.success).unwrap_or(false),
            format!("{:?}", tool_names),
        ),
        check(
            "second provider request received one tool message",
            reply_counts == vec![0, 1],
            format!("{:?}", reply_counts),
        ),
        check(
            "tool message contained file content",
            first_tool_msg.contains("cobalt"),
            first_tool_msg,
        ),
        check(
            "final answer used tool output",
            result.response.to_lowercase().contains("cobalt"),
            result.response.clone(),
        ),
    ];

    let passed = checks.iter().all(|c| c.passed);
    FlightTestScenarioResult {
        id: "FT-01".to_string(),
        title: "Read tool result reaches next model turn".to_string(),
        focus: "Provider tool_calls -> ToolRegistry -> tool role message -> final answer".to_string(),
        passed,
        duration_ms: started.elapsed().as_millis() as u64,
        provider_requests: provider.request_count(),
        tool_calls: result.tool_results.len(),
        checks,
        error: None,
    }
}

async fn run_ft02() -> FlightTestScenarioResult {
    let started = Instant::now();
    let ws = TempWorkspace::new("ft02");
    ws.write_file(
        "src/temperature.ts",
        "export function toFahrenheit(celsius: number): number {\n  return celsius - 32 * (5 / 9);\n}\n",
    );

    let provider = ScriptedProvider::new(vec![
        ProviderStep::tool_call(
            "read-before-edit",
            "read_file",
            json!({ "path": "src/temperature.ts" }),
            Some("Inspecting src/temperature.ts.".to_string()),
        ),
        ProviderStep::tool_call(
            "patch-formula",
            "replace_file_content",
            json!({
                "path": "src/temperature.ts",
                "target": "return celsius - 32 * (5 / 9);",
                "replacement": "return celsius * (9 / 5) + 32;"
            }),
            Some("Replacing formula.".to_string()),
        ),
        ProviderStep::tool_call(
            "read-after-edit",
            "read_file",
            json!({ "path": "src/temperature.ts" }),
            Some("Verifying updated file.".to_string()),
        ),
        ProviderStep::text("Fixed toFahrenheit so it returns celsius * (9 / 5) + 32."),
    ]);

    let engine = AgentEngine::new(&ws.path, test_config(), provider.clone());
    let mut input = EngineTickInput::new("Fix the Celsius-to-Fahrenheit conversion in src/temperature.ts.");
    input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
    let result = engine.tick(input).await;

    let updated = ws.read_file("src/temperature.ts");
    let tool_names: Vec<String> = result.tool_results.iter().map(|r| r.name.clone()).collect();
    let reply_counts = provider.tool_reply_count_by_request();

    let checks = vec![
        check(
            "turn completed",
            result.outcome == EngineTickOutcome::Completed,
            format!("{:?}", result.outcome),
        ),
        check(
            "three tool calls succeeded in order",
            tool_names == vec!["read_file", "replace_file_content", "read_file"]
                && result.tool_results.iter().all(|r| r.success),
            format!("{:?}", tool_names),
        ),
        check(
            "each model request saw cumulative tool replies",
            reply_counts == vec![0, 1, 2, 3],
            format!("{:?}", reply_counts),
        ),
        check(
            "workspace file matches repaired formula",
            updated == "export function toFahrenheit(celsius: number): number {\n  return celsius * (9 / 5) + 32;\n}\n",
            updated.trim().to_string(),
        ),
        check(
            "final answer reports the fix",
            result.response.contains("celsius * (9 / 5) + 32"),
            result.response.clone(),
        ),
    ];

    let passed = checks.iter().all(|c| c.passed);
    FlightTestScenarioResult {
        id: "FT-02".to_string(),
        title: "Read, edit, and verify file across multiple tool rounds".to_string(),
        focus: "Multi-round tool loop, file mutation, and read-after-write verification".to_string(),
        passed,
        duration_ms: started.elapsed().as_millis() as u64,
        provider_requests: provider.request_count(),
        tool_calls: result.tool_results.len(),
        checks,
        error: None,
    }
}

async fn run_ft03() -> FlightTestScenarioResult {
    let started = Instant::now();
    let ws = TempWorkspace::new("ft03");
    ws.write_file("src/note.txt", "steady-state\n");

    let provider = ScriptedProvider::new(vec![
        ProviderStep::tool_call(
            "invalid-read",
            "read_file",
            json!({}),
            Some("Attempting to read the note.".to_string()),
        ),
        ProviderStep::tool_call(
            "valid-read",
            "read_file",
            json!({ "path": "src/note.txt" }),
            Some("Retrying with path.".to_string()),
        ),
        ProviderStep::text("Recovered after validation error; the note is steady-state."),
    ]);

    let engine = AgentEngine::new(&ws.path, test_config(), provider.clone());
    let mut input = EngineTickInput::new("Read src/note.txt even if the first tool call is malformed.");
    input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
    let result = engine.tick(input).await;

    let first_error_msg = provider
        .tool_messages_by_request()
        .get(1)
        .and_then(|msgs| msgs.first())
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let parsed_error: Value = serde_json::from_str(&first_error_msg).unwrap_or(Value::Null);
    let call_ids: Vec<String> = result.tool_results.iter().filter_map(|r| r.call_id.clone()).collect();
    let successes: Vec<bool> = result.tool_results.iter().map(|r| r.success).collect();

    let checks = vec![
        check(
            "turn completed",
            result.outcome == EngineTickOutcome::Completed,
            format!("{:?}", result.outcome),
        ),
        check(
            "first tool call failed and second succeeded",
            call_ids == vec!["invalid-read", "valid-read"] && successes == vec![false, true],
            format!("callIds={:?} successes={:?}", call_ids, successes),
        ),
        check(
            "failed tool payload was returned to the model",
            parsed_error.get("success") == Some(&Value::Bool(false)),
            first_error_msg,
        ),
        check(
            "final answer includes content from recovery read",
            result.response.contains("steady-state"),
            result.response.clone(),
        ),
    ];

    let passed = checks.iter().all(|c| c.passed);
    FlightTestScenarioResult {
        id: "FT-03".to_string(),
        title: "Invalid tool arguments return an error and the turn recovers".to_string(),
        focus: "Schema validation error -> model recovery -> successful tool retry".to_string(),
        passed,
        duration_ms: started.elapsed().as_millis() as u64,
        provider_requests: provider.request_count(),
        tool_calls: result.tool_results.len(),
        checks,
        error: None,
    }
}

async fn run_ft04() -> FlightTestScenarioResult {
    let started = Instant::now();
    let ws = TempWorkspace::new("ft04");
    let provider = ScriptedProvider::new(vec![
        ProviderStep::http_error(503, "HTTP 503 Service Unavailable"),
        ProviderStep::text("Transient failure recovered on the second provider attempt."),
    ]);

    let engine = AgentEngine::new(&ws.path, test_config(), provider.clone());
    let mut input = EngineTickInput::new("Confirm the runtime retries a transient 503 error.");
    input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
    let result = engine.tick(input).await;

    let checks = vec![
        check(
            "turn completed",
            result.outcome == EngineTickOutcome::Completed,
            format!("{:?}", result.outcome),
        ),
        check(
            "provider was called twice",
            provider.request_count() == 2,
            format!("requests={}", provider.request_count()),
        ),
        check(
            "no tools were needed",
            result.tool_results.is_empty(),
            format!("tools={}", result.tool_results.len()),
        ),
        check(
            "second attempt response was returned",
            result.response.contains("Transient failure recovered"),
            result.response.clone(),
        ),
    ];

    let passed = checks.iter().all(|c| c.passed);
    FlightTestScenarioResult {
        id: "FT-04".to_string(),
        title: "Transient provider error retries before any tool runs".to_string(),
        focus: "Single automatic retry when a retryable provider error happens before tool side effects".to_string(),
        passed,
        duration_ms: started.elapsed().as_millis() as u64,
        provider_requests: provider.request_count(),
        tool_calls: result.tool_results.len(),
        checks,
        error: None,
    }
}

async fn run_ft05() -> FlightTestScenarioResult {
    let started = Instant::now();
    let ws = TempWorkspace::new("ft05");
    let provider = ScriptedProvider::new(vec![ProviderStep::WaitForAbort]);
    let engine = Arc::new(AgentEngine::new(&ws.path, test_config(), provider.clone()));
    let cancel_token = CancellationToken::new();
    let progress_events: Arc<Mutex<Vec<EngineProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));

    let events_clone = Arc::clone(&progress_events);
    let mut input = EngineTickInput::new("Wait until I cancel the turn.");
    input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
    input.cancel_token = Some(cancel_token.clone());
    input.on_progress = Some(Arc::new(move |ev| {
        events_clone.lock().unwrap().push(ev);
    }));

    let engine_task = {
        let eng = Arc::clone(&engine);
        tokio::spawn(async move { eng.tick(input).await })
    };

    provider.wait_for_request_count(1, 2_000).await;
    cancel_token.cancel();

    let result = engine_task.await.expect("FT-05 task panicked");
    let events = progress_events.lock().unwrap().clone();
    let has_completed_event = events.iter().any(|e| e.status == EngineProgressStatus::Completed);
    let cancelled_events: Vec<&EngineProgressEvent> = events
        .iter()
        .filter(|e| e.phase == EngineProgressPhase::Cancelled && e.status == EngineProgressStatus::Cancelled)
        .collect();

    let checks = vec![
        check(
            "turn outcome is cancelled",
            result.outcome == EngineTickOutcome::Cancelled,
            format!("{:?}", result.outcome),
        ),
        check(
            "response explains user cancellation",
            result.response.to_lowercase().contains("cancelled by user"),
            result.response.clone(),
        ),
        check(
            "no completed progress event was emitted",
            !has_completed_event,
            format!("events={}", events.len()),
        ),
        check(
            "single terminal cancelled progress event was emitted",
            cancelled_events.len() == 1,
            format!("cancelledEvents={}", cancelled_events.len()),
        ),
    ];

    let passed = checks.iter().all(|c| c.passed);
    FlightTestScenarioResult {
        id: "FT-05".to_string(),
        title: "Cancellation stops an in-flight provider request cleanly".to_string(),
        focus: "CancellationToken propagation and terminal cancelled outcome".to_string(),
        passed,
        duration_ms: started.elapsed().as_millis() as u64,
        provider_requests: provider.request_count(),
        tool_calls: result.tool_results.len(),
        checks,
        error: None,
    }
}

pub async fn run_agent_flight_test() -> FlightTestReport {
    let started_at = now_ms();
    let timer = Instant::now();
    let scenarios = vec![
        run_ft01().await,
        run_ft02().await,
        run_ft03().await,
        run_ft04().await,
        run_ft05().await,
    ];
    let total = scenarios.len();
    let passed = scenarios.iter().filter(|s| s.passed).count();
    let failed = total.saturating_sub(passed);
    let pass_rate = if total == 0 { 1.0 } else { passed as f64 / total as f64 };
    FlightTestReport {
        suite: "agent-flight-test-v1".to_string(),
        started_at,
        completed_at: now_ms(),
        summary: FlightTestSummary {
            total,
            passed,
            failed,
            pass_rate,
            duration_ms: timer.elapsed().as_millis() as u64,
        },
        scenarios,
    }
}

pub fn format_flight_test_report(report: &FlightTestReport) -> String {
    let mut lines = vec![format!(
        "Agent Flight Test ({}) - {}/{} passed ({:.0}%) in {}ms",
        report.suite,
        report.summary.passed,
        report.summary.total,
        report.summary.pass_rate * 100.0,
        report.summary.duration_ms
    )];
    for s in &report.scenarios {
        lines.push(format!(
            "[{}] {} {} ({}ms, requests={}, tools={})",
            if s.passed { "PASS" } else { "FAIL" },
            s.id,
            s.title,
            s.duration_ms,
            s.provider_requests,
            s.tool_calls
        ));
        for c in s.checks.iter().filter(|c| !c.passed) {
            lines.push(format!(
                "  - FAIL {}: {}",
                c.name,
                c.detail.as_deref().unwrap_or("check failed")
            ));
        }
    }
    lines.join("\n")
}

pub fn run_micro_benchmarks() -> Value {
    let t_arena = Instant::now();
    let mut arena = ArenaAllocator::new(16 * 1024 * 1024);
    for i in 0..100_000u32 {
        let _ = arena.allocate_node(i, i ^ 0x55AA);
        if arena.allocated_bytes() > 12 * 1024 * 1024 {
            arena.reset();
        }
    }
    let arena_us = t_arena.elapsed().as_micros() as u64;

    let t_parser = Instant::now();
    let parser = ToolCallArgParser::new();
    for _ in 0..10_000 {
        let _ =
            parser.parse_raw_arguments("```json\n{'file_path': 'src/main.rs', 'start_line': '10', trailing: True,}\n```");
    }
    let parser_us = t_parser.elapsed().as_micros() as u64;

    let t_redactor = Instant::now();
    for _ in 0..10_000 {
        let _ = DeterministicSecretRedactor::redact(
            "export OPENAI_API_KEY=sk-proj-1234567890abcdefghijklmnop AWS=AKIAIOSFODNN7EXAMPLE",
        );
    }
    let redactor_us = t_redactor.elapsed().as_micros() as u64;

    let t_compact = Instant::now();
    let compactor = SessionCompactor::default();
    let mut msgs = vec![SessionMessage::new(Role::System, "System prompt")];
    for i in 0..24 {
        msgs.push(SessionMessage::new(
            Role::User,
            format!("User request #{i} with context payload"),
        ));
        msgs.push(SessionMessage::new(
            Role::Assistant,
            format!("Assistant response #{i} with findings"),
        ));
    }
    let policy = CompactionPolicy {
        max_messages: Some(12),
        preserve_recent_turns: 3,
        ..Default::default()
    };
    for _ in 0..500 {
        let _ = compactor.compact(&msgs, &policy, &msgs);
    }
    let compact_us = t_compact.elapsed().as_micros() as u64;

    let t_spatial = Instant::now();
    let mut med = SpatialActionMediator::new(1440, 900, "bench-win");
    let mut scene = SpatialScene::new(1440, 900, "bench-win");
    for i in 0..50 {
        scene.targets.push(SpatialTarget::new(
            format!("el-{}", i),
            Some(i as u32),
            "button",
            format!("Element {}", i),
            [(i % 10) * 120, (i / 10) * 60, 100, 40],
        ));
    }
    med.update_scene(scene);
    let contract = SpatialActionContract {
        action_name: "click".into(),
        target_selector: Some("el-25".into()),
        ..Default::default()
    };
    for _ in 0..10_000 {
        let _ = med.ground_action(&contract);
    }
    let spatial_us = t_spatial.elapsed().as_micros() as u64;

    json!({
        "arena100kAllocsUs": arena_us,
        "parser10kRepairsUs": parser_us,
        "redactor10kScansUs": redactor_us,
        "compactor500RunsUs": compact_us,
        "spatial10kGroundingsUs": spatial_us,
    })
}
