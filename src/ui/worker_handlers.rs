// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::progress::{
    EngineProgressEvent, EngineProgressPhase, EngineProgressStatus,
};
use crate::domain::types::provider_registry::canonicalize_provider_id;
use crate::domain::types::runtime::{
    AgentConfig, CancellationToken, ExecutionAuthorityLevel,
    ExecutionPosture, UserApprovalChoice,
};
use crate::plumbing::hash_and_math::lock_unpoison;
use crate::ui::cli_presenter::Heav3nsUserConfig;
pub use crate::ui::provider_rpc::{
    build_sovereign_model_catalog, build_sovereign_model_catalog_with_recent,
    build_sovereign_model_discovery, handle_provider_credential_rpc,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{self};
use std::sync::{Arc, Mutex};



/// Shared multiplexed state across the async JSON-RPC stdin reader and background turn tasks.
pub struct WorkerSharedState {
    pub active_cancels: BTreeMap<String, CancellationToken>,
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
            active_cancels: BTreeMap::new(),
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
        let has_openai_env = std::env::var("OPENAI_API_KEY").ok().is_some_and(|v| !v.trim().is_empty());
        if st.active_provider == "openai"
            && !st.provider_keys.contains_key("openai")
            && !has_openai_env
            && AgentEngine::default_account_store().has_active_account()
        {
            st.active_provider = "openai-codex".to_string();
        }
        st
    }

    pub fn record_recent(&mut self, provider: &str, model: &str, reasoning: &str) {
        let canon = canonicalize_provider_id(provider);
        let m = model.trim();
        if canon.is_empty() || canon == "scripted" || m.is_empty() {
            return;
        }
        let r = if reasoning.trim().is_empty() {
            "default"
        } else {
            reasoning.trim()
        };
        self.recent_selections
            .retain(|(p, em, _)| !(p == &canon && em == m));
        self.recent_selections
            .insert(0, (canon, m.to_string(), r.to_string()));
        self.recent_selections.truncate(8);
    }
}

pub fn emit_jsonl_line(stdout_lock: &Arc<Mutex<io::Stdout>>, payload: &Value) {
    let mut out = lock_unpoison(stdout_lock);
    let _ = serde_json::to_writer(&mut *out, payload);
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

/// Allocates `event_seq` while holding `stdout_lock` so concurrent JSONL events emit in monotonic `seq` order.
pub fn emit_seq_event(
    stdout_lock: &Arc<Mutex<io::Stdout>>,
    event_seq: &AtomicI64,
    mut event: Value,
) {
    let mut out = lock_unpoison(stdout_lock);
    let seq = event_seq.fetch_add(1, Ordering::SeqCst) + 1;
    event["seq"] = json!(seq);
    let _ = serde_json::to_writer(&mut *out, &event);
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

/// Resolves a recorded worker run by exact `id` or by the 64-hex `state.inspect` digest (`sha256("<id>:<rev_idx>")`).
pub fn resolve_worker_run<'a>(runs: &'a [Value], id_or_hash: &str) -> Option<&'a Value> {
    if let Some(found) = runs
        .iter()
        .find(|r| r.get("id").and_then(|s| s.as_str()) == Some(id_or_hash))
    {
        return Some(found);
    }
    if id_or_hash.len() == 64 && id_or_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        for (idx, r) in runs.iter().rev().take(20).enumerate() {
            let raw_id = r.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if sha256_hex_ui(&format!("{raw_id}:{idx}")) == id_or_hash {
                return Some(r);
            }
        }
    }
    None
}

