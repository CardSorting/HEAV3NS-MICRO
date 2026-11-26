// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::domain::interfaces::IProjectRunner;
use crate::domain::types::provider_registry::{
    canonicalize_provider_id, lookup_provider_descriptor, retired_provider_remediation,
    ReasoningEffortLevel, PROVIDER_DESCRIPTORS,
};
use crate::domain::types::runtime::{CancellationToken, ExecutionPosture};
use crate::plumbing::hash_and_math::{fnv1a_32, lock_unpoison, now_ms};
use crate::ui::cli_presenter::Heav3nsUserConfig;
use crate::ui::rpc_dispatch::{
    build_hello_response,
    build_model_selection, sha256_hex,
};
use crate::ui::worker_handlers::{
    build_artifact_run_response, build_sovereign_model_catalog_with_recent,
    build_sovereign_model_discovery, emit_jsonl_line, emit_seq_event, handle_posture_and_receipts_rpc,
    read_bounded_stdin_line, resolve_worker_run, switch_worker_conversation, WorkerSharedState,
};
use serde_json::{json, Value};
use std::io;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::BufReader;
use tokio::task::JoinHandle;

use crate::ui::files_rpc::handle_files_rpc;
use crate::ui::stdio_worker_extended::dispatch_worker_rpc_extended;
use crate::ui::worker_submit::execute_multiplexed_submit;

