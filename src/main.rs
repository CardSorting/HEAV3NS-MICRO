// [LAYER: UI]
use heav3ns_micro::core::engine::AgentEngine;
use heav3ns_micro::core::flight_runner::{format_flight_test_report, run_agent_flight_test, run_micro_benchmarks};
use heav3ns_micro::domain::types::runtime::{AgentConfig, EngineTickInput, EngineTickOutcome};
use heav3ns_micro::infrastructure::http_provider::HttpProvider;
use heav3ns_micro::ui::cli_presenter::{parse_args, print_help, run_stdio_worker, CliOptions, Heav3nsUserConfig, RunMode};
use heav3ns_micro::ui::onboarding::{create_cli_progress_listener, format_auth_error_card, format_turn_summary, print_onboarding_notice, print_tool_catalog, print_welcome_banner, render_color_diff, run_doctor_cli, run_guided_demo, run_init_wizard, suggest_command};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::sync::Arc;

fn build_engine(opts: &CliOptions) -> (Arc<AgentEngine>, AgentConfig) {
    Heav3nsUserConfig::load_dotenv(&opts.cwd);
    let user_cfg = Heav3nsUserConfig::load();

    let mut cfg = AgentConfig::default();

    // 1. Resolve Provider
    if let Some(ref p) = opts.provider {
        cfg.provider = p.clone();
    } else if let Some(ref p) = user_cfg.as_ref().and_then(|c| c.provider.clone()) {
        cfg.provider = p.clone();
    }

    // 2. Resolve Model
    if let Some(ref m) = opts.model {
        cfg.model_name = m.clone();
    } else if let Some(ref m) = user_cfg.as_ref().and_then(|c| c.model.clone()) {
        cfg.model_name = m.clone();
    } else if cfg.provider.eq_ignore_ascii_case("gemini") || cfg.provider.eq_ignore_ascii_case("google") {
        cfg.model_name = "gemini-2.0-flash".to_string();
    } else if cfg.provider.eq_ignore_ascii_case("anthropic") {
        cfg.model_name = "claude-3-5-sonnet-20241022".to_string();
    } else if cfg.provider.eq_ignore_ascii_case("ollama") {
        cfg.model_name = "llama3.2".to_string();
    }

    // 3. Resolve Endpoint
    if let Some(ref ep) = opts.endpoint {
        cfg.api_endpoint = ep.clone();
    } else if let Some(ref ep) = user_cfg.as_ref().and_then(|c| c.endpoint.clone()) {
        cfg.api_endpoint = ep.clone();
    } else if cfg.provider.eq_ignore_ascii_case("gemini") || cfg.provider.eq_ignore_ascii_case("google") {
        cfg.api_endpoint = "https://generativelanguage.googleapis.com/v1beta".to_string();
    } else if cfg.provider.eq_ignore_ascii_case("anthropic") {
        cfg.api_endpoint = "https://api.anthropic.com/v1".to_string();
    } else if cfg.provider.eq_ignore_ascii_case("ollama") {
        cfg.api_endpoint = "http://localhost:11434/v1".to_string();
    }

    // 4. Resolve API Key
    if cfg.provider.eq_ignore_ascii_case("anthropic") {
        cfg.api_key = env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| user_cfg.as_ref().and_then(|c| c.api_key.clone()));
    } else if cfg.provider.eq_ignore_ascii_case("gemini") || cfg.provider.eq_ignore_ascii_case("google") {
        cfg.api_key = env::var("GEMINI_API_KEY")
            .or_else(|_| env::var("GOOGLE_API_KEY"))
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| user_cfg.as_ref().and_then(|c| c.api_key.clone()));
    } else if !cfg.provider.eq_ignore_ascii_case("ollama") {
        cfg.api_key = env::var("OPENAI_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| user_cfg.as_ref().and_then(|c| c.api_key.clone()));
    }

    let provider = Arc::new(HttpProvider::new(
        cfg.provider.clone(),
        cfg.api_endpoint.clone(),
        cfg.api_key.clone(),
        cfg.provider_timeout_ms,
    ));

    (Arc::new(AgentEngine::new(&opts.cwd, cfg.clone(), provider)), cfg)
}

