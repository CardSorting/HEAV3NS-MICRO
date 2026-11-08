// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::core::flight_runner::run_agent_flight_test;
use crate::domain::swarm::SwarmAssignment;
use crate::domain::types::runtime::{CancellationToken, EngineTickInput, ExecutionAuthorityLevel};
use crate::plumbing::hash_and_math::now_ms;
use serde_json::{json, Value};
use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CliOptions {
    pub mode: RunMode,
    pub format: String,
    pub output: Option<PathBuf>,
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub endpoint: Option<String>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    FlightTest,
    Bench,
    Smoke,
    StdioWorker,
    SinglePrompt,
    Interactive,
    Help,
}

pub fn parse_args() -> CliOptions {
    let mut opts = CliOptions {
        mode: RunMode::Interactive,
        format: "text".to_string(),
        output: None,
        cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        model: None,
        provider: None,
        endpoint: None,
        prompt: None,
    };

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--flight" | "--benchmark-flight" => opts.mode = RunMode::FlightTest,
            "--bench" => opts.mode = RunMode::Bench,
            "--smoke" => opts.mode = RunMode::Smoke,
            "--stdio" | "--worker" => opts.mode = RunMode::StdioWorker,
            "-h" | "--help" => opts.mode = RunMode::Help,
            "--json" => opts.format = "json".to_string(),
            "--format" => {
                if let Some(v) = args.get(i + 1) {
                    opts.format = v.clone();
                    i += 1;
                }
            }
            "--output" | "-o" => {
                if let Some(v) = args.get(i + 1) {
                    opts.output = Some(PathBuf::from(v));
                    i += 1;
                }
            }
            "--cwd" => {
                if let Some(v) = args.get(i + 1) {
                    opts.cwd = PathBuf::from(v);
                    i += 1;
                }
            }
            "--model" | "-m" => {
                if let Some(v) = args.get(i + 1) {
                    opts.model = Some(v.clone());
                    i += 1;
                }
            }
            "--provider" => {
                if let Some(v) = args.get(i + 1) {
                    opts.provider = Some(v.clone());
                    i += 1;
                }
            }
            "--endpoint" => {
                if let Some(v) = args.get(i + 1) {
                    opts.endpoint = Some(v.clone());
                    i += 1;
                }
            }
            "--prompt" | "-p" => {
                if let Some(v) = args.get(i + 1) {
                    opts.prompt = Some(v.clone());
                    opts.mode = RunMode::SinglePrompt;
                    i += 1;
                }
            }
            other if !other.starts_with('-') && opts.prompt.is_none() => {
                opts.prompt = Some(other.to_string());
                opts.mode = RunMode::SinglePrompt;
            }
            _ => {}
        }
        i += 1;
    }
    opts
}

pub fn print_help() {
    println!(
        "\
HEAV3NS-MICRO v0.1.0 — Hyper-Compressed Rust Micro Agent Runtime
Distilled from LUMI-NEW (zero bloat, 16-MiB Arena slab, self-healing tool parser, 23 native tools, wave scheduler)

USAGE:
  heav3ns-micro [OPTIONS] [PROMPT]

MODES:
  --flight, --benchmark-flight   Run the 5-scenario Agent Flight Test suite (FT-01..FT-05)
  --bench                        Run substrate micro-benchmarks + Flight Test suite
  --smoke                        Run deterministic runtime self-check & smoke verification
  --stdio                        Run headless newline-delimited JSONL stdio worker protocol
  -p, --prompt <TEXT>            Execute a single agent turn and exit

OPTIONS:
  --format <text|json>                  Output format for --flight (default: text)
  -o, --output <PATH>                   Write JSON flight report to file
  --cwd <DIR>                           Workspace root directory
  -m, --model <NAME>                    Active model name (default: gpt-5.6-terra)
  --provider <openai|anthropic|gemini>  Provider wire protocol (default: openai)
  --endpoint <URL>                      Provider API endpoint URL
  -h, --help                            Show this help message"
    );
}

pub async fn run_stdio_worker(engine: Arc<AgentEngine>) {
    let stdin = io::stdin();
    let mut active_cancel: Option<CancellationToken> = None;

    println!(
        "{}",
        json!({
            "type": "ready",
            "runtime": "heav3ns-micro",
            "version": env!("CARGO_PKG_VERSION"),
            "timestamp": now_ms()
        })
    );
    let _ = io::stdout().flush();

    for line in stdin.lock().lines() {
        let Ok(raw) = line else { break };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                println!(
                    "{}",
                    json!({ "type": "error", "code": "INVALID_JSON", "message": e.to_string() })
                );
                let _ = io::stdout().flush();
                continue;
            }
        };

        let req_id = parsed
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("req_0")
            .to_string();
        let method = parsed
            .get("type")
            .or_else(|| parsed.get("method"))
            .and_then(|v| v.as_str())
            .unwrap_or("tick");

        match method {
            "ping" | "health" => {
                let q = engine.queue().snapshot();
                println!(
                    "{}",
                    json!({
                        "id": req_id,
                        "type": "result",
                        "status": "ok",
                        "queueActive": q.active,
                        "queuePending": q.pending_count,
                        "timestamp": now_ms()
                    })
                );
            }
            "cancel" => {
                if let Some(ref tok) = active_cancel {
                    tok.cancel();
                }
                println!("{}", json!({ "id": req_id, "type": "result", "cancelled": true }));
            }
            "flight" => {
                let report = run_agent_flight_test().await;
                println!("{}", json!({ "id": req_id, "type": "result", "report": report }));
            }
            "swarm" => {
                let objective = parsed
                    .get("objective")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Swarm execution")
                    .to_string();
                let assignments: Vec<SwarmAssignment> = parsed
                    .get("assignments")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let token = active_cancel.get_or_insert_with(CancellationToken::new).clone();
                let res = engine.run_swarm(objective, assignments, Some(token), None).await;
                active_cancel = None;
                println!("{}", json!({ "id": req_id, "type": "result", "swarm": res }));
            }
            "shutdown" | "exit" => {
                println!("{}", json!({ "id": req_id, "type": "shutdown" }));
                let _ = io::stdout().flush();
                break;
            }
            "tick" | "chat" | "slash" => {
                let prompt = parsed
                    .get("prompt")
                    .or_else(|| parsed.get("command"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let token = active_cancel.get_or_insert_with(CancellationToken::new).clone();
                let req_id_progress = req_id.clone();
                let mut input = EngineTickInput::new(prompt);
                input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
                input.cancel_token = Some(token);
                input.on_progress = Some(Arc::new(move |ev| {
                    println!(
                        "{}",
                        json!({ "id": req_id_progress, "type": "progress", "event": ev })
                    );
                    let _ = io::stdout().flush();
                }));

                let result = engine.tick(input).await;
                active_cancel = None;
                println!("{}", json!({ "id": req_id, "type": "result", "result": result }));
            }
            other => {
                println!(
                    "{}",
                    json!({
                        "id": req_id,
                        "type": "error",
                        "code": "UNKNOWN_METHOD",
                        "message": format!("Unsupported method: {other}")
                    })
                );
            }
        }
        let _ = io::stdout().flush();
    }
}

