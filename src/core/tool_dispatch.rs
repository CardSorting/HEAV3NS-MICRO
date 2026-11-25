// [LAYER: CORE]
use crate::core::tool_governor::ToolExecutionCache;
use crate::domain::types::posture_eval::evaluate_tool_posture;
use crate::domain::types::posture_types::AuthorizationVerdict;
use crate::domain::types::progress::ToolExecutionRecord;
use crate::domain::types::runtime::{
    CancellationToken, ExecutionAuthorityLevel,
    UserApprovalChoice,
};
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::infrastructure::search_tools::resolve_workspace_path;
use crate::plumbing::hash_and_math::lock_unpoison;
use crate::plumbing::text_tokens::canonical_tool_name;
use futures::future::join_all;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;
use crate::core::tool_registry::ToolRegistry;

impl ToolRegistry {
    pub async fn execute_batch(
        &self,
        calls: &[ScheduledToolCall],
        cwd: &Path,
        authority: ExecutionAuthorityLevel,
        cancel_token: Option<&CancellationToken>,
    ) -> Vec<ToolExecutionRecord> {
        let waves = self.partition_waves(calls, cwd);
        let mut results = Vec::with_capacity(calls.len());
        let mut cancelled = false;
        for wave in waves {
            if cancelled || cancel_token.map(|t| t.is_cancelled()).unwrap_or(false) {
                cancelled = true;
                let now = Instant::now();
                for call in &wave {
                    results.push(Self::failed_record(
                        call,
                        call.args.clone(),
                        "Tool execution cancelled by user before dispatch".to_string(),
                        None,
                        now,
                    ));
                }
                continue;
            }
            if wave.len() == 1 {
                results.push(
                    self.execute_single(&wave[0], cwd, authority, cancel_token)
                        .await,
                );
            } else {
                let futs = wave
                    .iter()
                    .map(|call| self.execute_single(call, cwd, authority, cancel_token));
                results.extend(join_all(futs).await);
            }
        }
        results
    }

