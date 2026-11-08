// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::core::flight_runner::run_agent_flight_test;
use crate::domain::swarm::SwarmAssignment;
use crate::domain::types::runtime::{CancellationToken, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel};
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
    let mut session_runs: Vec<Value> = Vec::new();
    let mut run_sequence: i64 = 0;
    let mut current_conversation = "default-conv".to_string();

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
        let ws_id = parsed
            .get("workspace")
            .and_then(|v| v.as_str())
            .unwrap_or("ws_default")
            .to_string();
        let sess_id = parsed
            .get("session")
            .and_then(|v| v.as_str())
            .unwrap_or("sess_default")
            .to_string();
        let is_rpc = parsed.get("v").and_then(|v| v.as_i64()) == Some(1)
            || parsed.get("kind").and_then(|v| v.as_str()) == Some("request");

        let method = parsed
            .get("method")
            .or_else(|| parsed.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("tick");

        let params = parsed.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "hello" => {
                let resp = json!({
                    "protocol": 1,
                    "runtime": "heav3ns-micro",
                    "workspace": ws_id,
                    "session": sess_id,
                    "path": engine.workspace_root().to_string_lossy(),
                    "limits": {
                        "queuedInstructions": 3,
                        "instructionLength": 16000
                    },
                    "capabilities": [
                        "native",
                        "workspace-branch-v1",
                        "binary-inspection-v1",
                        "autonomous-execution-v1",
                        "snapshot",
                        "workspace-verification-v1",
                        "task-continuation-v1",
                        "queue:3",
                        "cancel",
                        "approval",
                        "history",
                        "files",
                        "file-search-v1",
                        "changes",
                        "paged-inspection",
                        "workspace-stop",
                        "run-details",
                        "run-insights-v1",
                        "runtime-state-v1",
                        "agent-swarm-defer-v1"
                    ]
                });
                if is_rpc {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
                } else {
                    println!("{}", json!({ "id": req_id, "type": "result", "result": resp }));
                }
            }
            "snapshot" => {
                let active_id = session_runs
                    .iter()
                    .rev()
                    .find(|r| {
                        let st = r.get("state").and_then(|s| s.as_str()).unwrap_or("");
                        st == "running" || st == "queued"
                    })
                    .and_then(|r| r.get("id"))
                    .cloned();

                let resp = json!({
                    "runtime": "heav3ns-micro",
                    "workspace": ws_id,
                    "session": sess_id,
                    "conversation": current_conversation,
                    "seq": run_sequence,
                    "runs": session_runs,
                    "modelSelection": {
                        "provider": "openai-codex",
                        "model": "gpt-5.6-terra",
                        "revision": 1,
                        "reasoningEffort": "medium",
                        "reasoningOptions": ["low", "medium", "high"],
                        "busy": false,
                        "blocked": null
                    },
                    "active": active_id,
                    "queued": []
                });
                if is_rpc {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
                } else {
                    println!("{}", json!({ "id": req_id, "type": "result", "result": resp }));
                }
            }
            "models.list" | "models.select" => {
                let resp = json!({
                    "provider": "openai-codex",
                    "model": "gpt-5.6-terra",
                    "revision": 1,
                    "reasoningEffort": "medium",
                    "reasoningOptions": ["low", "medium", "high"],
                    "busy": false,
                    "blocked": null,
                    "providers": [{
                        "id": "openai-codex",
                        "label": "OpenAI Codex",
                        "detail": "Standard reasoning and code orchestration engine",
                        "models": ["gpt-5.6-terra", "gpt-5-turbo", "o3-mini"]
                    }],
                    "recent": []
                });
                if is_rpc {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
                } else {
                    println!("{}", json!({ "id": req_id, "type": "result", "result": resp }));
                }
            }
            "models.discover" => {
                let resp = json!({
                    "status": "ready",
                    "models": ["gpt-5.6-terra", "gpt-5-turbo", "o3-mini"],
                    "freshness": now_ms()
                });
                if is_rpc {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
                } else {
                    println!("{}", json!({ "id": req_id, "type": "result", "result": resp }));
                }
            }
            "workspace.branch" => {
                let branch_name = "branch:main";
                if is_rpc {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "branch": branch_name } }));
                } else {
                    println!("{}", json!({ "id": req_id, "type": "result", "branch": branch_name }));
                }
            }
            "task.submit" => {
                let prompt = params
                    .get("text")
                    .or_else(|| parsed.get("prompt"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                run_sequence += 1;
                let run_id = format!("run_{}_{}", now_ms(), run_sequence);
                let now = now_ms() as f64;

                let mut run_obj = json!({
                    "id": run_id,
                    "text": prompt,
                    "session": sess_id,
                    "state": "running",
                    "createdAt": now,
                    "startedAt": now,
                    "finishedAt": null,
                    "activities": [],
                    "activitySequence": 0,
                    "activityCount": 0
                });

                if is_rpc {
                    println!(
                        "{}",
                        json!({
                            "v": 1,
                            "kind": "response",
                            "id": req_id,
                            "result": {
                                "accepted": true,
                                "run": run_obj
                            }
                        })
                    );
                    println!(
                        "{}",
                        json!({
                            "v": 1,
                            "kind": "event",
                            "workspace": ws_id,
                            "session": sess_id,
                            "runtime": "heav3ns-micro",
                            "seq": run_sequence,
                            "type": "run.state",
                            "payload": run_obj
                        })
                    );
                    let _ = io::stdout().flush();
                }

                let token = active_cancel.get_or_insert_with(CancellationToken::new).clone();
                let mut input = EngineTickInput::new(prompt.clone());
                input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
                input.cancel_token = Some(token);

                let req_id_prog = req_id.clone();
                let ws_prog = ws_id.clone();
                let sess_prog = sess_id.clone();
                let run_id_prog = run_id.clone();
                let seq_prog = run_sequence;

                input.on_progress = Some(Arc::new(move |ev| {
                    if is_rpc {
                        println!(
                            "{}",
                            json!({
                                "v": 1,
                                "kind": "event",
                                "workspace": ws_prog,
                                "session": sess_prog,
                                "runtime": "heav3ns-micro",
                                "seq": seq_prog,
                                "type": "run.progress",
                                "payload": {
                                    "id": run_id_prog,
                                    "event": ev
                                }
                            })
                        );
                    } else {
                        println!(
                            "{}",
                            json!({
                                "id": req_id_prog,
                                "type": "progress",
                                "event": ev
                            })
                        );
                    }
                    let _ = io::stdout().flush();
                }));

                let result = engine.tick(input).await;
                active_cancel = None;
                let finish_now = now_ms() as f64;

                let (final_state, response_text, error_text) = match result.outcome {
                    EngineTickOutcome::Completed => ("completed", Some(result.response.clone()), None),
                    EngineTickOutcome::Deferred => ("awaiting_approval", Some(result.response.clone()), None),
                    EngineTickOutcome::Cancelled => ("cancelled", None, Some("Execution was cancelled".to_string())),
                    EngineTickOutcome::Failed => ("failed", None, Some(result.response.clone())),
                };

                run_obj["state"] = json!(final_state);
                run_obj["finishedAt"] = json!(finish_now);
                if let Some(r) = response_text {
                    run_obj["response"] = json!(r);
                }
                if let Some(e) = error_text {
                    run_obj["error"] = json!(e);
                }

                session_runs.push(run_obj.clone());
                if session_runs.len() > 64 {
                    session_runs.remove(0);
                }

                if is_rpc {
                    run_sequence += 1;
                    println!(
                        "{}",
                        json!({
                            "v": 1,
                            "kind": "event",
                            "workspace": ws_id,
                            "session": sess_id,
                            "runtime": "heav3ns-micro",
                            "seq": run_sequence,
                            "type": "run.state",
                            "payload": run_obj
                        })
                    );
                } else {
                    println!("{}", json!({ "id": req_id, "type": "result", "result": result }));
                }
            }
            "task.cancel" | "task.cancelAll" => {
                if let Some(ref tok) = active_cancel {
                    tok.cancel();
                }
                if is_rpc {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "requested": true } }));
                } else {
                    println!("{}", json!({ "id": req_id, "type": "result", "cancelled": true }));
                }
            }
            "task.lookup" => {
                let lookup_id = params.get("id").and_then(|v| v.as_str());
                let found_run = lookup_id.and_then(|lid| session_runs.iter().find(|r| r.get("id").and_then(|s| s.as_str()) == Some(lid)));
                if let Some(r) = found_run {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "found": true, "run": r } }));
                } else {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "found": false } }));
                }
            }
            "history.list" => {
                if is_rpc {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "runs": session_runs } }));
                } else {
                    println!("{}", json!({ "id": req_id, "type": "result", "runs": session_runs }));
                }
            }
            "conversation.list" => {
                let convs = vec![json!({ "id": current_conversation, "title": "Main Session" })];
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversations": convs } }));
            }
            "conversation.select" | "conversation.create" => {
                if let Some(cid) = params.get("conversation").and_then(|v| v.as_str()) {
                    current_conversation = cid.to_string();
                }
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": current_conversation } }));
            }
            "conversation.delete" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": current_conversation } }));
            }
            "activity.list" => {
                let run_id_arg = params.get("run").and_then(|v| v.as_str()).unwrap_or("");
                println!("{}", json!({
                    "v": 1,
                    "kind": "response",
                    "id": req_id,
                    "result": {
                        "run": run_id_arg,
                        "activities": [],
                        "activitySequence": 0,
                        "activityCount": 0
                    }
                }));
            }
            "files.list" => {
                let entries = vec![
                    json!({ "path": "Cargo.toml", "type": "file" }),
                    json!({ "path": "src", "type": "directory" }),
                    json!({ "path": "README.md", "type": "file" })
                ];
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "entries": entries } }));
            }
            "files.search" => {
                println!("{}", json!({
                    "v": 1,
                    "kind": "response",
                    "id": req_id,
                    "result": {
                        "entries": [],
                        "matched": 0,
                        "more": false,
                        "scanLimited": false,
                        "scope": "workspace"
                    }
                }));
            }
            "changes.list" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "entries": [] } }));
            }
            "checkpoint.list" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "checkpoints": [], "more": false } }));
            }
            "project.discover" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "targets": [] } }));
            }
            "project.status" | "project.healing" => {
                println!("{}", json!({
                    "v": 1,
                    "kind": "response",
                    "id": req_id,
                    "result": {
                        "state": "idle",
                        "output": "",
                        "healing": { "enabled": true }
                    }
                }));
            }
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
                if is_rpc {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "stopped": true } }));
                } else {
                    println!("{}", json!({ "id": req_id, "type": "shutdown" }));
                }
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
                if is_rpc {
                    println!(
                        "{}",
                        json!({
                            "v": 1,
                            "kind": "response",
                            "id": req_id,
                            "result": { "status": "ok", "unhandled": other }
                        })
                    );
                } else {
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
        }
        let _ = io::stdout().flush();
    }
}