pub fn sha256_hex_ui(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Paginates UTF-8 text on char boundaries with SHA-256 revision stamping.
pub fn paginate_text_response(text: &str, rev_prefix: &str, offset: usize, limit: usize) -> Value {
    let total = text.len();
    let mut safe_offset = offset.min(total);
    while safe_offset > 0 && !text.is_char_boundary(safe_offset) {
        safe_offset -= 1;
    }
    let mut end = safe_offset.saturating_add(limit).min(total);
    while end > safe_offset && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end == safe_offset && safe_offset < total {
        end = safe_offset + 1;
        while end < total && !text.is_char_boundary(end) {
            end += 1;
        }
    }
    let rev = format!("{rev_prefix}-{}", &sha256_hex_ui(text)[..12]);
    json!({
        "text": &text[safe_offset..end],
        "binary": false,
        "revision": rev,
        "offset": safe_offset,
        "nextOffset": end,
        "size": total,
        "more": end < total
    })
}

pub async fn read_bounded_stdin_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    max_bytes: usize,
) -> std::io::Result<Option<String>> {
    use tokio::io::AsyncBufReadExt;
    let mut buf = Vec::new();
    loop {
        let chunk = reader.fill_buf().await?;
        if chunk.is_empty() {
            return if buf.is_empty() {
                Ok(None)
            } else {
                String::from_utf8(buf)
                    .map(Some)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            };
        }
        if let Some(pos) = chunk.iter().position(|&b| b == b'\n') {
            let take = pos + 1;
            if buf.len().saturating_add(take) > max_bytes {
                reader.consume(take);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("JSONL frame exceeds {max_bytes} byte cap"),
                ));
            }
            buf.extend_from_slice(&chunk[..take]);
            reader.consume(take);
            return String::from_utf8(buf)
                .map(Some)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e));
        }
        let n = chunk.len();
        if buf.len().saturating_add(n) > max_bytes {
            reader.consume(n);
            while let Ok(more) = reader.fill_buf().await {
                if more.is_empty() {
                    break;
                }
                if let Some(p) = more.iter().position(|&b| b == b'\n') {
                    reader.consume(p + 1);
                    break;
                }
                let m = more.len();
                reader.consume(m);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("JSONL frame exceeds {max_bytes} byte cap"),
            ));
        }
        buf.extend_from_slice(chunk);
        reader.consume(n);
    }
}

