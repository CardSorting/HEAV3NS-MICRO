// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::domain::pricing_and_taxonomy::CREDENTIAL_REJECTED_UNSENT_CODE;
use crate::domain::types::provider_registry::canonicalize_provider_id;
use crate::domain::types::runtime::{
    CancellationToken, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel,
    ExecutionPosture, UserApprovalChoice,
};
use crate::plumbing::hash_and_math::{lock_unpoison, now_ms};
pub use crate::ui::provider_rpc::{
    build_sovereign_model_catalog, build_sovereign_model_catalog_with_recent,
    build_sovereign_model_discovery, handle_provider_credential_rpc,
};
use serde_json::{json, Value};
use std::io::{self};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::ui::worker_handlers::{emit_jsonl_line, emit_seq_event, progress_to_walkthrough_activity, WorkerSharedState};

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
    turn_gate: Arc<tokio::sync::Mutex<()>>,
) {
    let effective_posture = posture_override.unwrap_or_else(|| engine.current_posture());
    let seq_start = event_seq.fetch_add(1, Ordering::SeqCst) + 1;
    let run_id = format!("run_{}_{}", now_ms(), seq_start);
    let token = CancellationToken::new();
    let conv_id = {
        let mut guard = lock_unpoison(&state);
        guard.active_cancels.insert(run_id.clone(), token.clone());
        guard.current_conversation.clone()
    };
    let _turn_guard = turn_gate.lock().await;
    let before_snap = engine.create_snapshot();
    let now = now_ms() as f64;

    let run_rev = Arc::new(AtomicI64::new(1));
    let mut run_obj = json!({
        "id": run_id,
        "requestId": req_id,
        "text": prompt,
        "session": sess_id,
        "conversation": conv_id,
        "posture": effective_posture.slug(),
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
        let mut guard = lock_unpoison(&state);
        guard
            .session_runs
            .retain(|r| r.get("id").and_then(|v| v.as_str()) != Some(&run_id));
        guard.session_runs.push(run_obj.clone());
        while guard.session_runs.len() > 64 {
            guard.session_runs.remove(0);
        }
    }

    if is_rpc {
        emit_jsonl_line(
            &stdout_lock,
            &json!({ "v": 1, "kind": "response", "id": req_id, "result": { "accepted": true, "run": run_obj } }),
        );
        emit_seq_event(
            &stdout_lock,
            &event_seq,
            json!({ "v": 1, "kind": "event", "workspace": ws_id, "session": sess_id, "runtime": "heav3ns-micro", "type": "run.state", "payload": run_obj }),
        );
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
        input.approval_callback = Some(Arc::new(
            move |tool_name: &str, summary: &str, risk_reason: &str| {
                let approval_id = format!(
                    "apr_{}_{}",
                    now_ms(),
                    seq_approve.fetch_add(1, Ordering::SeqCst) + 1
                );
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
                    let mut guard = lock_unpoison(&st_approve);
                    guard.pending_approvals.insert(approval_id.clone(), tx);
                    if let Some(existing) = guard
                        .session_runs
                        .iter_mut()
                        .find(|r| r.get("id").and_then(|v| v.as_str()) == Some(&run_a))
                    {
                        *existing = waiting_run.clone();
                    }
                }
                emit_seq_event(
                    &out_approve,
                    &seq_approve,
                    json!({
                        "v": 1,
                        "kind": "event",
                        "workspace": ws_a,
                        "session": sess_a,
                        "runtime": "heav3ns-micro",
                        "type": "approval.requested",
                        "run": run_a,
                        "payload": waiting_run
                    }),
                );
                let deadline = Instant::now() + Duration::from_secs(120);
                let tok_wait = tok_a.clone();
                let wait_decision = move || loop {
                    if tok_wait.is_cancelled() || Instant::now() >= deadline {
                        break UserApprovalChoice::Deny;
                    }
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(choice) => break choice,
                        Err(RecvTimeoutError::Timeout) => continue,
                        Err(RecvTimeoutError::Disconnected) => break UserApprovalChoice::Deny,
                    }
                };
                let decision = if tokio::runtime::Handle::try_current()
                    .is_ok_and(|h| h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread)
                {
                    tokio::task::block_in_place(wait_decision)
                } else {
                    std::thread::spawn(wait_decision)
                        .join()
                        .unwrap_or(UserApprovalChoice::Deny)
                };
                let resumed_rev = rev_approve.fetch_add(1, Ordering::SeqCst) + 1;
                let mut resumed_run = waiting_run;
                resumed_run["state"] = json!("running");
                resumed_run["revision"] = json!(resumed_rev);
                resumed_run["approval"]["decision"] =
                    json!(format!("{decision:?}").to_ascii_lowercase());
                {
                    let mut guard = lock_unpoison(&st_approve);
                    guard.pending_approvals.remove(&approval_id);
                    if let Some(existing) = guard
                        .session_runs
                        .iter_mut()
                        .find(|r| r.get("id").and_then(|v| v.as_str()) == Some(&run_a))
                    {
                        *existing = resumed_run.clone();
                    }
                }
                emit_seq_event(
                    &out_approve,
                    &seq_approve,
                    json!({
                        "v": 1,
                        "kind": "event",
                        "workspace": ws_a,
                        "session": sess_a,
                        "runtime": "heav3ns-micro",
                        "type": "approval.resolved",
                        "run": run_a,
                        "payload": resumed_run
                    }),
                );
                decision
            },
        ));
    }

    let live_acts = Arc::new(Mutex::new(Vec::<Value>::new()));
    let live_acts_cb = Arc::clone(&live_acts);
    let seq_cb = Arc::clone(&event_seq);
    let out_prog = Arc::clone(&stdout_lock);
    let (req_id_prog, ws_p, sess_p, run_p) = (
        req_id.clone(),
        ws_id.clone(),
        sess_id.clone(),
        run_id.clone(),
    );
    input.on_progress = Some(Arc::new(move |ev| {
        let act = progress_to_walkthrough_activity(&ev);
        {
            let mut guard = lock_unpoison(&live_acts_cb);
            if let Some(existing) = guard
                .iter_mut()
                .find(|a| a.get("activityId") == act.get("activityId"))
            {
                *existing = act.clone();
            } else if guard.len() < 120 {
                guard.push(act.clone());
            }
        }
        if is_rpc {
            emit_seq_event(
                &out_prog,
                &seq_cb,
                json!({ "v": 1, "kind": "event", "workspace": ws_p, "session": sess_p, "runtime": "heav3ns-micro", "type": "activity", "run": run_p, "payload": act }),
            );
        } else {
            emit_jsonl_line(
                &out_prog,
                &json!({ "v": 1, "id": req_id_prog, "type": "progress", "event": ev }),
            );
        }
    }));

    let result = engine.tick(input).await;
    let after_snap = if result.outcome == EngineTickOutcome::Failed && result.receipt.is_none() {
        before_snap.clone()
    } else {
        engine.create_snapshot()
    };
    let finish_now = now_ms() as f64;
    let unsent_auth = result.outcome == EngineTickOutcome::Failed
        && result.response.contains(CREDENTIAL_REJECTED_UNSENT_CODE);
    let (final_state, resp_txt, err_txt) = match result.outcome {
        EngineTickOutcome::Completed => ("completed", Some(result.response.clone()), None),
        EngineTickOutcome::Deferred => ("awaiting_approval", Some(result.response.clone()), None),
        EngineTickOutcome::Cancelled => (
            "cancelled",
            None,
            Some("Execution was cancelled".to_string()),
        ),
        EngineTickOutcome::Failed => ("failed", None, Some(result.response.clone())),
    };

    let acts = lock_unpoison(&live_acts).clone();
    let max_act_seq = acts
        .iter()
        .filter_map(|a| a.get("sequence").and_then(|v| v.as_i64()))
        .max()
        .unwrap_or(0);
    let final_rev = run_rev.fetch_add(1, Ordering::SeqCst) + 1;
    run_obj["state"] = json!(final_state);
    run_obj["revision"] = json!(final_rev);
    run_obj["finishedAt"] = json!(finish_now);
    run_obj["checkpointAfter"] = json!(after_snap.snapshot_id);
    run_obj["activities"] = json!(acts);
    run_obj["activitySequence"] = json!(max_act_seq);
    run_obj["activityCount"] = json!(acts.len());
    run_obj["unsentAuthRejected"] = json!(unsent_auth);
    if final_state == "awaiting_approval" && !run_obj.get("approval").is_some_and(Value::is_object)
    {
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
        let mut guard = lock_unpoison(&state);
        guard.active_cancels.remove(&run_id);
        let mut cat_ev = None;
        if result.is_slash_command {
            let snap = engine.config_snapshot();
            let next_prov = canonicalize_provider_id(&snap.provider);
            let next_reas = snap
                .reasoning_effort
                .unwrap_or_else(|| "default".to_string());
            if (!next_prov.is_empty()
                && next_prov != "scripted"
                && next_prov != guard.active_provider)
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
        if let Some(existing) = guard
            .session_runs
            .iter_mut()
            .find(|r| r.get("id").and_then(|v| v.as_str()) == Some(&run_id))
        {
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
            emit_seq_event(
                &stdout_lock,
                &event_seq,
                json!({ "v": 1, "kind": "event", "workspace": ws_id, "session": sess_id, "runtime": "heav3ns-micro", "type": "models.changed", "payload": cat_payload }),
            );
        }
        emit_seq_event(
            &stdout_lock,
            &event_seq,
            json!({ "v": 1, "kind": "event", "workspace": ws_id, "session": sess_id, "runtime": "heav3ns-micro", "type": "run.state", "payload": run_obj }),
        );
    } else {
        emit_jsonl_line(
            &stdout_lock,
            &json!({ "v": 1, "id": req_id, "type": "result", "result": result }),
        );
    }
}

