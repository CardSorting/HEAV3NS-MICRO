// [LAYER: UI]
use heav3ns_micro::core::engine::AgentEngine;
use heav3ns_micro::core::flight_runner::{format_flight_test_report, run_agent_flight_test, run_micro_benchmarks};
use heav3ns_micro::domain::types::provider_registry::{canonicalize_provider_id, lookup_provider_descriptor, resolve_effective_wire_override, resolve_provider_route, retired_provider_remediation, ReasoningEffortLevel, WireDialect, PROVIDER_DESCRIPTORS};
use heav3ns_micro::domain::types::runtime::{AgentConfig, CancellationToken, EngineTickInput, EngineTickOutcome};
use heav3ns_micro::infrastructure::http_provider::{probe_local_ollama_model, HttpProvider};
use heav3ns_micro::ui::cli_presenter::{parse_args, print_help, CliOptions, Heav3nsUserConfig, RunMode};
use heav3ns_micro::ui::onboarding::{create_cli_progress_listener, format_auth_error_card, format_turn_summary, print_onboarding_notice, print_tool_catalog, print_welcome_banner, render_color_diff, run_doctor_cli, run_guided_demo, run_init_wizard, suggest_command};
use heav3ns_micro::ui::stdio_worker::run_stdio_worker;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, Write};
use std::sync::Arc;