/// Builds a paginated `artifact.read` response from a recorded `run` output when `params["run"]` is present.
pub fn build_artifact_run_response(runs: &[Value], params: &Value) -> Option<Value> {
    let run_id = params.get("run").and_then(|v| v.as_str())?;
    let text = resolve_worker_run(runs, run_id)
        .and_then(|r| {
            r.get("response")
                .or_else(|| r.get("error"))
                .or_else(|| r.get("text"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("");
    let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let limit = (params
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(65536) as usize)
        .clamp(1, 512 * 1024);
    Some(paginate_text_response(text, "artifact", offset, limit))
}

/// Switches the active conversation thread while snapshotting and restoring per-thread `SessionStore` history.
pub fn switch_worker_conversation(
    state: &mut WorkerSharedState,
    engine: &AgentEngine,
    next_id: &str,
) {
    let prev_id = state.current_conversation.clone();
    if prev_id == next_id {
        return;
    }
    let cfg = engine.config_snapshot();
    let keep_prev = state
        .conversations
        .iter()
        .any(|c| c.get("id").and_then(Value::as_str) == Some(&prev_id));
    {
        let mut sess = lock_unpoison(&engine.session());
        if keep_prev {
            state
                .conversation_messages
                .insert(prev_id, sess.messages().to_vec());
        }
        sess.clear();
        if let Some(saved) = state
            .conversation_messages
            .get(next_id)
            .filter(|m| !m.is_empty())
        {
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
/// Handles `computer.*`, `browser.*`, `spatial.*`, `skills.*`, and `soul.status` RPCs.
pub async fn handle_automation_rpc(
    method: &str,
    req_id: &str,
    params: &Value,
    engine: &AgentEngine,
) -> Value {
    match method {
        "computer.action" | "computer.capture" | "computer.health" => {
            let mut req = params.clone();
            if method == "computer.capture" && req.get("action").is_none() {
                req["action"] = json!("capture");
            }
            if method == "computer.health" && req.get("action").is_none() {
                req["action"] = json!("audit_health");
            }
            if method == "computer.action" {
                let call = crate::domain::types::tool_schedule::ScheduledToolCall {
                    id: format!("req-{req_id}"),
                    name: "computer_use".into(),
                    args: req,
                };
                let res = engine
                    .tools()
                    .execute_single(
                        &call,
                        engine.workspace_root(),
                        ExecutionAuthorityLevel::Autonomous,
                        None,
                    )
                    .await;
                if res.success {
                    res.output
                } else {
                    json!({"error": res.error})
                }
            } else {
                lock_unpoison(&engine.tools().display_state)
                    .execute_action(&req)
                    .unwrap_or_else(|e| json!({"error": e}))
            }
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
            let res = engine
                .tools()
                .execute_single(
                    &call,
                    engine.workspace_root(),
                    ExecutionAuthorityLevel::Autonomous,
                    None,
                )
                .await;
            if res.success {
                res.output
            } else {
                json!({"error": res.error})
            }
        }
        "computer.patterns" => {
            let kstore = lock_unpoison(&engine.tools().computer_orchestrator.knowledge_store);
            json!({ "patterns": kstore.list_patterns(), "procedures": kstore.list_procedures() })
        }
        "browser.action" | "browser.snapshot" | "browser.navigate" => {
            let mut req = params.clone();
            if method == "browser.snapshot" && req.get("action").is_none() {
                req["action"] = json!("snapshot");
            }
            if method == "browser.navigate" && req.get("action").is_none() {
                req["action"] = json!("navigate");
            }
            if method == "browser.snapshot" {
                lock_unpoison(&engine.tools().display_state)
                    .execute_browser_action(&req)
                    .unwrap_or_else(|e| json!({"error": e}))
            } else {
                let call = crate::domain::types::tool_schedule::ScheduledToolCall {
                    id: format!("req-{req_id}"),
                    name: "browser_use".into(),
                    args: req,
                };
                let res = engine
                    .tools()
                    .execute_single(
                        &call,
                        engine.workspace_root(),
                        ExecutionAuthorityLevel::Autonomous,
                        None,
                    )
                    .await;
                if res.success {
                    res.output
                } else {
                    json!({"error": res.error})
                }
            }
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
            let res = engine
                .tools()
                .execute_single(
                    &call,
                    engine.workspace_root(),
                    ExecutionAuthorityLevel::Autonomous,
                    None,
                )
                .await;
            if res.success {
                res.output
            } else {
                json!({"error": res.error})
            }
        }
        _ => {
            let mutating = method == "skills.forge"
                || (method == "skills.lint"
                    && params
                        .get("autofix")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false));
            if mutating && engine.current_posture() == ExecutionPosture::Plan {
                return json!({ "error": "ExecutionPosture::Plan forbids skill mutations." });
            }
            let mut cat = lock_unpoison(&engine.tools().skill_catalog);
            match method {
                "skills.forge" => cat
                    .forge_skill(params)
                    .unwrap_or_else(|e| json!({"error": e})),
                "skills.lint" => {
                    let id = params
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("computer-use-operator");
                    cat.lint_and_autofix_skill(
                        id,
                        params
                            .get("autofix")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                    )
                    .unwrap_or_else(|e| json!({"error": e}))
                }
                "soul.status" => {
                    json!({ "activeSoul": cat.active_soul, "competencies": cat.competency_scores })
                }
                _ => cat.evaluate_dag(),
            }
        }
    }
}

/// Handles `posture.{get,set,step}`, `receipts.list`, `turn.undo`, and `request.preview` RPCs.
pub fn handle_posture_and_receipts_rpc(
    method: &str,
    params: &Value,
    engine: &AgentEngine,
) -> Value {
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
            if let Some(raw) = params
                .get("posture")
                .or_else(|| params.get("mode"))
                .and_then(Value::as_str)
            {
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
            let force = params
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
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
    let scope = ev
        .metadata
        .as_ref()
        .and_then(|m| m.scope.as_deref())
        .unwrap_or("activity");
    let source = ev
        .metadata
        .as_ref()
        .and_then(|m| m.source.as_deref())
        .unwrap_or("engine");
    let item_type = ev
        .metadata
        .as_ref()
        .and_then(|m| m.item_type.as_deref())
        .unwrap_or("tool");
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave10_worker_artifact_read_and_hash_run_resolution() {
        let runs = vec![json!({ "id": "run_abc", "response": "artifact-body-123" })];
        let hash_id = sha256_hex_ui("run_abc:0");
        assert_eq!(
            resolve_worker_run(&runs, &hash_id).and_then(|r| r["id"].as_str()),
            Some("run_abc")
        );
        let art =
            build_artifact_run_response(&runs, &json!({ "run": "run_abc", "offset": 0 })).unwrap();
        assert_eq!(art["text"], json!("artifact-body-123"));
    }

    #[tokio::test]
    async fn test_wave18_artifact_limit_clamp_and_bounded_stdin() {
        let runs = vec![json!({ "id": "r1", "response": "abcdef" })];
        let resp =
            build_artifact_run_response(&runs, &json!({ "run": "r1", "offset": 0, "limit": 0 }))
                .unwrap();
        assert_eq!(resp["nextOffset"], 1);

        let mut cursor =
            tokio::io::BufReader::new(std::io::Cursor::new(b"oversized_frame\nok\n".as_slice()));
        let err = read_bounded_stdin_line(&mut cursor, 8).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(
            read_bounded_stdin_line(&mut cursor, 8)
                .await
                .unwrap()
                .as_deref(),
            Some("ok\n")
        );
    }
}