pub async fn run_stdio_worker(engine: Arc<AgentEngine>, project_runner: Arc<dyn IProjectRunner>) {
    let stdout_lock = Arc::new(Mutex::new(io::stdout()));
    let state = Arc::new(Mutex::new(WorkerSharedState::from_engine_config(
        &engine.config_snapshot(),
    )));
    let event_seq = Arc::new(AtomicI64::new(0));
    let legacy_turn_gate = Arc::new(tokio::sync::Mutex::new(()));
    let pending_submits = Arc::new(AtomicUsize::new(0));
    let mut spawned_turns: Vec<JoinHandle<()>> = Vec::new();
    let mut active_signin_cancel: Option<CancellationToken> = None;

    let root_bytes = engine
        .workspace_root()
        .to_string_lossy()
        .as_bytes()
        .to_vec();
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

    let mut reader = BufReader::new(tokio::io::stdin());
    loop {
        let raw = match read_bounded_stdin_line(&mut reader, 4 * 1024 * 1024).await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(e) => {
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "type": "error", "code": "STDIN_READ_ERROR", "message": e.to_string() }),
                );
                if e.kind() == io::ErrorKind::InvalidData {
                    continue;
                }
                break;
            }
        };
        spawned_turns.retain(|h| !h.is_finished());
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "type": "error", "code": "INVALID_JSON", "message": e.to_string() }),
                );
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
            .filter(|s| !s.is_empty())
            .unwrap_or(&default_ws)
            .to_string();
        let sess_id = parsed
            .get("session")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&default_sess)
            .to_string();
        let is_rpc = parsed.get("v").and_then(|v| v.as_i64()) == Some(1)
            || parsed.get("kind").and_then(|v| v.as_str()) == Some("request");
        let method = parsed
            .get("method")
            .or_else(|| parsed.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("tick");
        let params = parsed
            .get("params")
            .cloned()
            .unwrap_or_else(|| parsed.clone());

        match method {
            "hello" => {
                let resp = build_hello_response(&ws_id, &sess_id, engine.workspace_root());
                if is_rpc {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }),
                    );
                } else {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "id": req_id, "type": "result", "result": resp }),
                    );
                }
            }
            "snapshot" => {
                let resp = {
                    let st = lock_unpoison(&state);
                    let active_id = st
                        .session_runs
                        .iter()
                        .rev()
                        .find(|r| {
                            matches!(
                                r.get("state").and_then(|s| s.as_str()),
                                Some("running" | "queued")
                            )
                        })
                        .and_then(|r| r.get("id"))
                        .cloned();
                    json!({
                        "runtime": "heav3ns-micro",
                        "workspace": ws_id,
                        "session": sess_id,
                        "conversation": st.current_conversation,
                        "posture": engine.current_posture().slug(),
                        "seq": event_seq.load(Ordering::SeqCst),
                        "runs": st.session_runs.iter().filter(|r| r.get("conversation").and_then(|v| v.as_str()).is_none_or(|c| c == st.current_conversation)).cloned().collect::<Vec<_>>(),
                        "modelSelection": build_model_selection(&st.active_provider, &st.active_model, &st.active_reasoning, st.model_revision),
                        "active": active_id,
                        "queued": []
                    })
                };
                if is_rpc {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }),
                    );
                } else {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "id": req_id, "type": "result", "result": resp }),
                    );
                }
            }
            "models.list" => {
                spawned_turns.retain(|h| !h.is_finished());
                if spawned_turns.len() >= 32 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending tasks." } }),
                    );
                    continue;
                }
                let (st, out) = (Arc::clone(&state), Arc::clone(&stdout_lock));
                spawned_turns.push(tokio::spawn(async move {
                    let local_ollama = AgentEngine::probe_ollama_models().await;
                    let resp = {
                        let st = lock_unpoison(&st);
                        build_sovereign_model_catalog_with_recent(
                            &st.active_provider,
                            &st.active_model,
                            &st.active_reasoning,
                            st.model_revision,
                            &local_ollama,
                            &st.recent_selections,
                        )
                    };
                    emit_jsonl_line(
                        &out,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }),
                    );
                }));
            }
            "models.select" => {
                if let Some(p_raw) = params
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    if let Some(retired_msg) = retired_provider_remediation(p_raw) {
                        emit_jsonl_line(
                            &stdout_lock,
                            &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": -32602, "message": retired_msg } }),
                        );
                        continue;
                    }
                }
                spawned_turns.retain(|h| !h.is_finished());
                if spawned_turns.len() >= 32 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending tasks." } }),
                    );
                    continue;
                }
                static MODEL_SELECT_SEQ: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let select_seq =
                    MODEL_SELECT_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                let (eng, st, out, seq, ws, sess) = (
                    Arc::clone(&engine),
                    Arc::clone(&state),
                    Arc::clone(&stdout_lock),
                    Arc::clone(&event_seq),
                    ws_id.clone(),
                    sess_id.clone(),
                );
                spawned_turns.push(tokio::spawn(async move {
                    let local_ollama = AgentEngine::probe_ollama_models().await;
                    let (stale, catalog) = {
                        let mut st = lock_unpoison(&st);
                        if MODEL_SELECT_SEQ.load(std::sync::atomic::Ordering::SeqCst) != select_seq {
                            (true, build_sovereign_model_catalog_with_recent(&st.active_provider, &st.active_model, &st.active_reasoning, st.model_revision, &local_ollama, &st.recent_selections))
                        } else {
                            if let Some(p) = params.get("provider").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) { st.active_provider = canonicalize_provider_id(p); }
                            if let Some(m) = params.get("model").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) { st.active_model = m.to_string(); }
                            if let Some(r) = params.get("reasoningEffort").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                                match ReasoningEffortLevel::parse_flexible(r) {
                                    Ok(None) => { st.active_reasoning = "default".to_string(); eng.set_reasoning_effort(None); }
                                    Ok(Some(lvl)) => { st.active_reasoning = lvl.as_str().to_string(); eng.set_reasoning_effort(Some(lvl.as_str().to_string())); }
                                    Err(_) => {}
                                }
                            }
                            let slot = lookup_provider_descriptor(&st.active_provider).map(|d| d.secret_slot).unwrap_or(st.active_provider.as_str());
                            let key_override = st.provider_keys.get(slot).or_else(|| st.provider_keys.get(&st.active_provider)).cloned();
                            let endpoints = Heav3nsUserConfig::load().map(|c| c.provider_endpoints).unwrap_or_default();
                            eng.switch_http_provider_with_vault(&st.active_provider, &st.active_model, None, key_override, &st.provider_keys, &endpoints);
                            st.active_model = eng.config_snapshot().model_name;
                            let (rp, rm, rr) = (st.active_provider.clone(), st.active_model.clone(), st.active_reasoning.clone());
                            st.record_recent(&rp, &rm, &rr);
                            if params.get("persist").and_then(|v| v.as_bool()) == Some(true) {
                                let _g = crate::ui::provider_rpc::USER_CONFIG_IO_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                                let mut ucfg = Heav3nsUserConfig::load().unwrap_or_default();
                                ucfg.record_recent_selection(&rp, &rm, &rr); let _ = ucfg.save();
                            }
                            st.model_revision += 1;
                            (false, build_sovereign_model_catalog_with_recent(&st.active_provider, &st.active_model, &st.active_reasoning, st.model_revision, &local_ollama, &st.recent_selections))
                        }
                    };
                    emit_jsonl_line(&out, &json!({ "v": 1, "kind": "response", "id": req_id, "result": catalog }));
                    if !stale { emit_seq_event(&out, &seq, json!({ "v": 1, "kind": "event", "workspace": ws, "session": sess, "runtime": "heav3ns-micro", "type": "models.changed", "payload": catalog })); }
                }));
            }
            "models.discover" | "models.refresh" => {
                spawned_turns.retain(|h| !h.is_finished());
                if spawned_turns.len() >= 32 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending tasks." } }),
                    );
                    continue;
                }
                let want_live = method == "models.refresh"
                    || params.get("refresh").and_then(|v| v.as_bool()) == Some(true);
                let (eng, st, out) = (
                    Arc::clone(&engine),
                    Arc::clone(&state),
                    Arc::clone(&stdout_lock),
                );
                spawned_turns.push(tokio::spawn(async move {
                    if want_live { let _ = AgentEngine::refresh_model_catalog().await; } else { AgentEngine::trigger_model_catalog_auto_refresh(); }
                    let (prov, keys_snap) = { let st = lock_unpoison(&st); (params.get("provider").and_then(|v| v.as_str()).unwrap_or(&st.active_provider).to_string(), st.provider_keys.clone()) };
                    let live_models = eng.probe_provider_models_live(&prov, &keys_snap, want_live).await;
                    emit_jsonl_line(&out, &json!({ "v": 1, "kind": "response", "id": req_id, "result": build_sovereign_model_discovery(&prov, &live_models) }));
                }));
            }
            "providers.list" => {
                let items: Vec<Value> = PROVIDER_DESCRIPTORS.iter().filter(|d| !d.retired).map(|d| json!({
                    "id": d.id, "label": d.display_name, "wireDialect": d.wire_dialect.as_str(), "defaultModel": d.default_model, "requiresApiKey": d.requires_api_key, "envVars": d.env_vars, "credentialUrl": d.credential_url
                })).collect();
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "count": items.len(), "providers": items } }),
                );
            }
            "posture.get" | "posture.set" | "posture.step" | "receipts.list" | "turn.undo"
            | "request.preview" => {
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": handle_posture_and_receipts_rpc(method, &params, &engine) }),
                );
            }
            "artifact.read" => {
                let resp =
                    build_artifact_run_response(&lock_unpoison(&state).session_runs, &params)
                        .unwrap_or_else(|| handle_files_rpc("files.read", &params, &engine));
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }),
                );
            }
            "workspace.branch" | "files.list" | "files.search" | "files.read"
            | "files.readBinary" | "changes.list" | "changes.read"
            | "git.init" | "git.stage" | "git.unstage" | "git.commit" | "git.discard"
            | "git.generateCommitMessage" | "git.push" | "git.pull"
            | "git.fetch" | "git.sync" | "git.log" | "git.show"
            | "git.branchList" | "git.checkout" | "git.stash" | "git.stashPop"
            | "git.addRemote" | "git.connectRepo" | "git.publishGitHub"
            | "git.githubStatus" | "git.githubSignIn" => {
                let resp = handle_files_rpc(method, &params, &engine);
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": resp }),
                );
            }
            "task.submit" => {
                spawned_turns.retain(|h| !h.is_finished());
                if spawned_turns.len() >= 32 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "RUNTIME_QUEUE_FULL", "message": "Too many pending turns." } }),
                    );
                    continue;
                }
                let prompt = params
                    .get("text")
                    .or_else(|| parsed.get("prompt"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if prompt.chars().count() > 16_000 {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": -32602, "message": "Instruction exceeds 16000 character limit." } }),
                    );
                    continue;
                }
                let posture_override = params
                    .get("posture")
                    .or_else(|| params.get("mode"))
                    .and_then(|v| v.as_str())
                    .and_then(ExecutionPosture::parse_label);
                let interactive_approval = params
                    .get("interactiveApproval")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
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
                    struct PendingGuard(Arc<AtomicUsize>);
                    impl Drop for PendingGuard {
                        fn drop(&mut self) {
                            self.0.fetch_sub(1, Ordering::SeqCst);
                        }
                    }
                    let _pg = PendingGuard(pending);
                    execute_multiplexed_submit(
                        eng,
                        st,
                        out,
                        seq,
                        req_id,
                        ws_id,
                        sess_id,
                        is_rpc,
                        prompt,
                        posture_override,
                        interactive_approval,
                        gate,
                    )
                    .await;
                });
                spawned_turns.push(handle);
            }
            "task.cancel" | "task.cancelAll" => {
                let target_run = params
                    .get("run")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let guard = lock_unpoison(&state);
                if method == "task.cancel" {
                    if let Some(id) = target_run {
                        if let Some(tok) = guard.active_cancels.get(id) {
                            tok.cancel();
                        }
                    } else {
                        for tok in guard.active_cancels.values() {
                            tok.cancel();
                        }
                    }
                } else {
                    for tok in guard.active_cancels.values() {
                        tok.cancel();
                    }
                }
                if is_rpc {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "requested": true } }),
                    );
                } else {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "id": req_id, "type": "result", "cancelled": true }),
                    );
                }
            }
            "task.edit" => {
                let edit_id = params
                    .get("run")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let mut st = lock_unpoison(&state);
                if let Some(r) = st
                    .session_runs
                    .iter_mut()
                    .find(|r| r.get("id").and_then(|s| s.as_str()) == Some(edit_id))
                {
                    if let Some(nt) = params.get("text").and_then(|v| v.as_str()) {
                        r["text"] = json!(nt);
                    }
                    let rev = r.get("revision").and_then(|v| v.as_i64()).unwrap_or(1) + 1;
                    r["revision"] = json!(rev);
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "updated": true, "run": r } }),
                    );
                } else {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "updated": false } }),
                    );
                }
            }
            "task.remove" => {
                let rm_id = params
                    .get("run")
                    .or_else(|| params.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let mut st = lock_unpoison(&state);
                let before = st.session_runs.len();
                st.session_runs
                    .retain(|r| r.get("id").and_then(|s| s.as_str()) != Some(rm_id));
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "removed": st.session_runs.len() < before } }),
                );
            }
            "task.lookup" => {
                let lookup_id = params
                    .get("id")
                    .or_else(|| params.get("request"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let st = lock_unpoison(&state);
                let found_run = st.session_runs.iter().find(|r| {
                    r.get("id").and_then(|s| s.as_str()) == Some(lookup_id)
                        || r.get("requestId").and_then(|s| s.as_str()) == Some(lookup_id)
                });
                if let Some(r) = found_run {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "found": true, "run": r } }),
                    );
                } else {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "found": false } }),
                    );
                }
            }
            "task.defer" | "task.deferStatus" | "task.resume" => {
                let deferred = method == "task.defer"
                    && params
                        .get("enabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "enabled": true, "deferred": deferred, "queued": 0 } }),
                );
            }
            "history.list" => {
                let runs = lock_unpoison(&state).session_runs.clone();
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "runs": runs } }),
                );
            }
            "history.read" => {
                let h_id = params
                    .get("id")
                    .or_else(|| params.get("run"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let st = lock_unpoison(&state);
                if let Some(r) = resolve_worker_run(&st.session_runs, h_id) {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": r, "outputAvailable": true } }),
                    );
                } else {
                    let fallback = json!({ "id": if h_id.is_empty() { "run_0" } else { h_id }, "text": "", "session": sess_id, "state": "completed", "createdAt": now_ms() as f64 });
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": fallback, "outputAvailable": false } }),
                    );
                }
            }
            "conversation.list" => {
                let convs = lock_unpoison(&state).conversations.clone();
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversations": convs } }),
                );
            }
            "conversation.select" | "conversation.create" => {
                if pending_submits.load(Ordering::SeqCst) > 0
                    || legacy_turn_gate.try_lock().is_err()
                {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "TURN_IN_PROGRESS", "message": "Cannot switch conversations while a turn is active." } }),
                    );
                    continue;
                }
                let cid = params
                    .get("id")
                    .or_else(|| params.get("conversation"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .unwrap_or_else(|| format!("conv-{}", now_ms()));
                let title = params
                    .get("title")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                let mut st = lock_unpoison(&state);
                if !st
                    .conversations
                    .iter()
                    .any(|c| c.get("id").and_then(|v| v.as_str()) == Some(&cid))
                {
                    let t =
                        title.unwrap_or_else(|| format!("Session {}", st.conversations.len() + 1));
                    st.conversations.push(json!({ "id": cid, "title": t }));
                }
                switch_worker_conversation(&mut st, &engine, &cid);
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "id": st.current_conversation, "conversation": st.current_conversation } }),
                );
            }
            "conversation.delete" => {
                if pending_submits.load(Ordering::SeqCst) > 0
                    || legacy_turn_gate.try_lock().is_err()
                {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "TURN_IN_PROGRESS", "message": "Cannot delete conversations while a turn is active." } }),
                    );
                    continue;
                }
                let del_opt = params
                    .get("id")
                    .or_else(|| params.get("conversation"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let mut st = lock_unpoison(&state);
                if let Some(ref del_id) = del_opt {
                    st.conversations
                        .retain(|c| c.get("id").and_then(|v| v.as_str()) != Some(del_id.as_str()));
                    st.conversation_messages.remove(del_id);
                }
                if st.conversations.is_empty() {
                    st.conversations
                        .push(json!({ "id": "default-conv", "title": "Main Session" }));
                }
                let next_conv = st.conversations[0]
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default-conv")
                    .to_string();
                if del_opt.as_deref() == Some(st.current_conversation.as_str())
                    && next_conv == st.current_conversation
                {
                    st.current_conversation.clear();
                }
                switch_worker_conversation(&mut st, &engine, &next_conv);
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "id": st.current_conversation, "conversation": st.current_conversation } }),
                );
            }
            "activity.list" => {
                let run_arg = params.get("run").and_then(|v| v.as_str()).unwrap_or("");
                let st = lock_unpoison(&state);
                if let Some(r) = st
                    .session_runs
                    .iter()
                    .find(|r| r.get("id").and_then(|s| s.as_str()) == Some(run_arg))
                {
                    let acts = r.get("activities").cloned().unwrap_or_else(|| json!([]));
                    let a_seq = r
                        .get("activitySequence")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let a_cnt = r.get("activityCount").and_then(|v| v.as_i64()).unwrap_or(0);
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": run_arg, "activities": acts, "activitySequence": a_seq, "activityCount": a_cnt } }),
                    );
                } else {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "run": run_arg, "activities": [], "activitySequence": 0, "activityCount": 0 } }),
                    );
                }
            }
            "checkpoint.list" => {
                let snaps = engine.snapshots_list();
                let flagged = lock_unpoison(&state).flagged_checkpoints.clone();
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
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "checkpoints": cps, "offset": 0, "total": cps.len(), "more": false } }),
                );
            }
            "checkpoint.preview" => {
                let cid = params.get("id").and_then(|v| v.as_str()).unwrap_or("cp-0");
                let cas_id = engine
                    .cas_snapshot_id_for_game_snapshot(cid)
                    .unwrap_or_else(|| cid.to_string());
                let (changes, diff_text) =
                    engine.cas_diff_between(&cas_id, None).unwrap_or_default();
                let total = changes.len();
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({
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
                    }),
                );
            }
            "checkpoint.restore" => {
                let cid = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let snaps = engine.snapshots_list();
                if let Some(idx) = snaps.iter().position(|s| s.snapshot_id == cid) {
                    if let Err(e) = engine.rewind_to_snapshot(idx) {
                        emit_jsonl_line(
                            &stdout_lock,
                            &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "REWIND_FAILED", "message": e } }),
                        );
                    } else {
                        let conv = lock_unpoison(&state).current_conversation.clone();
                        emit_jsonl_line(
                            &stdout_lock,
                            &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "conversation": conv } }),
                        );
                    }
                } else {
                    emit_jsonl_line(
                        &stdout_lock,
                        &json!({ "v": 1, "kind": "response", "id": req_id, "error": { "code": "NOT_FOUND", "message": format!("Checkpoint '{cid}' not found") } }),
                    );
                }
            }
            "checkpoint.flag" => {
                let flagged = params
                    .get("flagged")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                if let Some(cid) = params.get("id").and_then(|v| v.as_str()) {
                    let mut st = lock_unpoison(&state);
                    if flagged {
                        st.flagged_checkpoints.insert(cid.to_string());
                    } else {
                        st.flagged_checkpoints.remove(cid);
                    }
                }
                emit_jsonl_line(
                    &stdout_lock,
                    &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "flagged": flagged } }),
                );
            }
            other => {
                if dispatch_worker_rpc_extended(
                    other, &parsed, &params, &req_id, &ws_id, &sess_id, is_rpc,
                    &engine, &project_runner, &state, &stdout_lock,
                    &mut active_signin_cancel, &legacy_turn_gate, &mut spawned_turns, &event_seq, &pending_submits,
                ).await {
                    break;
                }
            }
        }
    }

    if let Some(tok) = active_signin_cancel.take() {
        tok.cancel();
    }
    for tok in lock_unpoison(&state).active_cancels.values() {
        tok.cancel();
    }
    let _ = project_runner.stop();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        for handle in spawned_turns {
            let _ = handle.await;
        }
    })
    .await;
}