async fn build_engine(opts: &CliOptions) -> (Arc<AgentEngine>, AgentConfig) {
    let dotenv = Heav3nsUserConfig::read_dotenv_map(&opts.cwd);
    let user_cfg = Heav3nsUserConfig::load();
    let mut cfg = AgentConfig::default();

    let mut saved_keys = user_cfg.as_ref().map(|c| c.effective_provider_keys()).unwrap_or_default();
    for d in PROVIDER_DESCRIPTORS {
        if d.retired {
            continue;
        }
        if !saved_keys.contains_key(d.secret_slot) {
            if let Some(val) = d.env_vars.iter().find_map(|ev| Heav3nsUserConfig::resolve_env_or_dotenv(ev, &dotenv)) {
                saved_keys.insert(d.secret_slot.to_string(), val);
            }
        }
    }
    let mut saved_endpoints = user_cfg.as_ref().map(|c| c.provider_endpoints.clone()).unwrap_or_default();
    for d in PROVIDER_DESCRIPTORS {
        if d.retired {
            continue;
        }
        if let Some(ep_env) = d.endpoint_env {
            if !saved_endpoints.contains_key(d.id) {
                if let Some(ep_val) = Heav3nsUserConfig::resolve_env_or_dotenv(ep_env, &dotenv) {
                    saved_endpoints.insert(d.id.to_string(), ep_val);
                }
            }
        }
    }
    if let (Some(p), Some(ep)) = (
        user_cfg.as_ref().and_then(|c| c.provider.as_deref()),
        user_cfg.as_ref().and_then(|c| c.endpoint.as_deref()),
    ) {
        let slot = lookup_provider_descriptor(p).map(|d| d.secret_slot).unwrap_or(p);
        saved_endpoints.entry(slot.to_string()).or_insert_with(|| ep.to_string());
    }

    let explicit_provider = opts
        .provider
        .clone()
        .or_else(|| user_cfg.as_ref().and_then(|c| c.provider.clone()));

    if let Some(ref raw_p) = explicit_provider {
        if let Some(retired_msg) = retired_provider_remediation(raw_p) {
            eprintln!("Error: {retired_msg}");
            std::process::exit(1);
        }
    }

    let selected_provider = if let Some(ref p) = explicit_provider {
        canonicalize_provider_id(p)
    } else if Heav3nsUserConfig::resolve_env_or_dotenv("OPENAI_API_KEY", &dotenv).is_some() {
        "openai".to_string()
    } else {
        PROVIDER_DESCRIPTORS
            .iter()
            .find(|d| {
                !d.retired
                    && d.requires_api_key
                    && d.env_vars
                        .iter()
                        .any(|ev| Heav3nsUserConfig::resolve_env_or_dotenv(ev, &dotenv).is_some())
            })
            .map(|d| d.id.to_string())
            .unwrap_or_else(|| "openai".to_string())
    };

    AgentEngine::trigger_model_catalog_auto_refresh();
    let catalog = AgentEngine::model_catalog_snapshot();

    let cfg_model_matches_provider = explicit_provider.is_none()
        || opts.provider.is_none()
        || user_cfg
            .as_ref()
            .and_then(|c| c.provider.as_deref())
            .is_some_and(|up| canonicalize_provider_id(up) == selected_provider);
    let explicit_model = opts.model.clone().or_else(|| {
        if cfg_model_matches_provider {
            user_cfg.as_ref().and_then(|c| c.model.clone())
        } else {
            None
        }
    });

    let wire_override = resolve_effective_wire_override(
        opts.wire_dialect.as_deref(),
        user_cfg.as_ref().and_then(|c| c.wire_dialect.as_deref()),
        user_cfg.as_ref().and_then(|c| c.provider.as_deref()),
        &selected_provider,
    );

    let route = resolve_provider_route(
        &selected_provider,
        explicit_model.as_deref(),
        opts.endpoint.as_deref(),
        None,
        wire_override.as_deref(),
        None,
        None,
        &saved_keys,
        &saved_endpoints,
        |env_key| Heav3nsUserConfig::resolve_env_or_dotenv(env_key, &dotenv),
    );

    cfg.provider = route.provider_id.clone();
    cfg.model_name = if explicit_model.is_some() {
        route.model_name
    } else if matches!(
        route.provider_id.as_str(),
        "gemini" | "anthropic" | "openrouter" | "deepseek" | "xai" | "mistral" | "qwen" | "moonshot" | "zai"
    ) {
        catalog.recommend_tiers(&route.provider_id).1
    } else {
        route.model_name
    };
    cfg.api_endpoint = route.api_endpoint;
    cfg.api_key = route.api_key;
    let has_explicit_wire = wire_override.is_some();
    cfg.wire_dialect = wire_override;
    let raw_reasoning = opts
        .reasoning_effort
        .clone()
        .or_else(|| user_cfg.as_ref().and_then(|c| c.reasoning_effort.clone()));
    cfg.reasoning_effort = raw_reasoning.and_then(|raw| {
        ReasoningEffortLevel::parse_flexible(&raw)
            .ok()
            .flatten()
            .map(|l| l.as_str().to_string())
    });
    cfg.fallback_providers = user_cfg
        .as_ref()
        .map(|c| c.fallback_providers.clone())
        .unwrap_or_default();
    cfg.provider_keys = saved_keys;
    cfg.provider_endpoints = saved_endpoints;
    if cfg.provider != "openai" {
        cfg.fallback_model = None;
    }

    let mut effective_wire = route.wire_dialect;
    let mut keep_explicit_wire = has_explicit_wire;
    // Zero-config local Ollama fallback when no API key is present and no cloud provider was forced
    if cfg.api_key.is_none()
        && explicit_provider.is_none()
        && opts.endpoint.is_none()
        && lookup_provider_descriptor(&cfg.provider).is_none_or(|d| d.requires_api_key)
    {
        if let Some(local_model) = probe_local_ollama_model().await {
            cfg.provider = "ollama".to_string();
            cfg.api_endpoint = "http://127.0.0.1:11434/v1".to_string();
            effective_wire = WireDialect::ChatCompletions;
            keep_explicit_wire = false;
            if opts.model.is_none() {
                cfg.model_name = local_model;
            }
        }
    }

    let provider = Arc::new(
        HttpProvider::new(
            cfg.provider.clone(),
            cfg.api_endpoint.clone(),
            cfg.api_key.clone(),
            cfg.provider_timeout_ms,
        )
        .with_wire_dialect(effective_wire)
        .with_explicit_wire_override(keep_explicit_wire),
    );

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
            let (engine, _) = build_engine(&opts).await;
            let fmt = if opts.format.eq_ignore_ascii_case("json") { "json" } else { "" };
            println!("{}", engine.render_context_hud(fmt));
        }
        RunMode::PreviewRequest => {
            let (engine, _) = build_engine(&opts).await;
            let fmt = if opts.format.eq_ignore_ascii_case("json") { "json" } else { "" };
            println!("{}", engine.render_request_manifest(fmt));
        }
        RunMode::Providers => {
            let dotenv = Heav3nsUserConfig::read_dotenv_map(&opts.cwd);
            let vault = Heav3nsUserConfig::load().map(|c| c.effective_provider_keys()).unwrap_or_default();
            if opts.format.eq_ignore_ascii_case("json") {
                let items: Vec<Value> = PROVIDER_DESCRIPTORS
                    .iter()
                    .filter(|d| !d.retired)
                    .map(|d| {
                        let env_ready = d.env_vars.iter().any(|ev| Heav3nsUserConfig::resolve_env_or_dotenv(ev, &dotenv).is_some());
                        let vault_ready = vault.contains_key(d.secret_slot);
                        json!({
                            "id": d.id,
                            "displayName": d.display_name,
                            "aliases": d.aliases,
                            "wireDialect": d.wire_dialect.as_str(),
                            "defaultModel": d.default_model,
                            "defaultEndpoint": d.default_endpoint,
                            "envVars": d.env_vars,
                            "requiresApiKey": d.requires_api_key,
                            "modelAware": d.model_aware,
                            "configured": !d.requires_api_key || env_ready || vault_ready,
                            "credentialUrl": d.credential_url
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&json!({ "count": items.len(), "providers": items })).unwrap_or_default());
            } else {
                let (engine, _) = build_engine(&opts).await;
                println!("{}", engine.inspect_or_switch_provider("list"));
            }
        }
        RunMode::Models { refresh } => {
            let cat = if refresh {
                match AgentEngine::refresh_model_catalog().await {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Live model catalog refresh failed ({e}); using cached/embedded catalog.");
                        AgentEngine::model_catalog_snapshot()
                    }
                }
            } else {
                AgentEngine::model_catalog_snapshot()
            };
            let prov = opts.provider.as_deref().unwrap_or("openrouter");
            let top: Vec<Value> = cat
                .newest_models_for_provider(prov, 25)
                .into_iter()
                .map(|e| {
                    json!({
                        "id": e.id,
                        "name": e.display_name,
                        "contextLength": e.context_length,
                        "inputPerMillion": e.input_per_million,
                        "outputPerMillion": e.output_per_million,
                        "cachedPerMillion": e.cached_per_million,
                        "createdUnix": e.created_unix
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "source": cat.source,
                    "fetchedAtMs": cat.fetched_at_ms,
                    "totalModels": cat.entries.len(),
                    "provider": prov,
                    "models": top
                }))
                .unwrap_or_default()
            );
        }
        RunMode::Receipts => {
            let (engine, _) = build_engine(&opts).await;
            let arg = if opts.format.eq_ignore_ascii_case("json") {
                "json"
            } else if opts.last_only {
                "last"
            } else {
                ""
            };
            let cmd = if arg.is_empty() { "/receipts".to_string() } else { format!("/receipts {arg}") };
            println!("{}", engine.tick(EngineTickInput::new(cmd)).await.response);
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
            let low = key.to_ascii_lowercase();
            if let Some(prov_slot) = low.strip_prefix("key.") {
                cfg.set_provider_key(prov_slot, value);
            } else if let Some(prov_slot) = low.strip_prefix("endpoint.") {
                let slot = lookup_provider_descriptor(prov_slot).map(|d| d.secret_slot).unwrap_or(prov_slot);
                cfg.provider_endpoints.insert(slot.to_string(), value);
            } else {
                match low.as_str() {
                    "provider" => {
                        if let Some(retired_msg) = retired_provider_remediation(&value) {
                            eprintln!("Error: {retired_msg}");
                            std::process::exit(1);
                        }
                        cfg.provider = Some(canonicalize_provider_id(&value));
                    }
                    "model" => cfg.model = Some(value),
                    "endpoint" => cfg.endpoint = Some(value),
                    "api_key" | "key" => {
                        if let Some(ref p) = cfg.provider.clone() {
                            cfg.set_provider_key(p, value.clone());
                        }
                        cfg.api_key = Some(value);
                    }
                    "reasoning" | "effort" => {
                        match ReasoningEffortLevel::parse_flexible(&value) {
                            Ok(None) => cfg.reasoning_effort = None,
                            Ok(Some(lvl)) => cfg.reasoning_effort = Some(lvl.as_str().to_string()),
                            Err(msg) => {
                                eprintln!("Error: {msg}");
                                std::process::exit(1);
                            }
                        }
                    }
                    "wire" | "wire_dialect" => cfg.wire_dialect = Some(value),
                    "fallbacks" | "fallback_providers" => {
                        cfg.fallback_providers = value
                            .split(',')
                            .map(|s| canonicalize_provider_id(s.trim()))
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                    _ => {
                        eprintln!("Unknown config key: {key}. Valid keys: provider, model, endpoint, key, key.<provider>, endpoint.<provider>, reasoning, wire, fallbacks");
                        std::process::exit(1);
                    }
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
            println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_default());
            if flight.summary.failed > 0 {
                std::process::exit(1);
            }
        }
        RunMode::Smoke => {
            let (engine, _) = build_engine(&opts).await;
            let r1 = engine.tick(EngineTickInput::new("remember: HEAV3NS-MICRO uses a 16-MiB Arena slab.")).await;
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
            let (engine, _) = build_engine(&opts).await;
            run_stdio_worker(engine).await;
        }
        RunMode::SinglePrompt => {
            let (engine, cfg) = build_engine(&opts).await;
            let cancel = CancellationToken::new();
            let cancel_sig = cancel.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    cancel_sig.cancel();
                }
            });
            let mut input = EngineTickInput::new(opts.prompt.unwrap_or_default());
            input.cancel_token = Some(cancel);
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
            let (mut engine, mut cfg) = build_engine(&opts).await;
            let has_key = cfg.api_key.is_some();
            let is_keyless_local = lookup_provider_descriptor(&cfg.provider).is_some_and(|d| !d.requires_api_key);
            print_welcome_banner(&opts.cwd, &cfg.provider, &cfg.model_name, has_key);
            if !has_key && !is_keyless_local {
                print_onboarding_notice(&cfg.provider, &cfg.model_name);
            } else {
                println!("Type /help for slash commands, /provider to switch across 43+ routes, /mode to cycle posture, /exit to quit.\n");
            }

            let valid_slash_cmds = [
                "/help", "/mode", "/posture", "/preview-request", "/preview", "/receipts", "/turn", "/restore",
                "/context", "/prompt", "/stats", "/vfs", "/diff", "/commit", "/discard", "/tools",
                "/guide", "/quickstart", "/demo", "/memory", "/skills", "/soul", "/forge", "/doctor",
                "/computer", "/spatial", "/browser", "/vision", "/legion", "/swarm", "/compact", "/model",
                "/models", "/provider", "/providers", "/reasoning", "/effort",
                "/snapshot", "/snapshots", "/rewind", "/undo", "/clear", "/health", "/pwd", "/workspace", "/cd",
            ];

            let progress_cb = create_cli_progress_listener();
            let stdin = io::stdin();
            loop {
                print!("heav3ns [{}]> ", engine.current_posture().slug());
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
                        if matches!(trimmed, "pwd" | "/pwd" | "workspace" | "/workspace") {
                            println!("Active workspace root: {}", engine.workspace_root().display());
                            continue;
                        }
                        if trimmed.starts_with("cd ") || trimmed.starts_with("/cd ") || trimmed == "cd" || trimmed == "/cd" {
                            let raw_target = trimmed.strip_prefix("/cd ").or_else(|| trimmed.strip_prefix("cd ")).map(str::trim).unwrap_or("~");
                            let clean_target = raw_target.trim_matches(|c| c == '\'' || c == '"');
                            let target_path = if clean_target == "~" || clean_target.starts_with("~/") {
                                if let Ok(home) = std::env::var("HOME") {
                                    if clean_target == "~" { std::path::PathBuf::from(home) } else { std::path::PathBuf::from(home).join(&clean_target[2..]) }
                                } else {
                                    std::path::PathBuf::from(clean_target)
                                }
                            } else {
                                std::path::PathBuf::from(clean_target)
                            };
                            let resolved = if target_path.is_absolute() { target_path } else { opts.cwd.join(target_path) };
                            if resolved.is_dir() {
                                opts.cwd = resolved.canonicalize().unwrap_or(resolved);
                                println!("  ✔ Workspace switched to: {}\n", opts.cwd.display());
                                let (new_engine, new_cfg) = build_engine(&opts).await;
                                engine = new_engine;
                                cfg = new_cfg;
                            } else {
                                println!("  ✖ Directory not found: {}\n", raw_target);
                            }
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
                        if matches!(trimmed, "diff" | "/diff") {
                            let diff_text = engine.tick(EngineTickInput::new("/diff")).await.response;
                            println!("{}", render_color_diff(&diff_text));
                            continue;
                        }
                        if trimmed.starts_with('/') {
                            let cmd_token = trimmed.split_whitespace().next().unwrap_or("");
                            if !valid_slash_cmds.contains(&cmd_token) {
                                if let Some(suggestion) = suggest_command(cmd_token, &valid_slash_cmds) {
                                    println!("Unknown command '{}'. Did you mean '{}'? Type /help for all commands.", cmd_token, suggestion);
                                    continue;
                                }
                            }
                        }

                        let cancel = CancellationToken::new();
                        let cancel_sig = cancel.clone();
                        let sig_task = tokio::spawn(async move {
                            if tokio::signal::ctrl_c().await.is_ok() {
                                cancel_sig.cancel();
                            }
                        });
                        let mut input = EngineTickInput::new(trimmed);
                        input.cancel_token = Some(cancel);
                        input.on_progress = Some(progress_cb.clone());
                        let res = engine.tick(input).await;
                        sig_task.abort();
                        cfg = engine.config_snapshot();
                        if res.outcome == EngineTickOutcome::Failed
                            && (res.response.contains("401") || res.response.contains("API key") || res.response.contains("Authorization"))
                        {
                            println!("{}", format_auth_error_card(&res.response, &cfg.provider, &cfg.model_name));
                        } else {
                            println!("\n{}", res.response);
                            if !res.is_slash_command {
                                println!("{}", format_turn_summary(&res.active_model, res.duration_ms, res.tool_results.len()));
                            }
                        }
                    }
                }
            }
        }
    }
}
