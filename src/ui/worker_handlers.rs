// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::domain::pricing_and_taxonomy::CREDENTIAL_REJECTED_UNSENT_CODE;
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::progress::{EngineProgressEvent, EngineProgressPhase, EngineProgressStatus};
use crate::domain::types::provider_registry::canonicalize_provider_id;
use crate::domain::types::runtime::{AgentConfig, CancellationToken, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel, ExecutionPosture, UserApprovalChoice};
use crate::plumbing::hash_and_math::now_ms;
use crate::ui::cli_presenter::Heav3nsUserConfig;
pub use crate::ui::provider_rpc::{build_sovereign_model_catalog, build_sovereign_model_catalog_with_recent, build_sovereign_model_discovery, handle_provider_credential_rpc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Shared multiplexed state across the async JSON-RPC stdin reader and background turn tasks.
pub struct WorkerSharedState {
    pub active_cancel: Option<CancellationToken>,
    pub session_runs: Vec<Value>,
    pub current_conversation: String,
    pub conversations: Vec<Value>,
    pub conversation_messages: BTreeMap<String, Vec<SessionMessage>>,
    pub active_provider: String,
    pub active_model: String,
    pub active_reasoning: String,
    pub model_revision: i64,
    pub provider_keys: BTreeMap<String, String>,
    pub recent_selections: Vec<(String, String, String)>,
    pub flagged_checkpoints: BTreeSet<String>,
    pub saved_formations: Vec<Value>,
    pub account_signed_in: bool,
    pub account_auto_switch: bool,
    pub account_label: String,
    pub pending_approvals: BTreeMap<String, mpsc::Sender<UserApprovalChoice>>,
}

impl WorkerSharedState {
    pub fn with_initial_model(initial_model: String) -> Self {
        Self {
            active_cancel: None,
            session_runs: Vec::new(),
            current_conversation: "default-conv".to_string(),
            conversations: vec![json!({ "id": "default-conv", "title": "Main Session" })],
            conversation_messages: BTreeMap::new(),
            active_provider: "openai-codex".to_string(),
            active_model: initial_model,
            active_reasoning: "default".to_string(),
            model_revision: 1,
            provider_keys: BTreeMap::new(),
            recent_selections: Vec::new(),
            flagged_checkpoints: BTreeSet::new(),
            saved_formations: Vec::new(),
            account_signed_in: true,
            account_auto_switch: true,
            account_label: "HEAV3NS Local Runtime".to_string(),
            pending_approvals: BTreeMap::new(),
        }
    }

    pub fn from_engine_config(cfg: &AgentConfig) -> Self {
        let mut st = Self::with_initial_model(cfg.model_name.clone());
        let canon = canonicalize_provider_id(&cfg.provider);
        st.active_provider = if canon.is_empty() || canon == "scripted" {
            "openai-codex".to_string()
        } else {
            canon
        };
        if let Some(ref eff) = cfg.reasoning_effort {
            let low = eff.trim().to_ascii_lowercase();
            if matches!(low.as_str(), "none" | "off" | "disabled") {
                st.active_reasoning = "none".to_string();
            } else if matches!(low.as_str(), "low" | "medium" | "high" | "xhigh") {
                st.active_reasoning = low;
            }
        }
        st.provider_keys = cfg.provider_keys.clone();
        if let Some(ucfg) = Heav3nsUserConfig::load() {
            for (k, v) in ucfg.effective_provider_keys() {
                st.provider_keys.insert(k, v);
            }
            st.recent_selections = ucfg.parsed_recent_selections();
        }
        st
    }

    pub fn record_recent(&mut self, provider: &str, model: &str, reasoning: &str) {
        let canon = canonicalize_provider_id(provider);
        let m = model.trim();
        if canon.is_empty() || canon == "scripted" || m.is_empty() {
            return;
        }
        let r = if reasoning.trim().is_empty() { "default" } else { reasoning.trim() };
        self.recent_selections.retain(|(p, em, _)| !(p == &canon && em == m));
        self.recent_selections.insert(0, (canon, m.to_string(), r.to_string()));
        self.recent_selections.truncate(8);
    }
}

pub fn emit_jsonl_line(stdout_lock: &Arc<Mutex<io::Stdout>>, payload: &Value) {
    if let Ok(mut out) = stdout_lock.lock() {
        let _ = writeln!(out, "{payload}");
        let _ = out.flush();
    }
}

/// Switches the active conversation thread while snapshotting and restoring per-thread `SessionStore` history.
pub fn switch_worker_conversation(state: &mut WorkerSharedState, engine: &AgentEngine, next_id: &str) {
    let prev_id = state.current_conversation.clone();
    if prev_id == next_id {
        return;
    }
    let cfg = engine.config_snapshot();
    {
        let mut sess = engine.session().lock().unwrap();
        state
            .conversation_messages
            .insert(prev_id, sess.messages().to_vec());
        sess.clear();
        if let Some(saved) = state.conversation_messages.get(next_id) {
            for msg in saved {
                sess.add_message(msg.clone());
            }
        } else {
            sess.add_message(SessionMessage::new(Role::System, cfg.system_prompt));
        }
    }
    state.current_conversation = next_id.to_string();
}

/// Discovers runnable workspace targets (`Cargo.toml`, `Package.swift`, `package.json`) with both `title` and `name`.
pub fn discover_project_targets(workspace_root: &Path) -> Value {
    let mut targets = Vec::new();
    if workspace_root.join("Cargo.toml").exists() {
        targets.push(json!({
            "id": "cargo-test",
            "title": "Rust Test Suite (cargo test)",
            "name": "cargo test",
            "command": "cargo test",
            "workingDirectory": "."
        }));
        targets.push(json!({
            "id": "flight-suite",
            "title": "Agent Flight Verification (--flight)",
            "name": "cargo run -- --flight",
            "command": "cargo run -- --flight",
            "workingDirectory": "."
        }));
    }
    if workspace_root.join("native/Package.swift").exists() {
        targets.push(json!({
            "id": "swift-native-test",
            "title": "Native macOS AppKit Tests (swift test)",
            "name": "swift test (native)",
            "command": "cd native && swift test",
            "workingDirectory": "native"
        }));
    } else if workspace_root.join("Package.swift").exists() {
        targets.push(json!({
            "id": "swift-test",
            "title": "Swift Package Tests (swift test)",
            "name": "swift test",
            "command": "swift test",
            "workingDirectory": "."
        }));
    }
    if workspace_root.join("package.json").exists() {
        targets.push(json!({
            "id": "npm-test",
            "title": "Node Test Suite (npm test)",
            "name": "npm test",
            "command": "npm test",
            "workingDirectory": "."
        }));
    }
    if targets.is_empty() {
        targets.push(json!({
            "id": "workspace-status",
            "title": "Workspace Status Check",
            "name": "git status",
            "command": "git status --short",
            "workingDirectory": "."
        }));
    }
    json!({ "targets": targets })
}

/// Handles `posture.{get,set,step}`, `receipts.list`, `turn.undo`, and `request.preview` RPCs.
pub fn handle_posture_and_receipts_rpc(method: &str, params: &Value, engine: &AgentEngine) -> Value {
    match method {
        "posture.get" => {
            let p = engine.current_posture();
            json!({
                "posture": p.slug(),
                "label": p.display_label(),
                "sandbox": format!("{:?}", p.sandbox_mode()).to_ascii_lowercase(),
                "stops": ["ask", "accept-edits", "plan", "auto-review", "full-access"]
            })
        }
        "posture.set" => {
            if let Some(raw) = params.get("posture").or_else(|| params.get("mode")).and_then(Value::as_str) {
                if let Some(parsed) = ExecutionPosture::parse_label(raw) {
                    engine.set_posture(parsed);
                }
            }
            let p = engine.current_posture();
            json!({
                "posture": p.slug(),
                "label": p.display_label(),
                "sandbox": format!("{:?}", p.sandbox_mode()).to_ascii_lowercase()
            })
        }
        "posture.step" => {
            let p = engine.step_posture();
            json!({
                "posture": p.slug(),
                "label": p.display_label(),
                "sandbox": format!("{:?}", p.sandbox_mode()).to_ascii_lowercase()
            })
        }
        "receipts.list" => {
            let limit = params.get("limit").and_then(Value::as_u64).unwrap_or(20) as usize;
            let all = engine.receipts_snapshot();
            let items: Vec<_> = all.into_iter().rev().take(limit.clamp(1, 64)).collect();
            json!({
                "schema": "heav3ns.receipt/v1",
                "count": items.len(),
                "receipts": items
            })
        }
        "turn.undo" => {
            let force = params.get("force").and_then(Value::as_bool).unwrap_or(false);
            match engine.undo_last_turn(force) {
                Ok(v) => v,
                Err(e) => json!({ "restoredCount": 0, "error": e, "wouldBlock": true }),
            }
        }
        "request.preview" => {
            let hint = params.get("prompt").and_then(Value::as_str).unwrap_or("");
            let manifest = engine.build_request_manifest(hint);
            serde_json::to_value(manifest).unwrap_or(Value::Null)
        }
        _ => json!({ "status": "ok" }),
    }
}

pub fn progress_to_walkthrough_activity(ev: &EngineProgressEvent) -> Value {
    let phase = match ev.phase {
        EngineProgressPhase::Planning => "planning",
        EngineProgressPhase::Connecting => "connecting",
        EngineProgressPhase::Thinking => "thinking",
        EngineProgressPhase::Tool | EngineProgressPhase::Writing => "writing",
        EngineProgressPhase::Verifying => "verifying",
        EngineProgressPhase::Responding => "responding",
        EngineProgressPhase::Completed => "completed",
        EngineProgressPhase::Deferred => "planning",
        EngineProgressPhase::Cancelled => "cancelled",
        EngineProgressPhase::Failed => "failed",
    };
    let status = match ev.status {
        EngineProgressStatus::Denied => "denied",
        EngineProgressStatus::Started => "started",
        EngineProgressStatus::InProgress => "in_progress",
        EngineProgressStatus::Completed => "completed",
        EngineProgressStatus::Deferred => "completed",
        EngineProgressStatus::Cancelled => "cancelled",
        EngineProgressStatus::Failed => "failed",
    };
    let seq = (ev.sequence as i64).max(1);
    let scope = ev.metadata.as_ref().and_then(|m| m.scope.as_deref()).unwrap_or("activity");
    let source = ev.metadata.as_ref().and_then(|m| m.source.as_deref()).unwrap_or("engine");
    let item_type = ev.metadata.as_ref().and_then(|m| m.item_type.as_deref()).unwrap_or("tool");
    json!({
        "activityId": ev.activity_id,
        "sequence": seq,
        "startedSequence": seq,
        "phase": phase,
        "status": status,
        "message": ev.message,
        "detail": ev.detail.clone().unwrap_or_default(),
        "timestamp": ev.timestamp as f64,
        "elapsedMs": ev.elapsed_ms.unwrap_or(0) as f64,
        "metadata": {
            "scope": if scope == "turn" { "turn" } else { "activity" },
            "source": source,
            "itemType": item_type,
            "operation": "execute",
            "files": []
        }
    })
}

/// Executes an async `task.submit` turn in the background without blocking the multiplexed JSON-RPC stdin loop!
#[allow(clippy::too_many_arguments)]
pub async fn execute_multiplexed_submit(
    engine: Arc<AgentEngine>,
    state: Arc<Mutex<WorkerSharedState>>,
    stdout_lock: Arc<Mutex<io::Stdout>>,
    event_seq: Arc<AtomicI64>,
    req_id: String,
    ws_id: String,
    sess_id: String,
    is_rpc: bool,
    prompt: String,
    posture_override: Option<ExecutionPosture>,
    interactive_approval: bool,
) {
    if let Some(p) = posture_override {
        engine.set_posture(p);
    }
    let before_snap = engine.create_snapshot();
    let seq_start = event_seq.fetch_add(1, Ordering::SeqCst) + 1;
    let run_id = format!("run_{}_{}", now_ms(), seq_start);
    let now = now_ms() as f64;
    let token = CancellationToken::new();
    let conv_id = {
        let mut guard = state.lock().unwrap();
        guard.active_cancel = Some(token.clone());
        guard.current_conversation.clone()
    };

    let run_rev = Arc::new(AtomicI64::new(1));
    let mut run_obj = json!({
        "id": run_id,
        "requestId": req_id,
        "text": prompt,
        "session": sess_id,
        "conversation": conv_id,
        "posture": engine.current_posture().slug(),
        "state": "running",
        "revision": 1,
        "createdAt": now,
        "startedAt": now,
        "finishedAt": null,
        "checkpointBefore": before_snap.snapshot_id,
        "activities": [],
        "activitySequence": 0,
        "activityCount": 0
    });

    {
        let mut guard = state.lock().unwrap();
        guard.session_runs.retain(|r| r.get("id").and_then(|v| v.as_str()) != Some(&run_id));
        guard.session_runs.push(run_obj.clone());
        while guard.session_runs.len() > 64 {
            guard.session_runs.remove(0);
        }
    }

    if is_rpc {
        emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "accepted": true, "run": run_obj } }));
        emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "event", "workspace": ws_id, "session": sess_id, "runtime": "heav3ns-micro", "seq": seq_start, "type": "run.state", "payload": run_obj }));
    }

    let mut input = EngineTickInput::new(prompt.clone());
    input.execution_authority = Some(ExecutionAuthorityLevel::Autonomous);
    input.posture = posture_override;
    input.cancel_token = Some(token.clone());

    if is_rpc && interactive_approval {
        let st_approve = Arc::clone(&state);
        let out_approve = Arc::clone(&stdout_lock);
        let seq_approve = Arc::clone(&event_seq);
        let rev_approve = Arc::clone(&run_rev);
        let (ws_a, sess_a, run_a) = (ws_id.clone(), sess_id.clone(), run_id.clone());
        let ws_scope = engine.workspace_root().to_string_lossy().to_string();
        let base_run = run_obj.clone();
        let tok_a = token.clone();
        input.approval_callback = Some(Arc::new(move |tool_name: &str, summary: &str, risk_reason: &str| {
            let approval_id = format!("apr_{}_{}", now_ms(), seq_approve.fetch_add(1, Ordering::SeqCst) + 1);
            let (tx, rx) = mpsc::channel();
            let waiting_rev = rev_approve.fetch_add(1, Ordering::SeqCst) + 1;
            let mut waiting_run = base_run.clone();
            waiting_run["state"] = json!("awaiting_approval");
            waiting_run["revision"] = json!(waiting_rev);
            waiting_run["approval"] = json!({
                "id": approval_id,
                "operation": approval_id,
                "tool": tool_name,
                "args": summary,
                "scope": ws_scope,
                "reason": risk_reason,
                "choices": ["allow_once", "allow_session", "deny"]
            });
            {
                let mut guard = st_approve.lock().unwrap();
                guard.pending_approvals.insert(approval_id.clone(), tx);
                if let Some(existing) = guard.session_runs.iter_mut().find(|r| r.get("id").and_then(|v| v.as_str()) == Some(&run_a)) {
                    *existing = waiting_run.clone();
                }
            }
            let s = seq_approve.fetch_add(1, Ordering::SeqCst) + 1;
            emit_jsonl_line(
                &out_approve,
                &json!({
                    "v": 1,
                    "kind": "event",
                    "workspace": ws_a,
                    "session": sess_a,
                    "runtime": "heav3ns-micro",
                    "seq": s,
                    "type": "approval.requested",
                    "run": run_a,
                    "payload": waiting_run
                }),
            );
            let deadline = Instant::now() + Duration::from_secs(120);
            let decision = loop {
                if tok_a.is_cancelled() || Instant::now() >= deadline {
                    break UserApprovalChoice::Deny;
                }
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(choice) => break choice,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break UserApprovalChoice::Deny,
                }
            };
            let resumed_rev = rev_approve.fetch_add(1, Ordering::SeqCst) + 1;
            let mut resumed_run = waiting_run;
            resumed_run["state"] = json!("running");
            resumed_run["revision"] = json!(resumed_rev);
            resumed_run["approval"]["decision"] = json!(format!("{decision:?}").to_ascii_lowercase());
            {
                let mut guard = st_approve.lock().unwrap();
                guard.pending_approvals.remove(&approval_id);
                if let Some(existing) = guard.session_runs.iter_mut().find(|r| r.get("id").and_then(|v| v.as_str()) == Some(&run_a)) {
                    *existing = resumed_run.clone();
                }
            }
            let s_res = seq_approve.fetch_add(1, Ordering::SeqCst) + 1;
            emit_jsonl_line(
                &out_approve,
                &json!({
                    "v": 1,
                    "kind": "event",
                    "workspace": ws_a,
                    "session": sess_a,
                    "runtime": "heav3ns-micro",
                    "seq": s_res,
                    "type": "approval.resolved",
                    "run": run_a,
                    "payload": resumed_run
                }),
            );
            decision
        }));
    }

    let live_acts = Arc::new(Mutex::new(Vec::<Value>::new()));
    let live_acts_cb = Arc::clone(&live_acts);
    let seq_cb = Arc::clone(&event_seq);
    let out_prog = Arc::clone(&stdout_lock);
    let (req_id_prog, ws_p, sess_p, run_p) = (req_id.clone(), ws_id.clone(), sess_id.clone(), run_id.clone());
    input.on_progress = Some(Arc::new(move |ev| {
        let act = progress_to_walkthrough_activity(&ev);
        {
            let mut guard = live_acts_cb.lock().unwrap();
            if let Some(existing) = guard.iter_mut().find(|a| a.get("activityId") == act.get("activityId")) {
                *existing = act.clone();
            } else if guard.len() < 120 {
                guard.push(act.clone());
            }
        }
        if is_rpc {
            let s = seq_cb.fetch_add(1, Ordering::SeqCst) + 1;
            emit_jsonl_line(&out_prog, &json!({ "v": 1, "kind": "event", "workspace": ws_p, "session": sess_p, "runtime": "heav3ns-micro", "seq": s, "type": "activity", "run": run_p, "payload": act }));
        } else {
            emit_jsonl_line(&out_prog, &json!({ "v": 1, "id": req_id_prog, "type": "progress", "event": ev }));
        }
    }));

    let result = engine.tick(input).await;
    let after_snap = engine.create_snapshot();
    let finish_now = now_ms() as f64;
    let unsent_auth = result.outcome == EngineTickOutcome::Failed
        && result.response.contains(CREDENTIAL_REJECTED_UNSENT_CODE);
    let (final_state, resp_txt, err_txt) = match result.outcome {
        EngineTickOutcome::Completed => ("completed", Some(result.response.clone()), None),
        EngineTickOutcome::Deferred => ("awaiting_approval", Some(result.response.clone()), None),
        EngineTickOutcome::Cancelled => ("cancelled", None, Some("Execution was cancelled".to_string())),
        EngineTickOutcome::Failed => ("failed", None, Some(result.response.clone())),
    };

    let acts = live_acts.lock().unwrap().clone();
    let max_act_seq = acts.iter().filter_map(|a| a.get("sequence").and_then(|v| v.as_i64())).max().unwrap_or(0);
    let final_rev = run_rev.fetch_add(1, Ordering::SeqCst) + 1;
    run_obj["state"] = json!(final_state);
    run_obj["revision"] = json!(final_rev);
    run_obj["finishedAt"] = json!(finish_now);
    run_obj["checkpointAfter"] = json!(after_snap.snapshot_id);
    run_obj["activities"] = json!(acts);
    run_obj["activitySequence"] = json!(max_act_seq);
    run_obj["activityCount"] = json!(acts.len());
    run_obj["unsentAuthRejected"] = json!(unsent_auth);
    if final_state == "awaiting_approval" && !run_obj.get("approval").is_some_and(Value::is_object) {
        run_obj["approval"] = json!({
            "operation": format!("defer_{run_id}"),
            "tool": "approval_gate",
            "args": prompt,
            "scope": engine.workspace_root().to_string_lossy().to_string(),
            "reason": result.response.clone()
        });
    }
    if let Some(ref rec) = result.receipt {
        run_obj["receipt"] = serde_json::to_value(rec).unwrap_or(Value::Null);
    }
    if let Some(r) = resp_txt {
        run_obj["response"] = json!(r);
    }
    if let Some(e) = err_txt {
        run_obj["error"] = json!(e);
    }

    let changed_catalog = {
        let mut guard = state.lock().unwrap();
        guard.active_cancel = None;
        let mut cat_ev = None;
        if result.is_slash_command {
            let snap = engine.config_snapshot();
            let next_prov = canonicalize_provider_id(&snap.provider);
            let next_reas = snap.reasoning_effort.unwrap_or_else(|| "default".to_string());
            if (!next_prov.is_empty() && next_prov != "scripted" && next_prov != guard.active_provider)
                || snap.model_name != guard.active_model
                || next_reas != guard.active_reasoning
            {
                if !next_prov.is_empty() && next_prov != "scripted" {
                    guard.active_provider = next_prov;
                }
                guard.active_model = snap.model_name;
                guard.active_reasoning = next_reas;
                let p_copy = guard.active_provider.clone();
                let m_copy = guard.active_model.clone();
                let r_copy = guard.active_reasoning.clone();
                guard.record_recent(&p_copy, &m_copy, &r_copy);
                guard.model_revision += 1;
                cat_ev = Some(build_sovereign_model_catalog_with_recent(
                    &guard.active_provider,
                    &guard.active_model,
                    &guard.active_reasoning,
                    guard.model_revision,
                    &[],
                    &guard.recent_selections,
                ));
            }
        }
        if let Some(existing) = guard.session_runs.iter_mut().find(|r| r.get("id").and_then(|v| v.as_str()) == Some(&run_id)) {
            *existing = run_obj.clone();
        } else {
            guard.session_runs.push(run_obj.clone());
        }
        while guard.session_runs.len() > 64 {
            guard.session_runs.remove(0);
        }
        cat_ev
    };

    if is_rpc {
        if let Some(cat_payload) = changed_catalog {
            let s_cat = event_seq.fetch_add(1, Ordering::SeqCst) + 1;
            emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "event", "workspace": ws_id, "session": sess_id, "runtime": "heav3ns-micro", "seq": s_cat, "type": "models.changed", "payload": cat_payload }));
        }
        let s_end = event_seq.fetch_add(1, Ordering::SeqCst) + 1;
        emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "kind": "event", "workspace": ws_id, "session": sess_id, "runtime": "heav3ns-micro", "seq": s_end, "type": "run.state", "payload": run_obj }));
    } else {
        emit_jsonl_line(&stdout_lock, &json!({ "v": 1, "id": req_id, "type": "result", "result": result }));
    }
}
