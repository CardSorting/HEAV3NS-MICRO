// [LAYER: UI]
use heav3ns_micro::core::engine::AgentEngine;
use heav3ns_micro::core::flight_runner::{format_flight_test_report, run_agent_flight_test, run_micro_benchmarks};
use heav3ns_micro::domain::types::runtime::{AgentConfig, EngineTickInput, EngineTickOutcome};
use heav3ns_micro::infrastructure::http_provider::HttpProvider;
use heav3ns_micro::ui::cli_presenter::{parse_args, print_help, run_stdio_worker, CliOptions, RunMode};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::sync::Arc;

fn build_engine(opts: &CliOptions) -> Arc<AgentEngine> {
    let mut cfg = AgentConfig::default();
    if let Some(ref m) = opts.model {
        cfg.model_name = m.clone();
    }
    if let Some(ref p) = opts.provider {
        cfg.provider = p.clone();
    }
    if let Some(ref ep) = opts.endpoint {
        cfg.api_endpoint = ep.clone();
    }
    if cfg.provider.eq_ignore_ascii_case("anthropic") && cfg.api_key.is_none() {
        cfg.api_key = env::var("ANTHROPIC_API_KEY").ok().filter(|s| !s.is_empty());
    } else if (cfg.provider.eq_ignore_ascii_case("gemini") || cfg.provider.eq_ignore_ascii_case("google"))
        && cfg.api_key.is_none()
    {
        cfg.api_key = env::var("GEMINI_API_KEY")
            .or_else(|_| env::var("GOOGLE_API_KEY"))
            .ok()
            .filter(|s| !s.is_empty());
    }
    let provider = Arc::new(HttpProvider::new(
        cfg.provider.clone(),
        cfg.api_endpoint.clone(),
        cfg.api_key.clone(),
        cfg.provider_timeout_ms,
    ));
    Arc::new(AgentEngine::new(&opts.cwd, cfg, provider))
}

#[tokio::main]
async fn main() {
    let opts = parse_args();
    match opts.mode {
        RunMode::Help => print_help(),
        RunMode::FlightTest => {
            let report = run_agent_flight_test().await;
            if let Some(ref out_path) = opts.output {
                if let Some(parent) = out_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(out_path, serde_json::to_string_pretty(&report).unwrap_or_default());
            }
            if opts.format.eq_ignore_ascii_case("json") {
                println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
            } else {
                println!("{}", format_flight_test_report(&report));
            }
            if report.summary.failed > 0 {
                std::process::exit(1);
            }
        }
        RunMode::Bench => {
            let micro = run_micro_benchmarks();
            let flight = run_agent_flight_test().await;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "runtime": "heav3ns-micro",
                    "microBenchmarks": micro,
                    "flightSuite": {
                        "passed": flight.summary.passed,
                        "total": flight.summary.total,
                        "durationMs": flight.summary.duration_ms
                    }
                }))
                .unwrap_or_default()
            );
            if flight.summary.failed > 0 {
                std::process::exit(1);
            }
        }
        RunMode::Smoke => {
            let engine = build_engine(&opts);
            let r1 = engine
                .tick(EngineTickInput::new(
                    "remember: HEAV3NS-MICRO uses a 16-MiB Arena slab.",
                ))
                .await;
            let r2 = engine.tick(EngineTickInput::new("/stats")).await;
            let r3 = engine.tick(EngineTickInput::new("/snapshot")).await;
            let flight = run_agent_flight_test().await;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "smoke": "PASS",
                    "memoryFastPath": r1.response,
                    "stats": serde_json::from_str::<Value>(&r2.response).unwrap_or(Value::String(r2.response)),
                    "snapshot": r3.response,
                    "flightSummary": flight.summary,
                }))
                .unwrap_or_default()
            );
            if flight.summary.failed > 0 {
                std::process::exit(1);
            }
        }
        RunMode::StdioWorker => run_stdio_worker(build_engine(&opts)).await,
        RunMode::SinglePrompt => {
            let engine = build_engine(&opts);
            let res = engine.tick(EngineTickInput::new(opts.prompt.unwrap_or_default())).await;
            println!("{}", res.response);
            if res.outcome == EngineTickOutcome::Failed {
                std::process::exit(1);
            }
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
                        if trimmed.is_empty() {
                            continue;
                        }
                        if matches!(trimmed, "/exit" | "/quit" | "exit" | "quit") {
                            break;
                        }
                        println!("{}", engine.tick(EngineTickInput::new(trimmed)).await.response);
                    }
                }
            }
        }
    }
}
