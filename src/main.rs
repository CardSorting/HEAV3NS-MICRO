use heav3ns_micro::engine::{AgentEngine, SwarmAssignment};
use heav3ns_micro::flight::{format_flight_test_report, run_agent_flight_test, run_micro_benchmarks};
use heav3ns_micro::provider::HttpProvider;
use heav3ns_micro::{now_ms, AgentConfig, CancellationToken, EngineTickInput, ExecutionAuthorityLevel};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

struct CliOptions {
    mode: RunMode,
    format: String,
    output: Option<PathBuf>,
    cwd: PathBuf,
    model: Option<String>,
    provider: Option<String>,
    endpoint: Option<String>,
    prompt: Option<String>,
}

#[derive(PartialEq, Eq)]
enum RunMode { FlightTest, Bench, Smoke, StdioWorker, SinglePrompt, Interactive, Help }

fn parse_args() -> CliOptions {
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
            "--format" => if let Some(v) = args.get(i + 1) { opts.format = v.clone(); i += 1; },
            "--output" | "-o" => if let Some(v) = args.get(i + 1) { opts.output = Some(PathBuf::from(v)); i += 1; },
            "--cwd" => if let Some(v) = args.get(i + 1) { opts.cwd = PathBuf::from(v); i += 1; },
            "--model" | "-m" => if let Some(v) = args.get(i + 1) { opts.model = Some(v.clone()); i += 1; },
            "--provider" => if let Some(v) = args.get(i + 1) { opts.provider = Some(v.clone()); i += 1; },
            "--endpoint" => if let Some(v) = args.get(i + 1) { opts.endpoint = Some(v.clone()); i += 1; },
            "--prompt" | "-p" => if let Some(v) = args.get(i + 1) { opts.prompt = Some(v.clone()); opts.mode = RunMode::SinglePrompt; i += 1; },
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

fn print_help() {
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

fn build_engine(opts: &CliOptions) -> Arc<AgentEngine> {
    let mut cfg = AgentConfig::default();
    if let Some(ref m) = opts.model { cfg.model_name = m.clone(); }
    if let Some(ref p) = opts.provider { cfg.provider = p.clone(); }
    if let Some(ref ep) = opts.endpoint { cfg.api_endpoint = ep.clone(); }
    if cfg.provider.eq_ignore_ascii_case("anthropic") && cfg.api_key.is_none() {
        cfg.api_key = env::var("ANTHROPIC_API_KEY").ok().filter(|s| !s.is_empty());
    } else if (cfg.provider.eq_ignore_ascii_case("gemini") || cfg.provider.eq_ignore_ascii_case("google")) && cfg.api_key.is_none() {
        cfg.api_key = env::var("GEMINI_API_KEY").or_else(|_| env::var("GOOGLE_API_KEY")).ok().filter(|s| !s.is_empty());
    }
    let provider = Arc::new(HttpProvider::new(
        cfg.provider.clone(), cfg.api_endpoint.clone(), cfg.api_key.clone(), cfg.provider_timeout_ms,
    ));
    Arc::new(AgentEngine::new(&opts.cwd, cfg, provider))
}

async fn run_stdio_worker(engine: Arc<AgentEngine>) {
    let stdin = io::stdin();
    let mut active_cancel: Option<CancellationToken> = None;

    println!("{}", json!({ "type": "ready", "runtime": "heav3ns-micro", "version": env!("CARGO_PKG_VERSION"), "timestamp": now_ms() }));
    let _ = io::stdout().flush();

    for line in stdin.lock().lines() {
        let Ok(raw) = line else { break };
        let trimmed = raw.trim();
        if trimmed.is_empty() { continue; }

        let parsed: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                println!("{}", json!({ "type": "error", "code": "INVALID_JSON", "message": e.to_string() }));
                let _ = io::stdout().flush();
                continue;
            }
        };

        let req_id = parsed.get("id").and_then(|v| v.as_str()).unwrap_or("req_0").to_string();
        let method = parsed.get("type").or_else(|| parsed.get("method")).and_then(|v| v.as_str()).unwrap_or("tick");

        match method {
            "ping" | "health" => {
                let q = engine.queue().snapshot();
                println!("{}", json!({ "id": req_id, "type": "result", "status": "ok", "queueActive": q.active, "queuePending": q.pending_count, "timestamp": now_ms() }));
            }
            "cancel" => {
                if let Some(ref tok) = active_cancel { tok.cancel(); }
                println!("{}", json!({ "id": req_id, "type": "result", "cancelled": true }));
            }
            "flight" => {
                let report = run_agent_flight_test().await;
                println!("{}", json!({ "id": req_id, "type": "result", "report": report }));
            }
            "swarm" => {
                let objective = parsed.get("objective").and_then(|v| v.as_str()).unwrap_or("Swarm execution").to_string();
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
                let prompt = parsed.get("prompt").or_else(|| parsed.get("command")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let token = active_cancel.get_or_insert_with(CancellationToken::new).clone();
                let req_id_progress = req_id.clone();
                let mut input = EngineTickInput::new(prompt);
                input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
                input.cancel_token = Some(token);
                input.on_progress = Some(Arc::new(move |ev| {
                    println!("{}", json!({ "id": req_id_progress, "type": "progress", "event": ev }));
                    let _ = io::stdout().flush();
                }));

                let result = engine.tick(input).await;
                active_cancel = None;
                println!("{}", json!({ "id": req_id, "type": "result", "result": result }));
            }
            other => {
                println!("{}", json!({ "id": req_id, "type": "error", "code": "UNKNOWN_METHOD", "message": format!("Unsupported method: {other}") }));
            }
        }
        let _ = io::stdout().flush();
    }
}

#[tokio::main]
async fn main() {
    let opts = parse_args();
    match opts.mode {
        RunMode::Help => print_help(),
        RunMode::FlightTest => {
            let report = run_agent_flight_test().await;
            if let Some(ref out_path) = opts.output {
                if let Some(parent) = out_path.parent() { let _ = fs::create_dir_all(parent); }
                let _ = fs::write(out_path, serde_json::to_string_pretty(&report).unwrap_or_default());
            }
            if opts.format.eq_ignore_ascii_case("json") {
                println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
            } else {
                println!("{}", format_flight_test_report(&report));
            }
            if report.summary.failed > 0 { std::process::exit(1); }
        }
        RunMode::Bench => {
            let micro = run_micro_benchmarks();
            let flight = run_agent_flight_test().await;
            println!("{}", serde_json::to_string_pretty(&json!({
                "runtime": "heav3ns-micro",
                "microBenchmarks": micro,
                "flightSuite": { "passed": flight.summary.passed, "total": flight.summary.total, "durationMs": flight.summary.duration_ms }
            })).unwrap_or_default());
            if flight.summary.failed > 0 { std::process::exit(1); }
        }
        RunMode::Smoke => {
            let engine = build_engine(&opts);
            let r1 = engine.tick(EngineTickInput::new("remember: HEAV3NS-MICRO uses a 16-MiB Arena slab.")).await;
            let r2 = engine.tick(EngineTickInput::new("/stats")).await;
            let r3 = engine.tick(EngineTickInput::new("/snapshot")).await;
            let flight = run_agent_flight_test().await;
            println!("{}", serde_json::to_string_pretty(&json!({
                "smoke": "PASS",
                "memoryFastPath": r1.response,
                "stats": serde_json::from_str::<Value>(&r2.response).unwrap_or(Value::String(r2.response)),
                "snapshot": r3.response,
                "flightSummary": flight.summary,
            })).unwrap_or_default());
            if flight.summary.failed > 0 { std::process::exit(1); }
        }
        RunMode::StdioWorker => run_stdio_worker(build_engine(&opts)).await,
        RunMode::SinglePrompt => {
            let engine = build_engine(&opts);
            let res = engine.tick(EngineTickInput::new(opts.prompt.unwrap_or_default())).await;
            println!("{}", res.response);
            if res.outcome == heav3ns_micro::EngineTickOutcome::Failed { std::process::exit(1); }
        }
        RunMode::Interactive => {
            let engine = build_engine(&opts);
            println!("HEAV3NS-MICRO v0.1.0 Interactive Runtime (type /help for commands, /exit to quit)");
            let stdin = io::stdin();
            loop {
                print!("heav3ns> ");
                let _ = io::stdout().flush();
                let mut line = String::new();
                match stdin.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() { continue; }
                        if matches!(trimmed, "/exit" | "/quit" | "exit" | "quit") { break; }
                        println!("{}", engine.tick(EngineTickInput::new(trimmed)).await.response);
                    }
                }
            }
        }
    }
}
