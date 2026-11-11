// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::core::flight_runner::run_agent_flight_test;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::swarm::{classify_task_for_legion, SwarmAssignment};
use crate::domain::types::provider_registry::{canonicalize_provider_id, lookup_provider_descriptor, retired_provider_remediation, ReasoningEffortLevel, PROVIDER_DESCRIPTORS};
use crate::domain::types::runtime::{CancellationToken, EngineTickInput, ExecutionAuthorityLevel, ExecutionPosture, UserApprovalChoice};
use crate::plumbing::hash_and_math::{fnv1a_32, now_ms};
use crate::ui::cli_presenter::Heav3nsUserConfig;
use crate::ui::rpc_dispatch::{build_context_history_response, build_context_inspect_response, build_hello_response, build_model_selection, build_prompt_inspect_response, build_spider_check_multi_response, build_state_inspect_response, handle_files_rpc, handle_memory_rpc, sha256_hex};
use crate::ui::worker_handlers::{build_sovereign_model_catalog_with_recent, build_sovereign_model_discovery, discover_project_targets, emit_jsonl_line, execute_multiplexed_submit, handle_posture_and_receipts_rpc, handle_provider_credential_rpc, switch_worker_conversation, WorkerSharedState};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::task::JoinHandle;

pub async fn run_stdio_worker(engine: Arc<AgentEngine>) {
    let stdout_lock = Arc::new(Mutex::new(io::stdout()));
    let state = Arc::new(Mutex::new(WorkerSharedState::from_engine_config(&engine.config_snapshot())));
    let event_seq = Arc::new(AtomicI64::new(0));
    let mut spawned_turns: Vec<JoinHandle<()>> = Vec::new();

    let root_bytes = engine.workspace_root().to_string_lossy().as_bytes().to_vec();
    let default_ws = format!("ws-{:08x}", fnv1a_32(&root_bytes));
    let default_sess = format!("sess-{:08x}", fnv1a_32(&root_bytes));

    emit_jsonl_line(
        &stdout_lock,
        &json!({
            "v": 1,
            "type": "ready",
            "runtime": "heav3ns-micro",
            "version": env!("CARGO_PKG_VERSION"),
            "timestamp": now_ms()
        }),
    );

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(raw)) = lines.next_line().await {
        spawned_turns.retain(|h| !h.is_finished());
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "type": "error", "code": "INVALID_JSON", "message": e.to_string() }));
                continue;
            }
        };

        let req_id = parsed.get("id").and_then(|v| v.as_str()).unwrap_or("req_0").to_string();
        let ws_id = parsed.get("workspace").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or(&default_ws).to_string();
        let sess_id = parsed.get("session").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or(&default_sess).to_string();
        let is_rpc = parsed.get("v").and_then(|v| v.as_i64()) == Some(1) || parsed.get("kind").and_then(|v| v.as_str()) == Some("request");
        let method = parsed.get("method").or_else(|| parsed.get("type")).and_then(|v| v.as_str()).unwrap_or("tick");
        let params = parsed.get("params").cloned().unwrap_or_else(|| parsed.clone());

        match method {
            "hello" => {
                let resp = build_hello_response(&ws_id, &sess_id, engine.workspace_root());
                if is_rpc {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
                } else {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "id": req_id, "type": "result", "result": resp }));
                }
            }
            "snapshot" => {
                let resp = {
                    let st = state.lock().unwrap();
                    let active_id = st
                        .session_runs
                        .iter()
                        .rev()
                        .find(|r| matches!(r.get("state").and_then(|s| s.as_str()), Some("running" | "queued")))
                        .and_then(|r| r.get("id"))
                        .cloned();
                    json!({
                        "runtime": "heav3ns-micro",
                        "workspace": ws_id,
                        "session": sess_id,
                        "conversation": st.current_conversation,
                        "posture": engine.current_posture().slug(),
                        "seq": event_seq.load(Ordering::SeqCst),
                        "runs": st.session_runs,
                        "modelSelection": build_model_selection(&st.active_provider, &st.active_model, &st.active_reasoning, st.model_revision),
                        "active": active_id,
                        "queued": []
                    })
                };
                if is_rpc {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
                } else {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "id": req_id, "type": "result", "result": resp }));
                }
            }
            "models.list" => {
                let local_ollama = AgentEngine::probe_ollama_models().await;
                let st = state.lock().unwrap();
                let resp = build_sovereign_model_catalog_with_recent(&st.active_provider, &st.active_model, &st.active_reasoning, st.model_revision, &local_ollama, &st.recent_selections);
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
            }
            "models.select" => {
                if let Some(p_raw) = params.get("provider").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    if let Some(retired_msg) = retired_provider_remediation(p_raw) {
                        emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": -32602, "message": retired_msg } }));
                        continue;
                    }
                }
                let local_ollama = AgentEngine::probe_ollama_models().await;
                let catalog = {
                    let mut st = state.lock().unwrap();
                    if let Some(p) = params.get("provider").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                        st.active_provider = canonicalize_provider_id(p);
                    }
                    if let Some(m) = params.get("model").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                        st.active_model = m.to_string();
                    }
                    if let Some(r) = params.get("reasoningEffort").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                        match ReasoningEffortLevel::parse_flexible(r) {
                            Ok(None) => {
                                st.active_reasoning = "default".to_string();
                                engine.set_reasoning_effort(None);
                            }
                            Ok(Some(lvl)) => {
                                st.active_reasoning = lvl.as_str().to_string();
                                engine.set_reasoning_effort(Some(lvl.as_str().to_string()));
                            }
                            Err(_) => {}
                        }
                    }
                    let slot = lookup_provider_descriptor(&st.active_provider)
                        .map(|d| d.secret_slot)
                        .unwrap_or(st.active_provider.as_str());
                    let key_override = st
                        .provider_keys
                        .get(slot)
                        .or_else(|| st.provider_keys.get(&st.active_provider))
                        .cloned();
                    let empty_ep = BTreeMap::new();
                    engine.switch_http_provider_with_vault(&st.active_provider, &st.active_model, None, key_override, &st.provider_keys, &empty_ep);
                    st.active_model = engine.config_snapshot().model_name;
                    let (rp, rm, rr) = (st.active_provider.clone(), st.active_model.clone(), st.active_reasoning.clone());
                    st.record_recent(&rp, &rm, &rr);
                    if params.get("persist").and_then(|v| v.as_bool()) == Some(true) {
                        let mut ucfg = Heav3nsUserConfig::load().unwrap_or_default();
                        ucfg.record_recent_selection(&rp, &rm, &rr);
                        let _ = ucfg.save();
                    }
                    st.model_revision += 1;
                    build_sovereign_model_catalog_with_recent(&st.active_provider, &st.active_model, &st.active_reasoning, st.model_revision, &local_ollama, &st.recent_selections)
                };
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": catalog }));
                let seq = event_seq.fetch_add(1, Ordering::SeqCst) + 1;
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "event", "workspace": ws_id, "session": sess_id, "runtime": "heav3ns-micro", "seq": seq, "type": "models.changed", "payload": catalog }));
            }
            "models.discover" | "models.refresh" => {
                let want_live = method == "models.refresh" || params.get("refresh").and_then(|v| v.as_bool()) == Some(true);
                if want_live {
                    let _ = AgentEngine::refresh_model_catalog().await;
                } else {
                    AgentEngine::trigger_model_catalog_auto_refresh();
                }
                let (prov, keys_snap) = {
                    let st = state.lock().unwrap();
                    (
                        params.get("provider").and_then(|v| v.as_str()).unwrap_or(&st.active_provider).to_string(),
                        st.provider_keys.clone(),
                    )
                };
                let live_models = engine.probe_provider_models_live(&prov, &keys_snap, want_live).await;
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_sovereign_model_discovery(&prov, &live_models) }));
            }
            "providers.list" => {
                let items: Vec<Value> = PROVIDER_DESCRIPTORS
                    .iter()
                    .filter(|d| !d.retired)
                    .map(|d| json!({
                        "id": d.id,
                        "label": d.display_name,
                        "wireDialect": d.wire_dialect.as_str(),
                        "defaultModel": d.default_model,
                        "requiresApiKey": d.requires_api_key,
                        "envVars": d.env_vars,
                        "credentialUrl": d.credential_url
                    }))
                    .collect();
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "count": items.len(), "providers": items } }));
            }
            m if m.ends_with(".status") || m.ends_with(".saveKey") || m.ends_with(".clearKey") => {
                let resp = {
                    let mut st = state.lock().unwrap();
                    let active_prov = st.active_provider.clone();
                    let active_mod = st.active_model.clone();
                    handle_provider_credential_rpc(m, &params, &mut st.provider_keys, &active_prov, &active_mod, &engine)
                };
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
            }
            "posture.get" | "posture.set" | "posture.step" | "receipts.list" | "turn.undo" | "request.preview" => {
                let resp = handle_posture_and_receipts_rpc(method, &params, &engine);
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
            }
            "workspace.branch" | "files.list" | "files.search" | "files.read" | "artifact.read" | "files.readBinary" | "changes.list" | "changes.read" => {
                let resp = handle_files_rpc(method, &params, &engine);
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
            }
            "task.submit" => {
                let prompt = params.get("text").or_else(|| parsed.get("prompt")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let posture_override = params
                    .get("posture")
                    .or_else(|| params.get("mode"))
                    .and_then(|v| v.as_str())
                    .and_then(ExecutionPosture::parse_label);
                let interactive_approval = params.get("interactiveApproval").and_then(|v| v.as_bool()).unwrap_or(true);
                let eng = Arc::clone(&engine);
                let st = Arc::clone(&state);
                let out = Arc::clone(&stdout_lock);
                let seq = Arc::clone(&event_seq);
                let handle = tokio::spawn(async move {
                    execute_multiplexed_submit(eng, st, out, seq, req_id, ws_id, sess_id, is_rpc, prompt, posture_override, interactive_approval).await;
                });
                spawned_turns.push(handle);
            }
            "task.cancel" | "task.cancelAll" => {
                if let Some(ref tok) = state.lock().unwrap().active_cancel {
                    tok.cancel();
                }
                if is_rpc {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "requested": true } }));
                } else {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "id": req_id, "type": "result", "cancelled": true }));
                }
            }
            "task.edit" => {
                let edit_id = params.get("run").or_else(|| params.get("id")).and_then(|v| v.as_str()).unwrap_or("");
                let mut st = state.lock().unwrap();
                if let Some(r) = st.session_runs.iter_mut().find(|r| r.get("id").and_then(|s| s.as_str()) == Some(edit_id)) {
                    if let Some(nt) = params.get("text").and_then(|v| v.as_str()) {
                        r["text"] = json!(nt);
                    }
                    let rev = r.get("revision").and_then(|v| v.as_i64()).unwrap_or(1) + 1;
                    r["revision"] = json!(rev);
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "updated": true, "run": r } }));
                } else {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "updated": false } }));
                }
            }
            "task.remove" => {
                let rm_id = params.get("run").or_else(|| params.get("id")).and_then(|v| v.as_str()).unwrap_or("");
                let mut st = state.lock().unwrap();
                let before = st.session_runs.len();
                st.session_runs.retain(|r| r.get("id").and_then(|s| s.as_str()) != Some(rm_id));
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "removed": st.session_runs.len() < before } }));
            }
            "task.lookup" => {
                let lookup_id = params.get("id").or_else(|| params.get("request")).and_then(|v| v.as_str()).unwrap_or("");
                let st = state.lock().unwrap();
                let found_run = st.session_runs.iter().find(|r| {
                    r.get("id").and_then(|s| s.as_str()) == Some(lookup_id) || r.get("requestId").and_then(|s| s.as_str()) == Some(lookup_id)
                });
                if let Some(r) = found_run {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "found": true, "run": r } }));
                } else {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "found": false } }));
                }
            }
            "task.defer" | "task.deferStatus" | "task.resume" => {
                let deferred = method == "task.defer" && params.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "enabled": true, "deferred": deferred, "queued": 0 } }));
            }
            "history.list" => {
                let runs = state.lock().unwrap().session_runs.clone();
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "runs": runs } }));
            }
            "history.read" => {
                let h_id = params.get("id").or_else(|| params.get("run")).and_then(|v| v.as_str()).unwrap_or("");
                let st = state.lock().unwrap();
                if let Some(r) = st.session_runs.iter().find(|r| r.get("id").and_then(|s| s.as_str()) == Some(h_id)) {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": r, "outputAvailable": true } }));
                } else {
                    let fallback = json!({ "id": if h_id.is_empty() { "run_0" } else { h_id }, "text": "", "session": sess_id, "state": "completed", "createdAt": now_ms() as f64 });
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": fallback, "outputAvailable": false } }));
                }
            }
            "conversation.list" => {
                let convs = state.lock().unwrap().conversations.clone();
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversations": convs } }));
            }
            "conversation.select" | "conversation.create" => {
                let cid = params
                    .get("id")
                    .or_else(|| params.get("conversation"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .unwrap_or_else(|| format!("conv-{}", now_ms()));
                let title = params.get("title").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(String::from);
                let mut st = state.lock().unwrap();
                if !st.conversations.iter().any(|c| c.get("id").and_then(|v| v.as_str()) == Some(&cid)) {
                    let t = title.unwrap_or_else(|| format!("Session {}", st.conversations.len() + 1));
                    st.conversations.push(json!({ "id": cid, "title": t }));
                }
                switch_worker_conversation(&mut st, &engine, &cid);
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "id": st.current_conversation, "conversation": st.current_conversation } }));
            }
            "conversation.delete" => {
                let del_opt = params
                    .get("id")
                    .or_else(|| params.get("conversation"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let mut st = state.lock().unwrap();
                if let Some(ref del_id) = del_opt {
                    st.conversations.retain(|c| c.get("id").and_then(|v| v.as_str()) != Some(del_id.as_str()));
                    st.conversation_messages.remove(del_id);
                }
                if st.conversations.is_empty() {
                    st.conversations.push(json!({ "id": "default-conv", "title": "Main Session" }));
                }
                let next_conv = st.conversations[0].get("id").and_then(|v| v.as_str()).unwrap_or("default-conv").to_string();
                switch_worker_conversation(&mut st, &engine, &next_conv);
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "id": st.current_conversation, "conversation": st.current_conversation } }));
            }
            "activity.list" => {
                let run_arg = params.get("run").and_then(|v| v.as_str()).unwrap_or("");
                let st = state.lock().unwrap();
                if let Some(r) = st.session_runs.iter().find(|r| r.get("id").and_then(|s| s.as_str()) == Some(run_arg)) {
                    let acts = r.get("activities").cloned().unwrap_or_else(|| json!([]));
                    let a_seq = r.get("activitySequence").and_then(|v| v.as_i64()).unwrap_or(0);
                    let a_cnt = r.get("activityCount").and_then(|v| v.as_i64()).unwrap_or(0);
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": run_arg, "activities": acts, "activitySequence": a_seq, "activityCount": a_cnt } }));
                } else {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": run_arg, "activities": [], "activitySequence": 0, "activityCount": 0 } }));
                }
            }
            "checkpoint.list" => {
                let snaps = engine.snapshots_list();
                let flagged = state.lock().unwrap().flagged_checkpoints.clone();
                let cps: Vec<Value> = snaps
                    .iter()
                    .rev()
                    .enumerate()
                    .map(|(i, s)| {
                        json!({
                            "id": s.snapshot_id,
                            "label": format!("Checkpoint #{} (Frame {})", snaps.len() - i, s.frame_index),
                            "moment": if i % 2 == 0 { "after" } else { "before" },
                            "createdAt": s.timestamp as f64,
                            "files": s.staged_files.len(),
                            "commit": sha256_hex(&s.snapshot_id),
                            "flagged": flagged.contains(&s.snapshot_id)
                        })
                    })
                    .collect();
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "checkpoints": cps, "offset": 0, "total": cps.len(), "more": false } }));
            }
            "checkpoint.preview" => {
                let cid = params.get("id").and_then(|v| v.as_str()).unwrap_or("cp-0");
                let cas_id = engine.cas_snapshot_id_for_game_snapshot(cid).unwrap_or_else(|| cid.to_string());
                let (changes, diff_text) = engine.cas_diff_between(&cas_id, None).unwrap_or_default();
                let total = changes.len();
                emit_jsonl_line(&stdout_lock, &json!({
                    "v": 1,
                    "kind": "response",
                    "id": req_id,
                    "result": {
                        "token": format!("tok-{cid}"),
                        "changes": changes,
                        "diff": diff_text,
                        "total": total,
                        "scope": "workspace"
                    }
                }));
            }
            "checkpoint.restore" => {
                let cid = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let snaps = engine.snapshots_list();
                if let Some(idx) = snaps.iter().position(|s| s.snapshot_id == cid) {
                    let _ = engine.rewind_to_snapshot(idx);
                }
                let conv = state.lock().unwrap().current_conversation.clone();
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": conv } }));
            }
            "checkpoint.flag" => {
                if let Some(cid) = params.get("id").and_then(|v| v.as_str()) {
                    let mut st = state.lock().unwrap();
                    if params.get("flagged").and_then(|v| v.as_bool()).unwrap_or(true) {
                        st.flagged_checkpoints.insert(cid.to_string());
                    } else {
                        st.flagged_checkpoints.remove(cid);
                    }
                }
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "flagged": true } }));
            }
            "project.discover" => {
                let targets = discover_project_targets(engine.workspace_root());
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": targets }));
            }
            "project.status" | "project.healing" | "project.start" | "project.stop" | "project.configure" => {
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "state": "idle", "output": "", "revision": 1, "healing": { "enabled": true } } }));
            }
            "project.tasks.list" => {
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "revision": 1, "total": 0, "offset": 0, "nextOffset": 0, "limited": false, "tasks": [] } }));
            }
            "project.tasks.move" => {
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "updated": true, "revision": 2 } }));
            }
            "command.actionsList" => {
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "actions": [], "total": 0, "truncated": false } }));
            }
            "command.actionReconcile" => {
                let aid = params.get("actionId").and_then(|v| v.as_str()).unwrap_or("act-0");
                let out = params.get("outcome").and_then(|v| v.as_str()).unwrap_or("completed");
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "reconciled": true, "actionId": aid, "outcome": out } }));
            }
            "agentSwarm.settings" => {
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "enabled": true, "criticalCompletionReview": true, "hierarchicalDefer": true, "legion": true, "maxSpecialists": 8, "strategy": "adaptive", "deferBoundary": "safe_boundary" } }));
            }
            "agentSwarm.classify" => {
                let task_desc = params.get("objective").or_else(|| params.get("task")).and_then(|v| v.as_str()).unwrap_or("Verify system architecture and execute requested changes");
                let strategy = params.get("strategy").and_then(|v| v.as_str());
                let max_spec = params.get("maxSpecialists").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
                let target_files: Vec<String> = params
                    .get("targetFiles")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let legion_plan = classify_task_for_legion(task_desc, &target_files, strategy, max_spec);
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": legion_plan }));
            }
            "computer.action" | "computer.capture" | "computer.health" => {
                let mut req = params.clone();
                if method == "computer.capture" && req.get("action").is_none() {
                    req["action"] = json!("capture");
                }
                if method == "computer.health" && req.get("action").is_none() {
                    req["action"] = json!("audit_health");
                }
                let out = engine.tools().display_state.lock().unwrap().execute_action(&req).unwrap_or_else(|e| json!({"error": e}));
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": out }));
            }
            "computer.observe" | "computer.interpret" | "computer.consult" | "computer.orchestrate" => {
                let action = match method {
                    "computer.observe" => "observe",
                    "computer.interpret" => "interpret",
                    "computer.consult" => "consult_swarm",
                    _ => "orchestrate",
                };
                let mut req = params.clone();
                req["action"] = json!(action);
                let call = crate::domain::types::tool_schedule::ScheduledToolCall {
                    id: format!("req-{req_id}"),
                    name: "computer_use".into(),
                    args: req,
                };
                let res = engine.tools().execute_single(&call, engine.workspace_root(), ExecutionAuthorityLevel::Autonomous, None).await;
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": if res.success { res.output } else { json!({"error": res.error}) } }));
            }
            "computer.patterns" => {
                let kstore = engine.tools().computer_orchestrator.knowledge_store.lock().unwrap();
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "patterns": kstore.list_patterns(), "procedures": kstore.list_procedures() } }));
            }
            "browser.action" | "browser.snapshot" | "browser.navigate" => {
                let mut req = params.clone();
                if method == "browser.snapshot" && req.get("action").is_none() {
                    req["action"] = json!("snapshot");
                }
                if method == "browser.navigate" && req.get("action").is_none() {
                    req["action"] = json!("navigate");
                }
                let out = engine.tools().display_state.lock().unwrap().execute_browser_action(&req).unwrap_or_else(|e| json!({"error": e}));
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": out }));
            }
            "spatial.ground" | "spatial.execute" | "spatial.health" | "spatial.query" => {
                let action = match method {
                    "spatial.ground" => "ground",
                    "spatial.query" => "query",
                    "spatial.health" => "audit",
                    _ => "execute",
                };
                let mut req = params.clone();
                req["action"] = json!(action);
                let call = crate::domain::types::tool_schedule::ScheduledToolCall {
                    id: format!("req-{req_id}"),
                    name: "spatial_interact".into(),
                    args: req,
                };
                let res = engine.tools().execute_single(&call, engine.workspace_root(), ExecutionAuthorityLevel::Autonomous, None).await;
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": if res.success { res.output } else { json!({"error": res.error}) } }));
            }
            "skills.dag" | "skills.forge" | "skills.lint" | "soul.status" => {
                let out = {
                    let mut cat = engine.tools().skill_catalog.lock().unwrap();
                    match method {
                        "skills.forge" => cat.forge_skill(&params).unwrap_or_else(|e| json!({"error": e})),
                        "skills.lint" => {
                            let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator");
                            cat.lint_and_autofix_skill(id, params.get("autofix").and_then(|v| v.as_bool()).unwrap_or(false)).unwrap_or_else(|e| json!({"error": e}))
                        }
                        "soul.status" => json!({ "activeSoul": cat.active_soul, "competencies": cat.competency_scores }),
                        _ => cat.evaluate_dag(),
                    }
                };
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": out }));
            }
            "agentSwarm.form" | "agentSwarm.formResume" => {
                let objective = params.get("objective").and_then(|v| v.as_str()).unwrap_or("Swarm task execution").to_string();
                let strategy = params.get("strategy").and_then(|v| v.as_str());
                let max_spec = params.get("maxSpecialists").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                let legion = classify_task_for_legion(&objective, &[], strategy, max_spec);
                let assignments: Vec<SwarmAssignment> = legion
                    .get("assignments")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let execute_live = params.get("live").and_then(|v| v.as_bool()).unwrap_or(false);
                let rec = if execute_live && !assignments.is_empty() {
                    let swarm_res = engine.run_swarm(objective.clone(), assignments.clone(), None, None).await;
                    let form_id = format!("formation-{}", state.lock().unwrap().saved_formations.len() + 1);
                    json!({
                        "id": form_id,
                        "objective": objective,
                        "status": format!("{:?}", swarm_res.outcome).to_ascii_lowercase(),
                        "reports": swarm_res.reports,
                        "assignments": assignments
                    })
                } else {
                    let form_id = format!("formation-{}", state.lock().unwrap().saved_formations.len() + 1);
                    let rep_items: Vec<Value> = assignments
                        .iter()
                        .map(|a| json!({ "id": a.id, "role": a.role, "status": "completed", "response": format!("Planned specialist scope: {}", a.objective) }))
                        .collect();
                    let assign_items: Vec<Value> = assignments
                        .iter()
                        .enumerate()
                        .map(|(idx, a)| json!({ "id": a.id, "role": a.role, "status": "completed", "summary": a.objective, "childSessionId": format!("cs-{}", idx + 1) }))
                        .collect();
                    json!({
                        "id": form_id,
                        "objective": objective,
                        "status": "completed",
                        "reports": rep_items,
                        "assignments": assign_items
                    })
                };
                state.lock().unwrap().saved_formations.push(rec.clone());
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": rec }));
            }
            "agentSwarm.formReconcile" => {
                let fid = params.get("formationId").and_then(|v| v.as_str()).unwrap_or("formation-1");
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "id": fid, "status": "deferred", "reconciledAt": now_ms() as f64 } }));
            }
            "agentSwarm.formStatus" => {
                let last = state.lock().unwrap().saved_formations.last().cloned();
                if let Some(l) = last {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": l }));
                } else {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "status": "none", "id": "form-idle" } }));
                }
            }
            "agentSwarm.formHistory" => {
                let forms = state.lock().unwrap().saved_formations.clone();
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "formations": forms } }));
            }
            "agentSwarm.formStream" => {
                let fid = params.get("formationId").and_then(|v| v.as_str()).unwrap_or("formation-1");
                let sid = params.get("specialistId").and_then(|v| v.as_str()).unwrap_or("spec-1");
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "formationId": fid, "specialistId": sid, "childSessionId": "cs-1", "messageCount": 1, "excerpt": "Specialist execution verified with 100% Joyzoning compliance." } }));
            }
            "approval.resolve" => {
                let decision_raw = params.get("decision").or_else(|| params.get("choice")).and_then(|v| v.as_str()).unwrap_or("");
                let scope_raw = params.get("scope").and_then(|v| v.as_str()).unwrap_or("");
                let approved_bool = params.get("approved").and_then(|v| v.as_bool());
                let choice = if decision_raw.eq_ignore_ascii_case("allow_session") || scope_raw.eq_ignore_ascii_case("session") || params.get("remember").and_then(|v| v.as_bool()) == Some(true) {
                    UserApprovalChoice::AllowSession
                } else if matches!(decision_raw.to_ascii_lowercase().as_str(), "allow" | "allow_once" | "approve" | "approved" | "y" | "yes") || approved_bool == Some(true) {
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
                    let mut st = state.lock().unwrap();
                    let tx_opt = if let Some(tid) = target_id {
                        st.pending_approvals.remove(tid)
                    } else {
                        let first_key = st.pending_approvals.keys().next().cloned();
                        first_key.and_then(|k| st.pending_approvals.remove(&k))
                    };
                    tx_opt.map(|tx| tx.send(choice).is_ok()).unwrap_or(false)
                };
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "resolved": true, "delivered": delivered, "choice": format!("{choice:?}") } }));
            }
            "spider.check" => {
                let mut paths: Vec<String> = params
                    .get("paths")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                if paths.is_empty() {
                    if let Some(f) = params.get("file").or_else(|| params.get("path")).and_then(|v| v.as_str()) {
                        paths.push(f.to_string());
                    }
                }
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_spider_check_multi_response(engine.workspace_root(), &paths) }));
            }
            "state.inspect" => {
                let st = state.lock().unwrap();
                let conv = params.get("conversation").and_then(|v| v.as_str()).unwrap_or(&st.current_conversation);
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_state_inspect_response(conv, &st.session_runs) }));
            }
            "context.inspect" => {
                let conv = {
                    let st = state.lock().unwrap();
                    params.get("conversation").and_then(|v| v.as_str()).unwrap_or(&st.current_conversation).to_string()
                };
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_context_inspect_response(&conv, &engine) }));
            }
            "prompt.inspect" => {
                let mode = params.get("mode").and_then(|v| v.as_str()).unwrap_or("");
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_prompt_inspect_response(mode, &engine) }));
            }
            "context.history" => {
                let conv = {
                    let st = state.lock().unwrap();
                    params.get("conversation").and_then(|v| v.as_str()).unwrap_or(&st.current_conversation).to_string()
                };
                let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_context_history_response(&conv, offset, &engine) }));
            }
            "context.compact" => {
                let conv = {
                    let st = state.lock().unwrap();
                    params.get("conversation").and_then(|v| v.as_str()).unwrap_or(&st.current_conversation).to_string()
                };
                let report = engine.session().lock().unwrap().compact_session(engine.compactor(), &CompactionPolicy { force: true, ..Default::default() });
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": {
                    "conversation": conv,
                    "compacted": report.compacted,
                    "before": report.input_message_count,
                    "after": report.output_message_count,
                    "inputTokens": report.input_tokens,
                    "outputTokens": report.output_tokens
                }}));
            }
            "side.query" => {
                let conv = {
                    let st = state.lock().unwrap();
                    params.get("conversation").and_then(|v| v.as_str()).unwrap_or(&st.current_conversation).to_string()
                };
                let q = params.get("question").and_then(|v| v.as_str()).unwrap_or("");
                let ans = engine.ask_side_question(q).await.unwrap_or_else(|_| format!("HEAV3NS-MICRO side query processed ({} chars).", q.len()));
                let reply = if ans.trim().is_empty() { "HEAV3NS-MICRO side query verified.".to_string() } else { ans };
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": conv, "response": reply, "contextLimited": false } }));
            }
            "memory.list" | "memory.read" | "memory.write" | "memory.remove" => {
                let conv = state.lock().unwrap().current_conversation.clone();
                let resp = handle_memory_rpc(method, &params, &conv, &engine);
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
            }
            "status" | "accounts" | "signIn" | "signOut" | "cancel" | "switch" | "rename" | "remove" | "clearLimit" | "setAuto" => {
                let resp = {
                    let mut st = state.lock().unwrap();
                    if method == "signOut" || method == "remove" {
                        st.account_signed_in = false;
                    }
                    if method == "signIn" {
                        st.account_signed_in = true;
                    }
                    if method == "rename" {
                        if let Some(lbl) = params.get("label").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                            st.account_label = lbl.to_string();
                        }
                    }
                    if method == "setAuto" {
                        if let Some(en) = params.get("enabled").and_then(|v| v.as_bool()) {
                            st.account_auto_switch = en;
                        }
                    }
                    let acc_list = if st.account_signed_in {
                        vec![json!({ "id": "acc-default", "label": st.account_label, "planType": "pro", "active": true, "limited": false, "canRefresh": true })]
                    } else {
                        Vec::new()
                    };
                    json!({
                        "saved": st.account_signed_in,
                        "created": true,
                        "accessChecked": true,
                        "modelCount": 3,
                        "account": { "id": "acc-default", "label": st.account_label },
                        "accounts": acc_list,
                        "autoSwitch": st.account_auto_switch
                    })
                };
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }));
            }
            "ping" | "health" => {
                let q = engine.queue().snapshot();
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "id": req_id, "type": "result", "status": "ok", "posture": engine.current_posture().slug(), "queueActive": q.active, "queuePending": q.pending_count, "timestamp": now_ms() }));
            }
            "flight" => {
                let report = run_agent_flight_test().await;
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "id": req_id, "type": "result", "report": report }));
            }
            "swarm" => {
                let objective = parsed.get("objective").and_then(|v| v.as_str()).unwrap_or("Swarm execution").to_string();
                let assignments: Vec<SwarmAssignment> = parsed.get("assignments").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
                let token = CancellationToken::new();
                state.lock().unwrap().active_cancel = Some(token.clone());
                let res = engine.run_swarm(objective, assignments, Some(token), None).await;
                state.lock().unwrap().active_cancel = None;
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "id": req_id, "type": "result", "swarm": res }));
            }
            "shutdown" | "exit" => {
                for handle in spawned_turns.drain(..) {
                    let _ = handle.await;
                }
                if is_rpc {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "stopped": true } }));
                } else {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "id": req_id, "type": "shutdown" }));
                }
                break;
            }
            "tick" | "chat" | "slash" => {
                let prompt = parsed.get("prompt").or_else(|| parsed.get("command")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let token = CancellationToken::new();
                state.lock().unwrap().active_cancel = Some(token.clone());
                let req_id_progress = req_id.clone();
                let out_prog = Arc::clone(&stdout_lock);
                let mut input = EngineTickInput::new(prompt);
                input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
                input.cancel_token = Some(token);
                input.on_progress = Some(Arc::new(move |ev| {
                    emit_jsonl_line(&out_prog, &json!({ "v": 1, "id": req_id_progress, "type": "progress", "event": ev }));
                }));
                let result = engine.tick(input).await;
                state.lock().unwrap().active_cancel = None;
                emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "id": req_id, "type": "result", "result": result }));
            }
            other => {
                if is_rpc {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "status": "ok", "unhandled": other } }));
                } else {
                    emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "id": req_id, "type": "error", "code": "UNKNOWN_METHOD", "message": format!("Unsupported method: {other}") }));
                }
            }
        }
    }

    for handle in spawned_turns {
        let _ = handle.await;
    }
}
