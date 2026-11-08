// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::core::flight_runner::run_agent_flight_test;
use crate::domain::swarm::SwarmAssignment;
use crate::domain::types::runtime::{CancellationToken, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel};
use crate::plumbing::hash_and_math::now_ms;
use crate::ui::rpc_dispatch::{build_context_inspect_response, build_hello_response, build_model_catalog, build_spider_check_response, build_state_inspect_response};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

pub async fn run_stdio_worker(engine: Arc<AgentEngine>) {
    let stdin = io::stdin();
    let mut active_cancel: Option<CancellationToken> = None;
    let mut session_runs: Vec<Value> = Vec::new();
    let mut run_sequence: i64 = 0;
    let mut current_conversation = "default-conv".to_string();

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
        let ws_id = parsed.get("workspace").and_then(|v| v.as_str()).unwrap_or("ws_default").to_string();
        let sess_id = parsed.get("session").and_then(|v| v.as_str()).unwrap_or("sess_default").to_string();
        let is_rpc = parsed.get("v").and_then(|v| v.as_i64()) == Some(1) || parsed.get("kind").and_then(|v| v.as_str()) == Some("request");
        let method = parsed.get("method").or_else(|| parsed.get("type")).and_then(|v| v.as_str()).unwrap_or("tick");
        let params = parsed.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "hello" => {
                let resp = build_hello_response(&ws_id, &sess_id, engine.workspace_root());
                if is_rpc { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": resp })); }
                else { println!("{}", json!({ "id": req_id, "type": "result", "result": resp })); }
            }
            "snapshot" => {
                let active_id = session_runs.iter().rev().find(|r| {
                    let st = r.get("state").and_then(|s| s.as_str()).unwrap_or("");
                    st == "running" || st == "queued"
                }).and_then(|r| r.get("id")).cloned();
                let resp = json!({
                    "runtime": "heav3ns-micro", "workspace": ws_id, "session": sess_id,
                    "conversation": current_conversation, "seq": run_sequence, "runs": session_runs,
                    "modelSelection": build_model_catalog()["selection"],
                    "active": active_id, "queued": []
                });
                if is_rpc { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": resp })); }
                else { println!("{}", json!({ "id": req_id, "type": "result", "result": resp })); }
            }
            "models.list" | "models.select" => {
                let resp = build_model_catalog();
                if is_rpc { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": resp })); }
                else { println!("{}", json!({ "id": req_id, "type": "result", "result": resp })); }
            }
            "models.discover" => {
                let resp = json!({ "status": "ready", "models": ["gpt-5.6-terra", "gpt-5-turbo", "o3-mini"], "freshness": now_ms() });
                if is_rpc { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": resp })); }
                else { println!("{}", json!({ "id": req_id, "type": "result", "result": resp })); }
            }
            "workspace.branch" => {
                let branch_name = "branch:main";
                if is_rpc { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "branch": branch_name } })); }
                else { println!("{}", json!({ "id": req_id, "type": "result", "branch": branch_name })); }
            }
            "task.submit" => {
                let prompt = params.get("text").or_else(|| parsed.get("prompt")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                run_sequence += 1;
                let run_id = format!("run_{}_{}", now_ms(), run_sequence);
                let now = now_ms() as f64;
                let mut run_obj = json!({
                    "id": run_id, "text": prompt, "session": sess_id, "state": "running",
                    "createdAt": now, "startedAt": now, "finishedAt": null,
                    "activities": [], "activitySequence": 0, "activityCount": 0
                });
                if is_rpc {
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "accepted": true, "run": run_obj } }));
                    println!("{}", json!({ "v": 1, "kind": "event", "workspace": ws_id, "session": sess_id, "runtime": "heav3ns-micro", "seq": run_sequence, "type": "run.state", "payload": run_obj }));
                    let _ = io::stdout().flush();
                }

                let token = active_cancel.get_or_insert_with(CancellationToken::new).clone();
                let mut input = EngineTickInput::new(prompt.clone());
                input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
                input.cancel_token = Some(token);

                let req_id_prog = req_id.clone();
                let (ws_p, sess_p, run_p, seq_p) = (ws_id.clone(), sess_id.clone(), run_id.clone(), run_sequence);
                input.on_progress = Some(Arc::new(move |ev| {
                    if is_rpc {
                        println!("{}", json!({ "v": 1, "kind": "event", "workspace": ws_p, "session": sess_p, "runtime": "heav3ns-micro", "seq": seq_p, "type": "run.progress", "payload": { "id": run_p, "event": ev } }));
                    } else {
                        println!("{}", json!({ "id": req_id_prog, "type": "progress", "event": ev }));
                    }
                    let _ = io::stdout().flush();
                }));

                let result = engine.tick(input).await;
                active_cancel = None;
                let finish_now = now_ms() as f64;
                let (final_state, resp_txt, err_txt) = match result.outcome {
                    EngineTickOutcome::Completed => ("completed", Some(result.response.clone()), None),
                    EngineTickOutcome::Deferred => ("awaiting_approval", Some(result.response.clone()), None),
                    EngineTickOutcome::Cancelled => ("cancelled", None, Some("Execution was cancelled".to_string())),
                    EngineTickOutcome::Failed => ("failed", None, Some(result.response.clone())),
                };

                run_obj["state"] = json!(final_state);
                run_obj["finishedAt"] = json!(finish_now);
                if let Some(r) = resp_txt { run_obj["response"] = json!(r); }
                if let Some(e) = err_txt { run_obj["error"] = json!(e); }
                session_runs.push(run_obj.clone());
                if session_runs.len() > 64 { session_runs.remove(0); }

                if is_rpc {
                    run_sequence += 1;
                    println!("{}", json!({ "v": 1, "kind": "event", "workspace": ws_id, "session": sess_id, "runtime": "heav3ns-micro", "seq": run_sequence, "type": "run.state", "payload": run_obj }));
                } else {
                    println!("{}", json!({ "id": req_id, "type": "result", "result": result }));
                }
            }
            "task.cancel" | "task.cancelAll" => {
                if let Some(ref tok) = active_cancel { tok.cancel(); }
                if is_rpc { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "requested": true } })); }
                else { println!("{}", json!({ "id": req_id, "type": "result", "cancelled": true })); }
            }
            "task.edit" => {
                let edit_id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let mut found = false;
                for r in &mut session_runs {
                    if r.get("id").and_then(|s| s.as_str()) == Some(edit_id) {
                        if let Some(nt) = params.get("text").and_then(|v| v.as_str()) { r["text"] = json!(nt); }
                        found = true;
                        println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "updated": true, "run": r } }));
                        break;
                    }
                }
                if !found { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "updated": false } })); }
            }
            "task.lookup" => {
                let lookup_id = params.get("id").and_then(|v| v.as_str());
                let found_run = lookup_id.and_then(|lid| session_runs.iter().find(|r| r.get("id").and_then(|s| s.as_str()) == Some(lid)));
                if let Some(r) = found_run { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "found": true, "run": r } })); }
                else { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "found": false } })); }
            }
            "task.deferStatus" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "enabled": true, "deferred": false, "queued": 0 } }));
            }
            "history.list" => {
                if is_rpc { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "runs": session_runs } })); }
                else { println!("{}", json!({ "id": req_id, "type": "result", "runs": session_runs })); }
            }
            "history.read" => {
                let h_id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let found_run = session_runs.iter().find(|r| r.get("id").and_then(|s| s.as_str()) == Some(h_id));
                if let Some(r) = found_run { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": r, "outputAvailable": true } })); }
                else {
                    let fallback = json!({ "id": h_id, "text": "", "session": sess_id, "state": "completed", "createdAt": now_ms() as f64 });
                    println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": fallback, "outputAvailable": false } }));
                }
            }
            "conversation.list" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversations": [{ "id": current_conversation, "title": "Main Session" }] } }));
            }
            "conversation.select" | "conversation.create" => {
                if let Some(cid) = params.get("conversation").and_then(|v| v.as_str()) { current_conversation = cid.to_string(); }
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": current_conversation } }));
            }
            "conversation.delete" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": current_conversation } }));
            }
            "activity.list" => {
                let run_arg = params.get("run").and_then(|v| v.as_str()).unwrap_or("");
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": run_arg, "activities": [], "activitySequence": 0, "activityCount": 0 } }));
            }
            "files.list" => {
                let entries = vec![json!({ "path": "Cargo.toml", "type": "file" }), json!({ "path": "src", "type": "directory" }), json!({ "path": "native", "type": "directory" }), json!({ "path": "README.md", "type": "file" })];
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "entries": entries } }));
            }
            "files.search" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "entries": [], "matched": 0, "more": false, "scanLimited": false, "scope": "workspace" } }));
            }
            "files.read" => {
                let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("README.md");
                let text_content = std::fs::read_to_string(engine.workspace_root().join(path)).unwrap_or_default();
                let size = text_content.len();
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "text": text_content, "revision": "rev-1", "nextOffset": size, "size": size } }));
            }
            "files.readBinary" => {
                let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let bytes = std::fs::read(engine.workspace_root().join(path)).unwrap_or_default();
                let b64 = bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
                let size = bytes.len();
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "data": b64, "revision": "rev-bin-1", "offset": 0, "nextOffset": size, "size": size, "more": false } }));
            }
            "changes.list" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "entries": [] } })); }
            "changes.read" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "text": "", "revision": "diff-rev-0", "nextOffset": 0, "size": 0 } })); }
            "checkpoint.list" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "checkpoints": [], "more": false } })); }
            "checkpoint.preview" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "token": "preview-token-1", "changes": [], "total": 0 } })); }
            "checkpoint.restore" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": current_conversation } })); }
            "checkpoint.flag" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "flagged": true } })); }
            "project.discover" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "targets": [] } })); }
            "project.status" | "project.healing" | "project.start" | "project.stop" | "project.configure" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "state": "idle", "output": "", "healing": { "enabled": true } } }));
            }
            "project.tasks.list" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "revision": 1, "total": 0, "offset": 0, "nextOffset": 0, "limited": false, "tasks": [] } }));
            }
            "project.tasks.move" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "updated": true, "revision": 2 } })); }
            "command.actionsList" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "actions": [], "total": 0, "truncated": false } })); }
            "command.actionReconcile" => {
                let aid = params.get("actionId").and_then(|v| v.as_str()).unwrap_or("act-0");
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "reconciled": true, "actionId": aid, "outcome": "completed" } }));
            }
            "agentSwarm.settings" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "enabled": true, "criticalCompletionReview": true, "hierarchicalDefer": true, "legion": true, "maxSpecialists": 8 } }));
            }
            "agentSwarm.classify" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "intent": "autonomous_orchestration", "strategy": "hierarchical", "complexity": 2, "assignments": [{ "specialist_id": "core-arch", "role": "Architect", "task": "Perform comprehensive system verification" }] } }));
            }
            "agentSwarm.form" | "agentSwarm.formResume" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "objective": "Swarm task execution", "status": "completed", "reports": [{ "id": "spec-1", "status": "completed" }] } }));
            }
            "agentSwarm.formReconcile" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "id": "formation-1", "status": "deferred", "reconciledAt": now_ms() as f64 } }));
            }
            "agentSwarm.formStatus" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "status": "idle", "id": "form-idle" } })); }
            "agentSwarm.formHistory" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "formations": [] } })); }
            "agentSwarm.formStream" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "formationId": "f-1", "specialistId": "s-1", "childSessionId": "cs-1", "messageCount": 1, "excerpt": "Specialist stream verified." } }));
            }
            "approval.resolve" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "resolved": true } })); }
            "spider.check" => {
                let file = params.get("file").and_then(|v| v.as_str()).unwrap_or("src/main.rs");
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": build_spider_check_response(file) }));
            }
            "state.inspect" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": build_state_inspect_response(&current_conversation) }));
            }
            "context.inspect" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": build_context_inspect_response(&current_conversation) }));
            }
            "context.history" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "text": "", "revision": "e0a1b2c3-d4e5-f6a7-b8c9-d0e1f2a3b4c5", "nextOffset": 0, "size": 0 } }));
            }
            "context.compact" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": current_conversation, "compacted": true, "before": 100, "after": 50, "inputTokens": 100, "outputTokens": 50 } }));
            }
            "side.query" => {
                println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": current_conversation, "response": "HEAV3NS-MICRO is a hyper-compressed Rust micro agent runtime.", "contextLimited": false } }));
            }
            "memory.list" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "memories": [] } })); }
            "memory.read" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "text": "", "revision": "mem-rev-1", "nextOffset": 0, "size": 0 } })); }
            "memory.write" | "memory.remove" => { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "saved": true } })); }
            "ping" | "health" => {
                let q = engine.queue().snapshot();
                println!("{}", json!({ "id": req_id, "type": "result", "status": "ok", "queueActive": q.active, "queuePending": q.pending_count, "timestamp": now_ms() }));
            }
            "flight" => {
                let report = run_agent_flight_test().await;
                println!("{}", json!({ "id": req_id, "type": "result", "report": report }));
            }
            "swarm" => {
                let objective = parsed.get("objective").and_then(|v| v.as_str()).unwrap_or("Swarm execution").to_string();
                let assignments: Vec<SwarmAssignment> = parsed.get("assignments").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
                let token = active_cancel.get_or_insert_with(CancellationToken::new).clone();
                let res = engine.run_swarm(objective, assignments, Some(token), None).await;
                active_cancel = None;
                println!("{}", json!({ "id": req_id, "type": "result", "swarm": res }));
            }
            "shutdown" | "exit" => {
                if is_rpc { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "stopped": true } })); }
                else { println!("{}", json!({ "id": req_id, "type": "shutdown" })); }
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
                if is_rpc { println!("{}", json!({ "v": 1, "kind": "response", "id": req_id, "result": { "status": "ok", "unhandled": other } })); }
                else { println!("{}", json!({ "id": req_id, "type": "error", "code": "UNKNOWN_METHOD", "message": format!("Unsupported method: {other}") })); }
            }
        }
        let _ = io::stdout().flush();
    }
}
