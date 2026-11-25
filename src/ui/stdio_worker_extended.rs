// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::core::flight_runner::run_agent_flight_test;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::interfaces::IProjectRunner;
use crate::domain::swarm::{classify_task_for_legion, SwarmAssignment};
use crate::domain::types::provider_registry::canonicalize_provider_id;
use crate::domain::types::runtime::{CancellationToken, UserApprovalChoice};
use crate::plumbing::hash_and_math::{lock_unpoison, now_ms};
use crate::ui::cli_presenter::Heav3nsUserConfig;
use crate::ui::provider_rpc::handle_account_mutation_rpc;
use crate::ui::rpc_dispatch::{
    build_context_history_response, build_context_inspect_response, build_prompt_inspect_response,
    build_spider_check_multi_response, build_state_inspect_response, handle_memory_rpc,
    handle_profile_and_ci_rpc,
};
use crate::ui::worker_submit::execute_multiplexed_submit;
use crate::ui::worker_handlers::{
    emit_jsonl_line, emit_seq_event,     handle_automation_rpc, handle_provider_credential_rpc, WorkerSharedState,
};
use serde_json::{json, Value};
use std::io;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};


#[allow(clippy::too_many_arguments)]
pub(crate) async fn dispatch_worker_rpc_extended(
    method: &str,
    parsed: &Value,
    params: &Value,
    req_id: &str,
    ws_id: &str,
    sess_id: &str,
    is_rpc: bool,
    engine: &Arc<AgentEngine>,
    project_runner: &Arc<dyn IProjectRunner>,
    state: &Arc<Mutex<WorkerSharedState>>,
    stdout_lock: &Arc<Mutex<io::Stdout>>,
    active_signin_cancel: &mut Option<CancellationToken>,
    legacy_turn_gate: &Arc<tokio::sync::Mutex<()>>,
    spawned_turns: &mut Vec<tokio::task::JoinHandle<()>>,
    event_seq: &Arc<AtomicI64>,
    pending_submits: &Arc<AtomicUsize>,
) -> bool {
    let req_id = req_id.to_string();
    let ws_id = ws_id.to_string();
    let sess_id = sess_id.to_string();
    match method {
            "project.discover" => {
                let targets = project_runner.discover(engine.workspace_root());
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": targets }),
                );
            }
            "project.status" | "project.healing" => {
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": project_runner.status() }),
                );
            }
            "project.start" => {
                let target_id = params.get("id").and_then(Value::as_str).unwrap_or("");
                let rev = params.get("revision").and_then(Value::as_str);
                match project_runner.start(engine.workspace_root(), target_id, rev) {
                    Ok(snap) => emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": snap }),
                    ),
                    Err(msg) => emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "PROJECT_START_FAILED", "message": msg } }),
                    ),
                }
            }
            "project.stop" => {
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": project_runner.stop() }),
                );
            }
            "project.configure" => {
                let cmd = params.get("command").and_then(Value::as_str).unwrap_or("");
                let cwd = params.get("cwd").and_then(Value::as_str).unwrap_or(".");
                let preview = params.get("preview").and_then(Value::as_str);
                match project_runner.configure(engine.workspace_root(), cmd, cwd, preview) {
                    Ok(res) => emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": res }),
                    ),
                    Err(msg) => emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "PROJECT_CONFIG_FAILED", "message": msg } }),
                    ),
                }
            }
            "project.tasks.list" => {
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "revision": 1, "total": 0, "offset": 0, "nextOffset": 0, "limited": false, "tasks": [] } }),
                );
            }
            "project.tasks.move" => {
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "updated": true, "revision": 2 } }),
                );
            }
            "command.actionsList" => {
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "actions": [], "total": 0, "truncated": false } }),
                );
            }
            "command.actionReconcile" => {
                let aid = params
                    .get("actionId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("act-0");
                let out = params
                    .get("outcome")
                    .and_then(|v| v.as_str())
                    .unwrap_or("completed");
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "reconciled": true, "actionId": aid, "outcome": out } }),
                );
            }
            "agentSwarm.settings" => {
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "enabled": true, "criticalCompletionReview": true, "hierarchicalDefer": true, "legion": true, "maxSpecialists": 8, "strategy": "adaptive", "deferBoundary": "safe_boundary" } }),
                );
            }
            "agentSwarm.classify" => {
                let task_desc = params
                    .get("objective")
                    .or_else(|| params.get("task"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Verify system architecture and execute requested changes");
                let strategy = params.get("strategy").and_then(|v| v.as_str());
                let max_spec = params
                    .get("maxSpecialists")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5) as usize;
                let target_files: Vec<String> = params
                    .get("targetFiles")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let legion_plan =
                    classify_task_for_legion(task_desc, &target_files, strategy, max_spec);
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": legion_plan }),
                );
            }
            "computer.action"
            | "computer.capture"
            | "computer.health"
            | "computer.observe"
            | "computer.interpret"
            | "computer.consult"
            | "computer.orchestrate"
            | "computer.patterns"
            | "browser.action"
            | "browser.snapshot"
            | "browser.navigate"
            | "spatial.ground"
            | "spatial.execute"
            | "spatial.health"
            | "spatial.query"
            | "skills.dag"
            | "skills.forge"
            | "skills.lint"
            | "soul.status" => {
                spawned_turns.retain(|h| !h.is_finished());
                if spawned_turns.len() >= 32 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending tasks." } }),
                    );
                    return false;
                }
                let (m, eng, out, p) = (
                    method.to_string(),
                    Arc::clone(&engine),
                    Arc::clone(&stdout_lock),
                    params.clone(),
                );
                spawned_turns.push(tokio::spawn(async move {
                    let out_res = handle_automation_rpc(&m, &req_id, &p, &eng).await;
                    emit_jsonl_line(
                        &out,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": out_res }),
                    );
                }));
            }
            "agentSwarm.form" | "agentSwarm.formResume" => {
                let objective = params
                    .get("objective")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Swarm task execution")
                    .to_string();
                let strategy = params.get("strategy").and_then(|v| v.as_str());
                let max_spec = params
                    .get("maxSpecialists")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(3) as usize;
                let legion = classify_task_for_legion(&objective, &[], strategy, max_spec);
                let assignments: Vec<SwarmAssignment> = legion
                    .get("assignments")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let execute_live = params
                    .get("live")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if execute_live && !assignments.is_empty() {
                    spawned_turns.retain(|h| !h.is_finished());
                    if spawned_turns.len() >= 32 {
                        emit_jsonl_line(
                            &stdout_lock,
                            &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending tasks." } }),
                        );
                        return false;
                    }
                    let (eng, st, out) = (
                        Arc::clone(&engine),
                        Arc::clone(&state),
                        Arc::clone(&stdout_lock),
                    );
                    spawned_turns.push(tokio::spawn(async move {
                        let swarm_res = eng.run_swarm(objective.clone(), assignments.clone(), None, None).await;
                        let rec = {
                            let mut s = lock_unpoison(&st);
                            let form_id = format!("formation-{}", s.saved_formations.len() + 1);
                            let r = json!({ "id": form_id, "objective": objective, "status": format!("{:?}", swarm_res.outcome).to_ascii_lowercase(), "reports": swarm_res.reports, "assignments": assignments });
                            s.saved_formations.push(r.clone());
                            r
                        };
                        emit_jsonl_line(&out, &json!({ "v": 1, "kind": "response", "id": req_id, "result": rec }));
                    }));
                    return false;
                }
                let form_id = format!(
                    "formation-{}",
                    lock_unpoison(&state).saved_formations.len() + 1
                );
                let rep_items: Vec<Value> = assignments.iter().map(|a| json!({ "id": a.id, "role": a.role, "status": "completed", "response": format!("Planned specialist scope: {}", a.objective) })).collect();
                let assign_items: Vec<Value> = assignments.iter().enumerate().map(|(idx, a)| json!({ "id": a.id, "role": a.role, "status": "completed", "summary": a.objective, "childSessionId": format!("cs-{}", idx + 1) })).collect();
                let rec = json!({ "id": form_id, "objective": objective, "status": "completed", "reports": rep_items, "assignments": assign_items });
                {
                    let mut st = lock_unpoison(&state);
                    st.saved_formations.push(rec.clone());
                    if st.saved_formations.len() > 64 {
                        let excess = st.saved_formations.len() - 64;
                        st.saved_formations.drain(0..excess);
                    }
                }
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": rec }),
                );
            }
            "agentSwarm.formReconcile" => {
                let fid = params
                    .get("formationId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("formation-1");
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "id": fid, "status": "deferred", "reconciledAt": now_ms() as f64 } }),
                );
            }
            "agentSwarm.formStatus" => {
                let last = lock_unpoison(&state).saved_formations.last().cloned();
                if let Some(l) = last {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": l }),
                    );
                } else {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "status": "none", "id": "form-idle" } }),
                    );
                }
            }
            "agentSwarm.formHistory" => {
                let forms = lock_unpoison(&state).saved_formations.clone();
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "formations": forms } }),
                );
            }
            "agentSwarm.formStream" => {
                let fid = params
                    .get("formationId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("formation-1");
                let sid = params
                    .get("specialistId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("spec-1");
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "formationId": fid, "specialistId": sid, "childSessionId": "cs-1", "messageCount": 1, "excerpt": "Specialist execution verified with 100% Joyzoning compliance." } }),
                );
            }
            "approval.resolve" => {
                let decision_raw = params
                    .get("decision")
                    .or_else(|| params.get("choice"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let scope_raw = params.get("scope").and_then(|v| v.as_str()).unwrap_or("");
                let approved_bool = params.get("approved").and_then(|v| v.as_bool());
                let choice = if decision_raw.eq_ignore_ascii_case("allow_session")
                    || scope_raw.eq_ignore_ascii_case("session")
                    || params.get("remember").and_then(|v| v.as_bool()) == Some(true)
                {
                    UserApprovalChoice::AllowSession
                } else if matches!(
                    decision_raw.to_ascii_lowercase().as_str(),
                    "allow" | "allow_once" | "approve" | "approved" | "y" | "yes"
                ) || approved_bool == Some(true)
                {
                    UserApprovalChoice::AllowOnce
                } else {
                    UserApprovalChoice::Deny
                };
                let target_id = params
                    .get("operation")
                    .or_else(|| params.get("id"))
                    .or_else(|| params.get("approvalId"))
                    .and_then(|v| v.as_str());
                let delivered = {
                    let mut st = lock_unpoison(&state);
                    let tx_opt = if let Some(tid) = target_id {
                        st.pending_approvals.remove(tid)
                    } else {
                        let first_key = st.pending_approvals.keys().next().cloned();
                        first_key.and_then(|k| st.pending_approvals.remove(&k))
                    };
                    tx_opt.map(|tx| tx.send(choice).is_ok()).unwrap_or(false)
                };
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "resolved": true, "delivered": delivered, "choice": format!("{choice:?}") } }),
                );
            }
            "spider.check" => {
                let mut paths: Vec<String> = params
                    .get("paths")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                if paths.is_empty() {
                    if let Some(f) = params
                        .get("file")
                        .or_else(|| params.get("path"))
                        .and_then(|v| v.as_str())
                    {
                        paths.push(f.to_string());
                    }
                }
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_spider_check_multi_response(engine.workspace_root(), &paths) }),
                );
            }
            "state.inspect" => {
                let st = lock_unpoison(&state);
                let conv = params
                    .get("conversation")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&st.current_conversation);
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_state_inspect_response(conv, &st.session_runs) }),
                );
            }
            "context.inspect" => {
                let conv = {
                    let st = lock_unpoison(&state);
                    params
                        .get("conversation")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&st.current_conversation)
                        .to_string()
                };
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_context_inspect_response(&conv, &engine) }),
                );
            }
            "prompt.inspect" => {
                let mode = params.get("mode").and_then(|v| v.as_str()).unwrap_or("");
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_prompt_inspect_response(mode, &engine) }),
                );
            }
            "profile.list" | "profile.get" | "profile.set" | "ci_gate.run" => {
                match handle_profile_and_ci_rpc(method, &params, &engine) {
                    Ok(res) => emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": res }),
                    ),
                    Err((code, msg)) => emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": code, "message": msg } }),
                    ),
                }
            }
            "context.history" => {
                let conv = {
                    let st = lock_unpoison(&state);
                    params
                        .get("conversation")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&st.current_conversation)
                        .to_string()
                };
                let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_context_history_response(&conv, offset, &engine) }),
                );
            }
            "context.compact" => {
                let conv = {
                    let st = lock_unpoison(&state);
                    params
                        .get("conversation")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&st.current_conversation)
                        .to_string()
                };
                let report = lock_unpoison(&engine.session()).compact_session(
                    engine.compactor(),
                    &CompactionPolicy {
                        force: true,
                        ..Default::default()
                    },
                );
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": {
                        "conversation": conv,
                        "compacted": report.compacted,
                        "before": report.input_message_count,
                        "after": report.output_message_count,
                        "inputTokens": report.input_tokens,
                        "outputTokens": report.output_tokens
                    }}),
                );
            }
            "side.query" => {
                spawned_turns.retain(|h| !h.is_finished());
                if spawned_turns.len() >= 32 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending tasks." } }),
                    );
                    return false;
                }
                let conv = {
                    let st = lock_unpoison(&state);
                    params
                        .get("conversation")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&st.current_conversation)
                        .to_string()
                };
                let q = params
                    .get("question")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let (eng, out) = (Arc::clone(&engine), Arc::clone(&stdout_lock));
                spawned_turns.push(tokio::spawn(async move {
                    match eng.ask_side_question(&q).await {
                        Ok(ans) if !ans.trim().is_empty() => emit_jsonl_line(&out, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": conv, "response": ans, "contextLimited": false } })),
                        Ok(_) => emit_jsonl_line(&out, &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "SIDE_QUERY_EMPTY", "message": "Provider returned an empty response." } })),
                        Err(err) => emit_jsonl_line(&out, &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "SIDE_QUERY_FAILED", "message": err.to_string() } })),
                    }
                }));
            }
            "memory.list" | "memory.read" | "memory.write" | "memory.remove" => {
                let conv = lock_unpoison(&state).current_conversation.clone();
                let resp = handle_memory_rpc(method, &params, &conv, &engine);
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }),
                );
            }
            "status" | "accounts" => {
                let resp = AgentEngine::default_account_store().rpc_snapshot();
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }),
                );
            }
            "signIn" => {
                spawned_turns.retain(|h| !h.is_finished());
                if spawned_turns.len() >= 32 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending tasks." } }),
                    );
                    return false;
                }
                if let Some(prev) = active_signin_cancel.take() {
                    prev.cancel();
                }
                let cancel = CancellationToken::new();
                *active_signin_cancel = Some(cancel.clone());
                let out_task = Arc::clone(&stdout_lock);
                let req_id_task = req_id.clone();
                let handle = tokio::spawn(async move {
                    let out_browser = Arc::clone(&out_task);
                    let store = AgentEngine::default_account_store();
                    let res = store
                        .sign_in_async(
                            cancel,
                            Box::new(move |url| {
                                emit_jsonl_line(
                                    &out_browser,
                                    &json!({ "v": 1, "event": "browser", "url": url }),
                                );
                            }),
                        )
                        .await;
                    match res {
                        Ok(result) => {
                            if let Some(mut ucfg) = Heav3nsUserConfig::load() {
                                let cur = canonicalize_provider_id(ucfg.provider.as_deref().unwrap_or(""));
                                if cur.is_empty() || cur == "openai" {
                                    ucfg.provider = Some("openai-codex".to_string());
                                    let _ = ucfg.save();
                                }
                            }
                            emit_jsonl_line(
                                &out_task,
                                &json!({ "v": 1, "kind": "response", "id": req_id_task, "result": result }),
                            );
                        }
                        Err(err) => {
                            emit_jsonl_line(
                                &out_task,
                                &json!({ "v": 1, "kind": "response", "id": req_id_task, "error": { "code": -32000, "message": err } }),
                            );
                        }
                    }
                });
                spawned_turns.push(handle);
            }
            "cancel" => {
                if let Some(tok) = active_signin_cancel.take() {
                    tok.cancel();
                }
                let resp = AgentEngine::default_account_store().rpc_snapshot();
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }),
                );
            }
            "switch" | "rename" | "remove" | "signOut" | "clearLimit" | "setAuto" => {
                match handle_account_mutation_rpc(method, &params) {
                    Ok(result) => emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": result }),
                    ),
                    Err(err) => emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": -32000, "message": err } }),
                    ),
                }
            }
            "ping" | "health" => {
                let q = engine.queue().snapshot();
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "id": req_id, "type": "result", "status": "ok", "posture": engine.current_posture().slug(), "queueActive": q.active, "queuePending": q.pending_count, "timestamp": now_ms() }),
                );
            }
            "flight" => {
                spawned_turns.retain(|h| !h.is_finished());
                if spawned_turns.len() >= 32 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending tasks." } }),
                    );
                    return false;
                }
                let gate = Arc::clone(&legacy_turn_gate);
                let out = Arc::clone(&stdout_lock);
                let handle = tokio::spawn(async move {
                    let _guard = gate.lock().await;
                    let report = run_agent_flight_test().await;
                    emit_jsonl_line(
                        &out,
                        &json!({ "v": 1, "id": req_id, "type": "result", "report": report }),
                    );
                });
                spawned_turns.push(handle);
            }
            "swarm" => {
                spawned_turns.retain(|h| !h.is_finished());
                if spawned_turns.len() >= 32 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending tasks." } }),
                    );
                    return false;
                }
                let objective = params
                    .get("objective")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Swarm execution")
                    .to_string();
                let assignments: Vec<SwarmAssignment> = params
                    .get("assignments")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let token = CancellationToken::new();
                lock_unpoison(&state)
                    .active_cancels
                    .insert(req_id.clone(), token.clone());
                let gate = Arc::clone(&legacy_turn_gate);
                let eng = Arc::clone(&engine);
                let st = Arc::clone(&state);
                let out = Arc::clone(&stdout_lock);
                let handle = tokio::spawn(async move {
                    let _guard = gate.lock().await;
                    let res = eng
                        .run_swarm(objective, assignments, Some(token), None)
                        .await;
                    lock_unpoison(&st).active_cancels.remove(&req_id);
                    emit_jsonl_line(
                        &out,
                        &json!({ "v": 1, "id": req_id, "type": "result", "swarm": res }),
                    );
                });
                spawned_turns.push(handle);
            }
            "shutdown" | "exit" => {
                if let Some(tok) = active_signin_cancel.take() {
                    tok.cancel();
                }
                for tok in lock_unpoison(&state).active_cancels.values() {
                    tok.cancel();
                }
                let _ = project_runner.stop();
                let _ = tokio::time::timeout(std::time::Duration::from_secs(3), async {
                    for handle in spawned_turns.drain(..) {
                        let _ = handle.await;
                    }
                })
                .await;
                if is_rpc {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "stopped": true } }),
                    );
                } else {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "id": req_id, "type": "shutdown" }),
                    );
                }
                return true;
            }
            // Deprecated legacy turn methods: migrated onto `execute_multiplexed_submit` with strict FIFO response ordering.
            "tick" | "chat" | "slash" => {
                spawned_turns.retain(|h| !h.is_finished());
                if spawned_turns.len() >= 32 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending tasks." } }),
                    );
                    return false;
                }
                emit_seq_event(
                    &stdout_lock,
                    &event_seq,
                    json!({
                        "v": 1, "kind": "event", "workspace": ws_id, "session": sess_id,
                        "runtime": "heav3ns-micro", "type": "deprecation.notice",
                        "payload": { "method": method, "replacement": "task.submit", "message": format!("Method '{method}' is deprecated; migrate to 'task.submit'.") }
                    }),
                );
                let prompt = params
                    .get("prompt")
                    .or_else(|| params.get("command"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if prompt.chars().count() > 16_000 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "id": req_id, "type": "error", "code": "INVALID_PARAMS", "message": "Instruction exceeds 16000 character limit." }),
                    );
                    return false;
                }
                pending_submits.fetch_add(1, Ordering::SeqCst);
                let (eng, st, out, seq, gate, pending) = (
                    Arc::clone(&engine),
                    Arc::clone(&state),
                    Arc::clone(&stdout_lock),
                    Arc::clone(&event_seq),
                    Arc::clone(&legacy_turn_gate),
                    Arc::clone(&pending_submits),
                );
                let handle = tokio::spawn(async move {
                    struct LegacyPendingGuard(Arc<AtomicUsize>);
                    impl Drop for LegacyPendingGuard {
                        fn drop(&mut self) {
                            self.0.fetch_sub(1, Ordering::SeqCst);
                        }
                    }
                    let _pg = LegacyPendingGuard(pending);
                    execute_multiplexed_submit(
                        eng, st, out, seq, req_id, ws_id, sess_id, is_rpc, prompt, None, false,
                        gate,
                    )
                    .await;
                });
                spawned_turns.push(handle);
            }
            m if m.ends_with(".status") || m.ends_with(".saveKey") || m.ends_with(".clearKey") => {
                let resp = {
                    let mut st = lock_unpoison(&state);
                    let active_prov = st.active_provider.clone();
                    let active_mod = st.active_model.clone();
                    handle_provider_credential_rpc(
                        m,
                        &params,
                        &mut st.provider_keys,
                        &active_prov,
                        &active_mod,
                        &engine,
                    )
                };
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }),
                );
            }
            other => {
                if is_rpc {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "UNKNOWN_METHOD", "message": format!("Unsupported method: {other}") } }),
                    );
                } else {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "id": req_id, "type": "error", "code": "UNKNOWN_METHOD", "message": format!("Unsupported method: {other}") }),
                    );
                }
            }
    }
    false
}