    pub async fn execute_single(
        &self,
        call: &ScheduledToolCall,
        cwd: &Path,
        authority: ExecutionAuthorityLevel,
        cancel_token: Option<&CancellationToken>,
    ) -> ToolExecutionRecord {
        let started = Instant::now();
        let canonical = canonical_tool_name(&call.name);
        let Some(spec) = self.get_spec(canonical) else {
            let err_msg = format!("Tool '{}' not found in registry", call.name);
            return Self::failed_record(call, call.args.clone(), err_msg, None, started);
        };
        self.unlock_tool_by_name(spec.name);

        let (prepared_args, validation) =
            self.parser.prepare_arguments(&call.args, &spec.parameters);
        let prepared_val = Value::Object(prepared_args.clone());
        if !validation.valid {
            let err_msg = format!(
                "Tool '{}' argument schema validation failed: {}",
                canonical,
                validation.errors.join("; ")
            );
            return ToolExecutionRecord {
                name: call.name.clone(),
                call_id: Some(call.id.clone()),
                args: Some(prepared_val),
                output: json!({ "error": err_msg, "suggestions": validation.suggestions }),
                success: false,
                denied: None,
                output_limited: None,
                error: Some(err_msg),
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }

        // Monotonic 9-layer authorization reducer (`ExecutionPosture` + `RepoLaw` + Credential Guard + Session Allowlist)
        let posture = self.effective_posture_for(authority);
        if canonical != "execute_tools" {
            let eval = {
                let law = lock_unpoison(&self.constitution).clone();
                let allowed = lock_unpoison(&self.session_allowlist).clone();
                evaluate_tool_posture(
                    posture,
                    canonical,
                    &prepared_val,
                    spec.is_mutating,
                    law.as_ref(),
                    &allowed,
                )
            };
            match eval.verdict {
                AuthorizationVerdict::Deny => {
                    return Self::failed_record(
                        call,
                        prepared_val,
                        eval.reason,
                        Some(true),
                        started,
                    );
                }
                AuthorizationVerdict::AskUser => {
                    let gate = lock_unpoison(&self.approval_gate).clone();
                    let choice = if let Some(cb) = gate {
                        cb(canonical, &eval.reason, &eval.session_rule_key)
                    } else {
                        UserApprovalChoice::Deny
                    };
                    match choice {
                        UserApprovalChoice::AllowOnce => {
                            lock_unpoison(&self.turn_approvals)
                                .push(format!("{canonical}: allow_once ({})", eval.layer));
                        }
                        UserApprovalChoice::AllowSession => {
                            let mut allow = lock_unpoison(&self.session_allowlist);
                            if !allow.contains(&eval.session_rule_key) {
                                allow.push(eval.session_rule_key.clone());
                            }
                            lock_unpoison(&self.turn_approvals).push(format!(
                                "{canonical}: allow_session ({})",
                                eval.session_rule_key
                            ));
                        }
                        UserApprovalChoice::Deny => {
                            let msg = format!(
                                "Approval required ({}) — denied: {}",
                                posture.display_label(),
                                eval.reason
                            );
                            return Self::failed_record(
                                call,
                                prepared_val,
                                msg,
                                Some(true),
                                started,
                            );
                        }
                    }
                }
                AuthorizationVerdict::Allow => {}
            }
        }

        // Read-Before-Write & External Staleness Guard (`FileReadTracker`)
        if let Some(p_str) = prepared_args
            .get("path")
            .or_else(|| prepared_args.get("source"))
            .and_then(Value::as_str)
        {
            let resolved = resolve_workspace_path(cwd, p_str);
            if let Err(stale_err) = lock_unpoison(&self.read_tracker)
                .verify_before_mutation(canonical, &resolved, posture)
            {
                return Self::failed_record(call, prepared_val, stale_err, None, started);
            }
        }
        if canonical == "move_file" {
            if let Some(dst_str) = prepared_args.get("target").and_then(Value::as_str) {
                let resolved = resolve_workspace_path(cwd, dst_str);
                if let Err(stale_err) = lock_unpoison(&self.read_tracker)
                    .verify_before_mutation(canonical, &resolved, posture)
                {
                    return Self::failed_record(call, prepared_val, stale_err, None, started);
                }
            }
        }
        if canonical == "apply_unified_diff" {
            if let Some(diff_text) = prepared_args
                .get("diff")
                .or_else(|| prepared_args.get("patch"))
                .and_then(Value::as_str)
            {
                for op in crate::plumbing::fuzzy_patch::DeterministicPatchEngine::parse(diff_text) {
                    let resolved = resolve_workspace_path(cwd, &op.file_path);
                    if let Err(stale_err) = lock_unpoison(&self.read_tracker)
                        .verify_before_mutation(canonical, &resolved, posture)
                    {
                        return Self::failed_record(call, prepared_val, stale_err, None, started);
                    }
                }
            }
        }

        let soft_loop = authority.is_autonomous() || !spec.is_mutating;
        if let Ok(mut lb) = self.loop_breaker.lock() {
            if let Err(loop_err) = lb.record_and_check(canonical, &prepared_args, soft_loop) {
                return Self::failed_record(call, prepared_val, loop_err, None, started);
            }
        }

        let cacheable = !spec.is_mutating && ToolExecutionCache::is_cacheable(canonical);
        let cache_key = if cacheable {
            let key = ToolExecutionCache::compute_key(canonical, &prepared_args, cwd);
            if let Ok(mut c) = self.cache.lock() {
                if let Some(cached_val) = c.lookup(&key) {
                    let disk_stale = canonical == "view_file"
                        && prepared_args
                            .get("path")
                            .and_then(Value::as_str)
                            .is_some_and(|p_str| {
                                let resolved = crate::core::tool_governor::normalize_tracker_key(
                                    &resolve_workspace_path(cwd, p_str),
                                );
                                if !lock_unpoison(&self.read_tracker).is_disk_fresh(&resolved) {
                                    c.invalidate_paths(&[resolved]);
                                    true
                                } else {
                                    false
                                }
                            });
                    if !disk_stale {
                        return ToolExecutionRecord {
                            name: call.name.clone(),
                            call_id: Some(call.id.clone()),
                            args: Some(prepared_val),
                            output: cached_val,
                            success: true,
                            denied: None,
                            output_limited: None,
                            error: None,
                            duration_ms: started.elapsed().as_millis() as u64,
                        };
                    }
                }
            }
            Some(key)
        } else {
            None
        };

        let target_resources =
            self.extract_target_resources_from_map(canonical, &prepared_args, cwd);

        match self
            .dispatch_builtin(
                canonical,
                &prepared_args,
                cwd,
                authority,
                posture,
                cancel_token,
            )
            .await
        {
            Ok((val, denied)) => {
                if spec.is_mutating {
                    if let Ok(mut c) = self.cache.lock() {
                        c.invalidate_paths(&target_resources);
                    }
                    if !denied {
                        let mut rt = lock_unpoison(&self.read_tracker);
                        if canonical == "delete_file" {
                            for p in &target_resources {
                                rt.remove_path(p);
                            }
                        } else if canonical == "move_file" {
                            if let Some(src_str) =
                                prepared_args.get("source").and_then(Value::as_str)
                            {
                                rt.remove_path(&resolve_workspace_path(cwd, src_str));
                            }
                            if let Some(dst_str) =
                                prepared_args.get("target").and_then(Value::as_str)
                            {
                                rt.record_disk_file(&resolve_workspace_path(cwd, dst_str), true);
                            }
                        } else if matches!(
                            canonical,
                            "write_file"
                                | "replace_file_content"
                                | "multi_replace_file_content"
                                | "edit_file_anchored"
                                | "apply_unified_diff"
                        ) {
                            for p in &target_resources {
                                if p.exists() {
                                    rt.record_disk_file(p, true);
                                } else {
                                    rt.remove_path(p);
                                }
                            }
                        }
                    }
                } else {
                    if canonical == "view_file" && !denied {
                        let full = prepared_args.get("startLine").is_none()
                            && prepared_args.get("endLine").is_none()
                            && prepared_args.get("contentOffset").is_none();
                        for p in &target_resources {
                            lock_unpoison(&self.read_tracker).record_disk_file(p, full);
                        }
                    }
                    if let Some(key) = cache_key {
                        if let Ok(mut c) = self.cache.lock() {
                            c.store(key, target_resources, val.clone());
                        }
                    }
                }
                let tool_ok =
                    !denied && val.get("success").and_then(Value::as_bool).unwrap_or(true);
                ToolExecutionRecord {
                    name: call.name.clone(),
                    call_id: Some(call.id.clone()),
                    args: Some(prepared_val),
                    output: val,
                    success: tool_ok,
                    denied: if denied { Some(true) } else { None },
                    output_limited: None,
                    error: if denied {
                        Some("Permission denied for this operation".to_string())
                    } else {
                        None
                    },
                    duration_ms: started.elapsed().as_millis() as u64,
                }
            }
            Err(err_msg) => Self::failed_record(call, prepared_val, err_msg, None, started),
        }
    }

}