#[tokio::main]
async fn main() {
    let mut opts = parse_args();
    match opts.mode {
        RunMode::Help => print_help(),
        RunMode::Doctor => {
            run_doctor_cli(&opts.cwd, opts.provider.as_deref(), opts.model.as_deref());
        }
        RunMode::Context => {
            let (engine, _) = build_engine(&opts);
            let fmt = if opts.format.eq_ignore_ascii_case("json") { "json" } else { "" };
            println!("{}", engine.render_context_hud(fmt));
        }
        RunMode::Init => {
            if let Err(e) = run_init_wizard(&opts.cwd) {
                eprintln!("Initialization wizard error: {e}");
                std::process::exit(1);
            }
        }
        RunMode::Demo => {
            run_guided_demo(&opts.cwd).await;
        }
        RunMode::Tools => {
            print_tool_catalog();
        }
        RunMode::Version => {
            println!("heav3ns-micro {}", env!("CARGO_PKG_VERSION"));
        }
        RunMode::ConfigShow => match Heav3nsUserConfig::load() {
            Some(cfg) => println!("{}", serde_json::to_string_pretty(&cfg).unwrap_or_default()),
            None => println!("No configuration file found at {}", Heav3nsUserConfig::config_path().display()),
        },
        RunMode::ConfigSet { key, value } => {
            let mut cfg = Heav3nsUserConfig::load().unwrap_or_default();
            match key.to_ascii_lowercase().as_str() {
                "provider" => cfg.provider = Some(value),
                "model" => cfg.model = Some(value),
                "endpoint" => cfg.endpoint = Some(value),
                "api_key" | "key" => cfg.api_key = Some(value),
                _ => {
                    eprintln!("Unknown config key: {key}. Valid keys: provider, model, endpoint, key");
                    std::process::exit(1);
                }
            }
            if let Err(e) = cfg.save() {
                eprintln!("Failed to save config: {e}");
                std::process::exit(1);
            } else {
                println!("Configuration updated at {}", Heav3nsUserConfig::config_path().display());
            }
        }
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
            let mut payload = json!({
                "runtime": "heav3ns-micro",
                "microBenchmarks": micro,
                "flightSuite": {
                    "passed": flight.summary.passed,
                    "total": flight.summary.total,
                    "durationMs": flight.summary.duration_ms
                }
            });

            if opts.stress || opts.tests_dir.is_some() {
                let full = heav3ns_micro::core::benchmark_runner::run_full_benchmark_suite(opts.tests_dir.as_deref()).await;
                payload["stressBenchmarks"] = full["stressBenchmarks"].clone();
                payload["workspaceTasks"] = full["workspaceTasks"].clone();
                payload["totalDurationMs"] = full["totalDurationMs"].clone();
            }

            println!(
                "{}",
                serde_json::to_string_pretty(&payload).unwrap_or_default()
            );
            if flight.summary.failed > 0 {
                std::process::exit(1);
            }
        }
        RunMode::Smoke => {
            let (engine, _) = build_engine(&opts);
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
        RunMode::StdioWorker => {
            let (engine, _) = build_engine(&opts);
            run_stdio_worker(engine).await;
        }
        RunMode::SinglePrompt => {
            let (engine, cfg) = build_engine(&opts);
            let mut input = EngineTickInput::new(opts.prompt.unwrap_or_default());
            input.on_progress = Some(create_cli_progress_listener());
            let res = engine.tick(input).await;
            if res.outcome == EngineTickOutcome::Failed && (res.response.contains("401") || res.response.contains("API key") || res.response.contains("Authorization")) {
                eprintln!("{}", format_auth_error_card(&res.response, &cfg.provider, &cfg.model_name));
                std::process::exit(1);
            }
            println!("\n{}", res.response);
            println!("{}", format_turn_summary(&res.active_model, res.duration_ms, res.tool_results.len()));
            if res.outcome == EngineTickOutcome::Failed {
                std::process::exit(1);
            }
        }
        RunMode::Interactive => {
            let (mut engine, mut cfg) = build_engine(&opts);
            let has_key = cfg.api_key.is_some();
            print_welcome_banner(&opts.cwd, &cfg.provider, &cfg.model_name, has_key);
            if !has_key && !cfg.provider.eq_ignore_ascii_case("ollama") {
                print_onboarding_notice(&cfg.provider, &cfg.model_name);
            } else {
                println!("Type /help for slash commands, /doctor to diagnose, /demo for offline tour, /exit to quit.\n");
            }

            let valid_slash_cmds = [
                "/help", "/context", "/prompt", "/stats", "/vfs", "/diff", "/commit", "/discard", "/tools",
                "/guide", "/demo", "/memory", "/skills", "/soul", "/forge", "/doctor",
                "/computer", "/spatial", "/browser", "/vision", "/legion", "/compact", "/model",
                "/snapshot", "/snapshots", "/rewind", "/undo", "/clear", "/health",
                "/pwd", "/workspace", "/cd",
            ];

            let progress_cb = create_cli_progress_listener();
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
                        if matches!(trimmed, "/exit" | "/quit" | "exit" | "quit" | ":q") {
                            println!("Session ended. Goodbye!");
                            break;
                        }
                        // Handle user-friendly aliases without leading slash
                        if matches!(trimmed, "pwd" | "/pwd" | "workspace" | "/workspace") {
                            println!("Active workspace root: {}", engine.workspace_root().display());
                            continue;
                        }
                        if trimmed.starts_with("cd ") || trimmed.starts_with("/cd ") || trimmed == "cd" || trimmed == "/cd" {
                            let raw_target = if let Some(p) = trimmed.strip_prefix("/cd ") {
                                p.trim()
                            } else if let Some(p) = trimmed.strip_prefix("cd ") {
                                p.trim()
                            } else {
                                "~"
                            };
                            let clean_target = raw_target.trim_matches(|c| c == '\'' || c == '"');
                            let target_path = if clean_target == "~" || clean_target.starts_with("~/") {
                                if let Ok(home) = std::env::var("HOME") {
                                    if clean_target == "~" {
                                        std::path::PathBuf::from(home)
                                    } else {
                                        std::path::PathBuf::from(home).join(&clean_target[2..])
                                    }
                                } else {
                                    std::path::PathBuf::from(clean_target)
                                }
                            } else {
                                std::path::PathBuf::from(clean_target)
                            };
                            let resolved = if target_path.is_absolute() {
                                target_path
                            } else {
                                opts.cwd.join(target_path)
                            };
                            if resolved.is_dir() {
                                opts.cwd = resolved.canonicalize().unwrap_or(resolved);
                                println!("  ✔ Workspace switched to: {}\n", opts.cwd.display());
                                let (new_engine, new_cfg) = build_engine(&opts);
                                engine = new_engine;
                                cfg = new_cfg;
                            } else {
                                println!("  ✖ Directory not found: {}\n", raw_target);
                            }
                            continue;
                        }
                        if trimmed == "model" || trimmed == "/model" {
                            println!("Active model: {} (provider: {})\n", cfg.model_name, cfg.provider);
                            println!("Popular choices:\n  • gpt-5.6-terra (Default / Fast Reasoning)\n  • gpt-4o (High-Throughput Multimodal)\n  • gpt-4o-mini (Lightweight)\n  • gemini-2.0-flash (Google Gemini)\n  • claude-3-5-sonnet-20241022 (Claude)\n  • llama3.2 (Local Ollama)\n\nTo switch model: /model <name>");
                            continue;
                        }
                        if matches!(trimmed, "help" | "?") {
                            println!("{}", engine.tick(EngineTickInput::new("/help")).await.response);
                            continue;
                        }
                        if matches!(trimmed, "doctor") {
                            run_doctor_cli(&opts.cwd, Some(&cfg.provider), Some(&cfg.model_name));
                            continue;
                        }
                        if matches!(trimmed, "demo" | "tutorial") {
                            run_guided_demo(&opts.cwd).await;
                            continue;
                        }
                        if matches!(trimmed, "tools") {
                            print_tool_catalog();
                            continue;
                        }
                        if matches!(trimmed, "stats") {
                            println!("{}", engine.tick(EngineTickInput::new("/stats")).await.response);
                            continue;
                        }
                        if matches!(trimmed, "context") {
                            println!("{}", engine.render_context_hud(""));
                            continue;
                        }
                        if matches!(trimmed, "prompt") {
                            println!("{}", engine.render_prompt_inspector(""));
                            continue;
                        }
                        if matches!(trimmed, "vfs") {
                            println!("{}", engine.tick(EngineTickInput::new("/vfs")).await.response);
                            continue;
                        }
                        if matches!(trimmed, "diff" | "/diff") {
                            let diff_text = engine.tick(EngineTickInput::new("/diff")).await.response;
                            println!("{}", render_color_diff(&diff_text));
                            continue;
                        }
                        if matches!(trimmed, "clear") {
                            println!("{}", engine.tick(EngineTickInput::new("/clear")).await.response);
                            continue;
                        }

                        // Check for unknown slash command typo and suggest
                        if trimmed.starts_with('/') {
                            let cmd_token = trimmed.split_whitespace().next().unwrap_or("");
                            if !valid_slash_cmds.contains(&cmd_token) {
                                if let Some(suggestion) = suggest_command(cmd_token, &valid_slash_cmds) {
                                    println!("Unknown command '{}'. Did you mean '{}'? Type /help for all commands.", cmd_token, suggestion);
                                    continue;
                                }
                            }
                        }

                        let mut input = EngineTickInput::new(trimmed);
                        input.on_progress = Some(progress_cb.clone());
                        let res = engine.tick(input).await;
                        if res.outcome == EngineTickOutcome::Failed
                            && (res.response.contains("401") || res.response.contains("API key") || res.response.contains("Authorization"))
                        {
                            println!("{}", format_auth_error_card(&res.response, &cfg.provider, &cfg.model_name));
                        } else {
                            println!("\n{}", res.response);
                            println!("{}", format_turn_summary(&res.active_model, res.duration_ms, res.tool_results.len()));
                        }
                    }
                }
            }
        }
    }
}
