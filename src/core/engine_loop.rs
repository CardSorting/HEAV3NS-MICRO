// [LAYER: CORE]
use crate::core::request_queue::PromptComposer;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::interfaces::ILlmProvider;
use crate::domain::prefix_and_manifest::PrefixFingerprint;
use crate::domain::pricing_and_taxonomy::{ProviderTokenUsage, CREDENTIAL_REJECTED_UNSENT_CODE};
use crate::domain::profiles::{AgentProfile, CiGateReceipt};
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::progress::{
    EngineProgressMetadata, EngineProgressPhase, EngineProgressStatus,
    ToolExecutionRecord,
};
use crate::domain::types::runtime::{
    AgentConfig, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel, ExecutionPosture,
};
use crate::domain::types::tick_result::{EngineTickResult, ProviderError, ProviderRequest};
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::seatbelt_profile::{inspect_command_security, CommandSecurityVerdict};
use crate::plumbing::secret_redactor::DeterministicSecretRedactor;
use crate::plumbing::text_tokens::{canonical_tool_name, estimate_text_tokens};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::core::engine::AgentEngine;

impl AgentEngine {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_tick_rounds(
        &self,
        input: EngineTickInput,
        cfg: &AgentConfig,
        turn_num: u64,
        frame_index: u64,
        effective_posture: ExecutionPosture,
        authority: ExecutionAuthorityLevel,
        snap_before: Option<String>,
        started_ms: u64,
        turn_activity_id: &str,
        max_rounds: usize,
        mut active_model: String,
        mut is_fallback_model: bool,
        mut turn_route: Option<(Arc<dyn ILlmProvider>, String, String)>,
        mut all_tool_results: Vec<ToolExecutionRecord>,
        mut tools_admitted: bool,
        mut last_composed_summary: String,
        mut final_response: String,
        mut accumulated_usage: ProviderTokenUsage,
        mut last_prefix_hash: String,
        mut last_prefix_status: String,
        mut completed_with_assistant_reply: bool,
    ) -> EngineTickResult {
        let mut dev_mutations_performed = false;
        let mut ci_gate_checked_after_mutation = false;
        let mut ci_gate_failed_blocking = false;
        let mut last_ci_receipt: Option<CiGateReceipt> = None;
        for round in 0..max_rounds {
            if input
                .cancel_token
                .as_ref()
                .map(|t| t.is_cancelled())
                .unwrap_or(false)
            {
                let msg = "Turn cancelled by user.".to_string();
                {
                    let mut sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                    Self::heal_dangling_tools(&mut sess, "[Turn cancelled by user]");
                }
                self.emit_progress_internal(
                    &input.on_progress,
                    &turn_activity_id,
                    EngineProgressPhase::Cancelled,
                    EngineProgressStatus::Cancelled,
                    &msg,
                    None,
                    started_ms,
                    None,
                );
                let dur = now_ms().saturating_sub(started_ms);
                let (r_prov, r_ep) = turn_route
                    .as_ref()
                    .map(|(_, p, e)| (p.as_str(), e.as_str()))
                    .unwrap_or((&cfg.provider, &cfg.api_endpoint));
                let receipt = self.build_and_persist_turn_receipt(
                    turn_num,
                    frame_index,
                    r_prov,
                    r_ep,
                    &active_model,
                    effective_posture,
                    EngineTickOutcome::Cancelled,
                    dur,
                    accumulated_usage,
                    last_prefix_hash,
                    last_prefix_status,
                    snap_before,
                    &all_tool_results,
                    None,
                );
                return EngineTickResult {
                    frame_index,
                    outcome: EngineTickOutcome::Cancelled,
                    active_model,
                    is_fallback_model,
                    is_slash_command: false,
                    composed_prompt: last_composed_summary,
                    response: msg,
                    tool_results: all_tool_results,
                    safe_to_retry: Some(!tools_admitted),
                    duration_ms: dur,
                    receipt: Some(receipt),
                };
            }

            if round > 0 && input.should_defer.as_ref().map(|f| f()).unwrap_or(false) {
                let msg = "Turn deferred between tool rounds.".to_string();
                {
                    let mut sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                    Self::heal_dangling_tools(&mut sess, "[Turn deferred between tool rounds]");
                }
                self.emit_progress_internal(
                    &input.on_progress,
                    &turn_activity_id,
                    EngineProgressPhase::Deferred,
                    EngineProgressStatus::Deferred,
                    &msg,
                    None,
                    started_ms,
                    None,
                );
                let dur = now_ms().saturating_sub(started_ms);
                let (r_prov, r_ep) = turn_route
                    .as_ref()
                    .map(|(_, p, e)| (p.as_str(), e.as_str()))
                    .unwrap_or((&cfg.provider, &cfg.api_endpoint));
                let receipt = self.build_and_persist_turn_receipt(
                    turn_num,
                    frame_index,
                    r_prov,
                    r_ep,
                    &active_model,
                    effective_posture,
                    EngineTickOutcome::Deferred,
                    dur,
                    accumulated_usage,
                    last_prefix_hash,
                    last_prefix_status,
                    snap_before,
                    &all_tool_results,
                    None,
                );
                return EngineTickResult {
                    frame_index,
                    outcome: EngineTickOutcome::Deferred,
                    active_model,
                    is_fallback_model,
                    is_slash_command: false,
                    composed_prompt: last_composed_summary,
                    response: msg,
                    tool_results: all_tool_results,
                    safe_to_retry: Some(!tools_admitted),
                    duration_ms: dur,
                    receipt: Some(receipt),
                };
            }

            let unlocked_before: Vec<&str> = self
                .tools
                .unlocked_tools
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .copied()
                .collect();
            let routing_hint = {
                let sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                let tail = sess
                    .messages()
                    .last()
                    .map(|m| m.content.as_str())
                    .unwrap_or("");
                format!("{}\n{}", input.prompt, tail)
            };
            let openai_tools = self
                .tools
                .select_tool_schemas_for(&routing_hint, self.forbid_swarm_delegation);
            let unlocked_after: Vec<&str> = self
                .tools
                .unlocked_tools
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .copied()
                .collect();
            if unlocked_after != unlocked_before {
                self.prefix_tracker
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .declare_repin("tool_schema_unlock");
            }

            let tool_defs_tokens =
                estimate_text_tokens(&Value::Array(openai_tools.clone()).to_string());
            let soul = Some(
                self.tools
                    .skill_catalog
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .active_soul
                    .clone(),
            );
            let active_profile = self.active_profile();
            let spec = crate::domain::prompt_spec::SystemPromptSpec::for_turn_with_profile(
                &cfg,
                authority,
                soul,
                Some(active_profile),
            );
            let pinned_sys = self
                .pinned_system_prompt
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            let (messages_for_provider, composed_summary) = {
                let sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                let mem = self.memory.lock().unwrap_or_else(|e| e.into_inner());
                PromptComposer::compose_with_posture(
                    &cfg,
                    &self.workspace_root,
                    &sess,
                    &mem,
                    tool_defs_tokens,
                    &spec,
                    effective_posture,
                    pinned_sys.as_deref(),
                )
            };
            if pinned_sys.is_none() {
                if let Some(sys_msg) = messages_for_provider
                    .first()
                    .filter(|m| m.role == Role::System)
                {
                    *self
                        .pinned_system_prompt
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = Some(sys_msg.content.clone());
                }
            }
            let sys_str = messages_for_provider
                .first()
                .map(|m| m.content.as_str())
                .unwrap_or("");
            let fp = PrefixFingerprint::from_wire_prefix(sys_str, &openai_tools);
            last_prefix_hash = fp.combined_hash.clone();
            last_prefix_status = self
                .prefix_tracker
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record_prefix(fp)
                .label();
            *self
                .last_prefix_status
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = last_prefix_status.clone();
            last_composed_summary = composed_summary;

            let resp = match self
                .complete_round_with_backoff(
                    &cfg,
                    round,
                    max_rounds,
                    tools_admitted,
                    &mut active_model,
                    &mut is_fallback_model,
                    &mut turn_route,
                    &messages_for_provider,
                    &openai_tools,
                    &input.cancel_token,
                    &input.on_progress,
                    started_ms,
                )
                .await
            {
                Ok(r) => r,
                Err((ProviderError::Cancelled, _)) => {
                    let msg = "Turn cancelled by user.".to_string();
                    {
                        let mut sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                        Self::heal_dangling_tools(&mut sess, "[Turn cancelled by user]");
                    }
                    self.emit_progress_internal(
                        &input.on_progress,
                        &turn_activity_id,
                        EngineProgressPhase::Cancelled,
                        EngineProgressStatus::Cancelled,
                        &msg,
                        None,
                        started_ms,
                        None,
                    );
                    let dur = now_ms().saturating_sub(started_ms);
                    let (r_prov, r_ep) = turn_route
                        .as_ref()
                        .map(|(_, p, e)| (p.as_str(), e.as_str()))
                        .unwrap_or((&cfg.provider, &cfg.api_endpoint));
                    let receipt = self.build_and_persist_turn_receipt(
                        turn_num,
                        frame_index,
                        r_prov,
                        r_ep,
                        &active_model,
                        effective_posture,
                        EngineTickOutcome::Cancelled,
                        dur,
                        accumulated_usage,
                        last_prefix_hash,
                        last_prefix_status,
                        snap_before,
                        &all_tool_results,
                        None,
                    );
                    return EngineTickResult {
                        frame_index,
                        outcome: EngineTickOutcome::Cancelled,
                        active_model,
                        is_fallback_model,
                        is_slash_command: false,
                        composed_prompt: last_composed_summary,
                        response: msg,
                        tool_results: all_tool_results,
                        safe_to_retry: Some(!tools_admitted),
                        duration_ms: dur,
                        receipt: Some(receipt),
                    };
                }
                Err((err, envelope)) => {
                    let is_unsent_auth = envelope.as_ref().and_then(|e| e.code.as_deref())
                        == Some(CREDENTIAL_REJECTED_UNSENT_CODE);
                    if is_unsent_auth && !tools_admitted {
                        let mut sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                        sess.truncate_active_messages(1);
                    } else {
                        let mut sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                        Self::heal_dangling_tools(&mut sess, "[Turn halted due to provider error]");
                    }
                    let err_str = if let Some(env) = envelope {
                        let code_tag = env
                            .code
                            .as_deref()
                            .map(|c| format!("[{c}] "))
                            .unwrap_or_default();
                        format!("{code_tag}{} — {}", env.message, env.remediation)
                    } else {
                        err.to_string()
                    };
                    self.emit_progress_internal(
                        &input.on_progress,
                        &turn_activity_id,
                        EngineProgressPhase::Failed,
                        EngineProgressStatus::Failed,
                        &err_str,
                        None,
                        started_ms,
                        None,
                    );
                    let dur = now_ms().saturating_sub(started_ms);
                    let (r_prov, r_ep) = turn_route
                        .as_ref()
                        .map(|(_, p, e)| (p.as_str(), e.as_str()))
                        .unwrap_or((&cfg.provider, &cfg.api_endpoint));
                    let receipt = self.build_and_persist_turn_receipt(
                        turn_num,
                        frame_index,
                        r_prov,
                        r_ep,
                        &active_model,
                        effective_posture,
                        EngineTickOutcome::Failed,
                        dur,
                        accumulated_usage,
                        last_prefix_hash,
                        last_prefix_status,
                        snap_before,
                        &all_tool_results,
                        None,
                    );
                    return EngineTickResult {
                        frame_index,
                        outcome: EngineTickOutcome::Failed,
                        active_model,
                        is_fallback_model,
                        is_slash_command: false,
                        composed_prompt: last_composed_summary,
                        response: err_str,
                        tool_results: all_tool_results,
                        safe_to_retry: Some(!tools_admitted),
                        duration_ms: dur,
                        receipt: Some(receipt),
                    };
                }
            };

            let req_for_usage = ProviderRequest {
                model: active_model.clone(),
                messages: messages_for_provider,
                tools: openai_tools,
                max_tokens: cfg.max_output_tokens,
                temperature: cfg.temperature,
                reasoning_effort: cfg.reasoning_effort.clone(),
            };
            Self::accumulate_usage(&mut accumulated_usage, &req_for_usage, &resp);

            if resp.tool_calls.is_empty() {
                let candidate_reply = resp.content.unwrap_or_default();
                let active_profile = self.active_profile();
                let is_scripted_without_custom_gate =
                    !self.has_custom_ci_gate && self.provider_arc().provider_name() == "scripted";
                if active_profile.ci_gate.enabled
                    && active_profile.ci_gate.trigger_on_dev_tasks
                    && dev_mutations_performed
                    && !ci_gate_checked_after_mutation
                    && !is_scripted_without_custom_gate
                {
                    let gate_id = format!("ci-gate-r{round}");
                    self.emit_progress_internal(
                        &input.on_progress,
                        &gate_id,
                        EngineProgressPhase::Verifying,
                        EngineProgressStatus::Started,
                        format!(
                            "Running HEAV3NS-JOY CI gate (profile={}, theme={})",
                            active_profile.id, active_profile.ci_gate.theme
                        ),
                        None,
                        started_ms,
                        None,
                    );
                    let gate_receipt = self
                        .ci_gate_runner
                        .run_ci_gate(&self.workspace_root, &active_profile.ci_gate);
                    let gate_passed = gate_receipt.passed;
                    let gate_status = if gate_passed {
                        EngineProgressStatus::Completed
                    } else {
                        EngineProgressStatus::Failed
                    };
                    self.emit_progress_internal(
                        &input.on_progress,
                        &gate_id,
                        EngineProgressPhase::Verifying,
                        gate_status,
                        format!(
                            "HEAV3NS-JOY CI gate {}: {} blocking, {} advisory ({}ms)",
                            if gate_passed { "PASSED" } else { "FAILED" },
                            gate_receipt.blocking_count,
                            gate_receipt.advisory_count,
                            gate_receipt.duration_ms
                        ),
                        Some(gate_receipt.summary.clone()),
                        started_ms,
                        None,
                    );
                    ci_gate_checked_after_mutation = true;
                    last_ci_receipt = Some(gate_receipt.clone());
                    ci_gate_failed_blocking = !gate_passed && active_profile.ci_gate.fail_closed;
                    if ci_gate_failed_blocking && round + 1 < max_rounds {
                        let mut sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                        sess.add_message(SessionMessage::new(Role::Assistant, candidate_reply));
                        sess.add_message(SessionMessage::new(
                            Role::User,
                            format!(
                                "[HEAV3NS-JOY CI GATE FAILED — exit={}, blocking={}]\n{}\n\nFix all blocking Joyzoning/Spider violations above before concluding.",
                                gate_receipt.exit_code,
                                gate_receipt.blocking_count,
                                gate_receipt.summary
                            ),
                        ));
                        continue;
                    }
                    if ci_gate_failed_blocking {
                        final_response = format!(
                            "{candidate_reply}\n\n[HEAV3NS-JOY CI GATE FAILED — fail_closed=true, blocking={}]\n{}",
                            gate_receipt.blocking_count,
                            gate_receipt.summary
                        );
                        completed_with_assistant_reply = true;
                        let mut sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                        sess.add_message(SessionMessage::new(Role::Assistant, final_response.clone()));
                        break;
                    }
                }

                final_response = candidate_reply;
                completed_with_assistant_reply = true;
                let mut sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                sess.add_message(SessionMessage::new(Role::Assistant, final_response.clone()));
                if sess.messages().len() > cfg.max_turns {
                    sess.compact_session(
                        &self.compactor,
                        &CompactionPolicy {
                            max_messages: Some(cfg.max_turns),
                            preserve_recent_turns: 4,
                            ..Default::default()
                        },
                    );
                }
                break;
            }

            tools_admitted = true;
            {
                let mut assistant_msg =
                    SessionMessage::new(Role::Assistant, resp.content.clone().unwrap_or_default());
                assistant_msg.tool_calls = Some(resp.tool_calls.clone());
                self.session
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .add_message(assistant_msg);
            }

            let scheduled_calls: Vec<ScheduledToolCall> = resp
                .tool_calls
                .iter()
                .map(|tc| {
                    let (parsed_map, _) = self
                        .tools
                        .parser
                        .parse_raw_arguments(&tc.function.arguments);
                    ScheduledToolCall {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        args: Value::Object(parsed_map),
                    }
                })
                .collect();

            let tool_desc: Vec<String> = scheduled_calls
                .iter()
                .map(|c| {
                    let target = c
                        .args
                        .get("path")
                        .or_else(|| c.args.get("CommandLine"))
                        .or_else(|| c.args.get("command"))
                        .or_else(|| c.args.get("query"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if target.is_empty() {
                        c.name.clone()
                    } else {
                        format!("{}: {}", c.name, target)
                    }
                })
                .collect();

            let batch_id = format!("tools-r{round}");
            self.emit_progress_internal(
                &input.on_progress,
                &batch_id,
                EngineProgressPhase::Tool,
                EngineProgressStatus::Started,
                tool_desc.join("; "),
                None,
                started_ms,
                Some(EngineProgressMetadata {
                    source: Some("scheduler".to_string()),
                    scope: Some("step".to_string()),
                    attempt: Some(1),
                    item_type: Some("tool".to_string()),
                    completed_steps: Some(0),
                    total_steps: Some(scheduled_calls.len()),
                    ..Default::default()
                }),
            );

            let unlocked_before_batch: Vec<&str> = self
                .tools
                .unlocked_tools
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .copied()
                .collect();
            let records = self
                .execute_turn_tool_batch(&scheduled_calls, authority, input.cancel_token.as_ref())
                .await;
            if self
                .tools
                .unlocked_tools
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .copied()
                .collect::<Vec<_>>()
                != unlocked_before_batch
            {
                self.prefix_tracker
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .declare_repin("tool_schema_unlock");
            }

            {
                let mut sess = self.session.lock().unwrap_or_else(|e| e.into_inner());
                let mut gov = self
                    .tools
                    .governor
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                for (tc, mut rec) in resp.tool_calls.iter().zip(records.into_iter()) {
                    let is_skill_call =
                        rec.name == "use_skill" || canonical_tool_name(&rec.name) == "use_skill";
                    let (out_text, spill_id) = if is_skill_call {
                        let raw = match &rec.output {
                            Value::String(s) => s.clone(),
                            other => serde_json::to_string_pretty(other)
                                .unwrap_or_else(|_| other.to_string()),
                        };
                        (DeterministicSecretRedactor::redact(&raw), None)
                    } else {
                        let governed = gov.govern(&rec.output);
                        if governed.was_truncated {
                            rec.output_limited = Some(true);
                        }
                        (governed.output_text, governed.spill_id)
                    };

                    let wire_payload = if rec.success {
                        json!({ "success": true, "output": out_text, "spillId": spill_id })
                    } else {
                        json!({
                            "success": false,
                            "error": rec.error.clone().unwrap_or_else(|| "Tool execution failed".to_string()),
                            "output": out_text,
                            "spillId": spill_id,
                        })
                    };

                    sess.add_message(SessionMessage::tool_result(
                        tc.id.clone(),
                        rec.name.clone(),
                        wire_payload.to_string(),
                    ));
                    let canon_name = canonical_tool_name(&rec.name);
                    let cmd_mutated = canon_name == "run_command"
                        && rec
                            .args
                            .as_ref()
                            .and_then(|a| a.get("command").or_else(|| a.get("CommandLine")))
                            .and_then(Value::as_str)
                            .is_some_and(|cmd| {
                                matches!(
                                    inspect_command_security(cmd),
                                    CommandSecurityVerdict::WorkspaceMutating { .. }
                                        | CommandSecurityVerdict::RequireApproval { .. }
                                )
                            });
                    if rec.success && (AgentProfile::is_dev_mutation_tool(canon_name) || cmd_mutated) {
                        dev_mutations_performed = true;
                        ci_gate_checked_after_mutation = false;
                        ci_gate_failed_blocking = false;
                    }
                    all_tool_results.push(rec);
                }
            }

            let batch_records: Vec<&ToolExecutionRecord> = all_tool_results
                .iter()
                .rev()
                .take(resp.tool_calls.len())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();

            let mut batch_files: Vec<String> = Vec::new();
            let mut batch_diffs: Vec<String> = Vec::new();
            let mut batch_added: usize = 0;
            let mut batch_removed: usize = 0;
            let mut batch_diff_truncated = false;
            for r in &batch_records {
                if let Some(rel) = r.output.get("relativePath").and_then(Value::as_str) {
                    if !rel.is_empty() && !batch_files.iter().any(|f| f == rel) {
                        batch_files.push(rel.to_string());
                    }
                }
                if let Some(diff_s) = r.output.get("diff").and_then(Value::as_str) {
                    if !diff_s.trim().is_empty() {
                        batch_diffs.push(diff_s.to_string());
                    }
                }
                if let Some(a) = r.output.get("linesAdded").and_then(Value::as_u64) {
                    batch_added = batch_added.saturating_add(a as usize);
                }
                if let Some(rem) = r.output.get("linesRemoved").and_then(Value::as_u64) {
                    batch_removed = batch_removed.saturating_add(rem as usize);
                }
                if r.output.get("diffTruncated").and_then(Value::as_bool) == Some(true) {
                    batch_diff_truncated = true;
                }
            }

            let combined_diff = if batch_diffs.is_empty() {
                None
            } else {
                let joined = batch_diffs.join("\n");
                if joined.len() > 3_500 {
                    batch_diff_truncated = true;
                    Some(format!(
                        "{}\n...[diff truncated]",
                        crate::plumbing::text_tokens::safe_utf8_prefix(&joined, 3_500)
                    ))
                } else {
                    Some(joined)
                }
            };

            let completed_desc: Vec<String> = batch_records
                .iter()
                .map(|r| {
                    if let (Some(rel), Some(a), Some(rem)) = (
                        r.output.get("relativePath").and_then(Value::as_str),
                        r.output.get("linesAdded").and_then(Value::as_u64),
                        r.output.get("linesRemoved").and_then(Value::as_u64),
                    ) {
                        format!("{} {} (+{}/-{}, {}ms)", r.name, rel, a, rem, r.duration_ms)
                    } else {
                        format!(
                            "{} ({}, {}ms)",
                            r.name,
                            if r.success { "ok" } else { "err" },
                            r.duration_ms
                        )
                    }
                })
                .collect();

            self.emit_progress_internal(
                &input.on_progress,
                &batch_id,
                EngineProgressPhase::Tool,
                EngineProgressStatus::Completed,
                completed_desc.join("; "),
                combined_diff.clone(),
                started_ms,
                Some(EngineProgressMetadata {
                    source: Some("scheduler".to_string()),
                    scope: Some("step".to_string()),
                    attempt: Some(1),
                    item_type: Some("tool".to_string()),
                    completed_steps: Some(resp.tool_calls.len()),
                    total_steps: Some(resp.tool_calls.len()),
                    files: if batch_files.is_empty() { None } else { Some(batch_files) },
                    preview_kind: combined_diff.as_ref().map(|_| "diff".to_string()),
                    preview_text: combined_diff,
                    preview_limited: if batch_diff_truncated { Some(true) } else { None },
                    lines_added: if batch_added > 0 || batch_removed > 0 { Some(batch_added) } else { None },
                    lines_removed: if batch_added > 0 || batch_removed > 0 { Some(batch_removed) } else { None },
                }),
            );
        }

        let cancelled = !completed_with_assistant_reply
            && input
                .cancel_token
                .as_ref()
                .is_some_and(|t| t.is_cancelled());
        if cancelled {
            final_response = "Turn cancelled by user.".to_string();
        } else if final_response.trim().is_empty() {
            final_response = if !all_tool_results.is_empty() {
                let success_count = all_tool_results.iter().filter(|r| r.success).count();
                let tool_names: Vec<String> =
                    all_tool_results.iter().map(|r| r.name.clone()).collect();
                format!(
                    "Completed {} tool action(s) successfully: {}.",
                    success_count,
                    tool_names.join(", ")
                )
            } else {
                "Turn completed with no text response from model.".to_string()
            };
        }
        if !completed_with_assistant_reply {
            Self::heal_dangling_tools(
                &mut self.session.lock().unwrap_or_else(|e| e.into_inner()),
                if cancelled {
                    "[Turn cancelled by user]"
                } else {
                    &final_response
                },
            );
        }
        let (outcome, phase, status, note) = if cancelled {
            (
                EngineTickOutcome::Cancelled,
                EngineProgressPhase::Cancelled,
                EngineProgressStatus::Cancelled,
                "Turn cancelled by user.",
            )
        } else if ci_gate_failed_blocking {
            (
                EngineTickOutcome::Failed,
                EngineProgressPhase::Failed,
                EngineProgressStatus::Failed,
                "HEAV3NS-JOY CI gate failed (fail_closed=true)",
            )
        } else {
            (
                EngineTickOutcome::Completed,
                EngineProgressPhase::Completed,
                EngineProgressStatus::Completed,
                "Turn completed",
            )
        };
        self.emit_progress_internal(
            &input.on_progress,
            &turn_activity_id,
            phase,
            status,
            note,
            None,
            started_ms,
            None,
        );
        let dur = now_ms().saturating_sub(started_ms);
        let (r_prov, r_ep) = turn_route
            .as_ref()
            .map(|(_, p, e)| (p.as_str(), e.as_str()))
            .unwrap_or((&cfg.provider, &cfg.api_endpoint));
        let receipt = self.build_and_persist_turn_receipt(
            turn_num,
            frame_index,
            r_prov,
            r_ep,
            &active_model,
            effective_posture,
            outcome,
            dur,
            accumulated_usage,
            last_prefix_hash,
            last_prefix_status,
            snap_before,
            &all_tool_results,
            last_ci_receipt,
        );

        EngineTickResult {
            frame_index,
            outcome,
            active_model,
            is_fallback_model,
            is_slash_command: false,
            composed_prompt: last_composed_summary,
            response: final_response,
            tool_results: all_tool_results,
            safe_to_retry: Some(!tools_admitted),
            duration_ms: dur,
            receipt: Some(receipt),
        }
    }
}
