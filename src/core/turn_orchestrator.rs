// [LAYER: CORE]
use crate::core::engine::AgentEngine;
use crate::core::session_store::SessionStore;
use crate::domain::interfaces::{
    ILlmProvider, IModelPricingRegistry, IWorkspaceSnapshotCas, ProgressCallback,
};
use crate::domain::pricing_and_taxonomy::{
    compute_retry_delay_ms, resolve_compatible_fallback_model_with_catalog, BillingMetering, ErrorCategory,
    ErrorEnvelope, ProviderTokenUsage,
};
use crate::domain::profiles::CiGateReceipt;
use crate::domain::turn_receipt::{CommandRunReceipt, TurnReceipt};
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::posture_types::RepoConstitution;
use crate::domain::types::progress::{
    EngineProgressMetadata, EngineProgressPhase, EngineProgressStatus, ToolExecutionRecord,
};
use crate::domain::types::provider_registry::{
    lookup_provider_descriptor, resolve_provider_route, ProviderChain,
};
use crate::domain::types::runtime::{
    AgentConfig, CancellationToken, EngineTickOutcome, ExecutionPosture,
};
use crate::domain::types::tick_result::{ProviderError, ProviderRequest, ProviderResponse};
use crate::infrastructure::http_provider::HttpProvider;
use crate::infrastructure::pricing_updater::global_pricing_registry;
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::text_tokens::{canonical_tool_name, estimate_text_tokens};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Loads `.heav3ns/constitution.json` or `.codewhale/constitution.json` (`RepoLaw`), failing closed if malformed.
pub fn load_workspace_constitution(workspace_root: &Path) -> Option<RepoConstitution> {
    let candidates = [
        workspace_root.join(".heav3ns").join("constitution.json"),
        workspace_root.join(".codewhale").join("constitution.json"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return match fs::read_to_string(&candidate) {
                Ok(raw) => Some(RepoConstitution::parse_strict(&raw)),
                Err(e) => Some(RepoConstitution {
                    version: 0,
                    rules: Vec::new(),
                    invalid_error: Some(format!("Failed reading constitution ({e}); fail-closed")),
                }),
            };
        }
    }
    None
}

/// Ensures `SessionStore` never ends on dangling `Role::Tool` results when `max_rounds` is exhausted or a turn aborts mid-tool-loop.
pub fn heal_dangling_tool_tail(session: &mut SessionStore, summary_note: &str) {
    let ends_with_tool = session
        .messages()
        .last()
        .map(|m| m.role == Role::Tool)
        .unwrap_or(false);
    if ends_with_tool {
        session.add_message(SessionMessage::new(Role::Assistant, summary_note));
    }
}

/// Accumulates exact or estimated token usage across multi-round tool calls within a single turn.
pub fn accumulate_round_usage(
    accum: &mut ProviderTokenUsage,
    req: &ProviderRequest,
    resp: &ProviderResponse,
) {
    if let Some(ref u) = resp.usage {
        let first_round = accum.prompt_tokens == 0 && accum.completion_tokens == 0;
        accum.prompt_tokens = accum.prompt_tokens.saturating_add(u.prompt_tokens);
        accum.completion_tokens = accum.completion_tokens.saturating_add(u.completion_tokens);
        accum.cached_prompt_tokens = accum
            .cached_prompt_tokens
            .saturating_add(u.cached_prompt_tokens);
        accum.is_exact = if first_round {
            u.is_exact
        } else {
            accum.is_exact && u.is_exact
        };
    } else {
        let req_str = serde_json::to_string(&req.messages).unwrap_or_default();
        let tools_str = serde_json::to_string(&req.tools).unwrap_or_default();
        let out_str = resp.content.as_deref().unwrap_or("");
        let tc_tokens: usize = resp
            .tool_calls
            .iter()
            .map(|tc| {
                8 + estimate_text_tokens(&tc.function.name)
                    + estimate_text_tokens(&tc.function.arguments)
            })
            .sum();
        accum.prompt_tokens = accum
            .prompt_tokens
            .saturating_add(estimate_text_tokens(&req_str) + estimate_text_tokens(&tools_str));
        accum.completion_tokens = accum
            .completion_tokens
            .saturating_add(estimate_text_tokens(out_str) + tc_tokens);
        accum.is_exact = false;
    }
}

impl AgentEngine {
    pub(crate) fn load_constitution(workspace_root: &Path) -> Option<RepoConstitution> {
        load_workspace_constitution(workspace_root)
    }

    pub(crate) fn heal_dangling_tools(session: &mut SessionStore, summary_note: &str) {
        heal_dangling_tool_tail(session, summary_note);
    }

    pub(crate) fn accumulate_usage(
        accum: &mut ProviderTokenUsage,
        req: &ProviderRequest,
        resp: &ProviderResponse,
    ) {
        accumulate_round_usage(accum, req, resp);
    }

    /// Executes a single provider round with jittered exponential backoff across ALL rounds (`0..max_rounds`),
    /// separating `402`/spent-balance (`Budget`) from `429` (`RateLimit`) and preserving sticky multi-round `ProviderChain` failover.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn complete_round_with_backoff(
        &self,
        cfg: &AgentConfig,
        round: usize,
        max_rounds: usize,
        tools_admitted: bool,
        active_model: &mut String,
        is_fallback_model: &mut bool,
        turn_route: &mut Option<(Arc<dyn ILlmProvider>, String, String)>,
        messages_for_provider: &[SessionMessage],
        openai_tools: &[Value],
        cancel_token: &Option<CancellationToken>,
        on_progress: &Option<ProgressCallback>,
        started_ms: u64,
    ) -> Result<ProviderResponse, (ProviderError, Option<ErrorEnvelope>)> {
        let is_scripted = cfg.provider.eq_ignore_ascii_case("scripted")
            || self
                .provider_arc()
                .provider_name()
                .eq_ignore_ascii_case("scripted");
        let max_attempts: u32 = if is_scripted {
            if tools_admitted {
                1
            } else {
                2
            }
        } else {
            3
        };
        let mut last_err: Option<(ProviderError, Option<ErrorEnvelope>)> = None;

        for attempt in 1..=max_attempts {
            let conn_id = format!("provider-r{round}-a{attempt}");
            let active_prov_id = turn_route
                .as_ref()
                .map(|(_, p, _)| p.as_str())
                .unwrap_or(&cfg.provider);
            if attempt >= 2 && !*is_fallback_model {
                let catalog = global_pricing_registry().snapshot_catalog();
                if let Some(fb) = resolve_compatible_fallback_model_with_catalog(
                    active_prov_id,
                    active_model,
                    if turn_route.is_none() {
                        cfg.fallback_model.as_deref()
                    } else {
                        None
                    },
                    &catalog,
                ) {
                    *active_model = fb;
                    *is_fallback_model = true;
                }
            }

            self.emit_progress_internal(
                on_progress,
                &conn_id,
                EngineProgressPhase::Connecting,
                EngineProgressStatus::Started,
                format!(
                    "Requesting completion from {} (attempt {attempt})",
                    *active_model
                ),
                None,
                started_ms,
                Some(EngineProgressMetadata {
                    source: Some("provider".to_string()),
                    scope: Some("turn".to_string()),
                    attempt: Some(attempt),
                    item_type: None,
                    completed_steps: Some(round),
                    total_steps: Some(max_rounds),
                }),
            );

            let req = ProviderRequest {
                model: active_model.clone(),
                messages: messages_for_provider.to_vec(),
                tools: openai_tools.to_vec(),
                max_tokens: cfg.max_output_tokens,
                temperature: cfg.temperature,
                reasoning_effort: cfg.reasoning_effort.clone(),
            };

            let provider = turn_route
                .as_ref()
                .map(|(p, _, _)| Arc::clone(p))
                .unwrap_or_else(|| self.provider_arc());
            match provider
                .complete_with_progress(req, cancel_token.clone(), on_progress.clone())
                .await
            {
                Ok(resp) => {
                    self.emit_progress_internal(
                        on_progress,
                        &conn_id,
                        EngineProgressPhase::Thinking,
                        EngineProgressStatus::InProgress,
                        "Received model response",
                        None,
                        started_ms,
                        None,
                    );
                    return Ok(resp);
                }
                Err(err) => {
                    if err.is_cancelled()
                        || cancel_token
                            .as_ref()
                            .map(|t| t.is_cancelled())
                            .unwrap_or(false)
                    {
                        return Err((ProviderError::Cancelled, None));
                    }

                    let (envelope, retry_after_ms) = match &err {
                        ProviderError::HttpStatus {
                            status,
                            body,
                            retry_after_ms,
                            ..
                        } => (
                            Some(ErrorEnvelope::classify_http(*status, body, tools_admitted)),
                            *retry_after_ms,
                        ),
                        ProviderError::Timeout => (
                            Some(ErrorEnvelope {
                                category: ErrorCategory::Timeout,
                                recoverable: true,
                                code: Some("provider_timeout".to_string()),
                                message: err.to_string(),
                                remediation: "Retrying with bounded backoff.".to_string(),
                            }),
                            None,
                        ),
                        ProviderError::Transport(_) => (
                            Some(ErrorEnvelope {
                                category: ErrorCategory::Network,
                                recoverable: true,
                                code: Some("transport_error".to_string()),
                                message: err.to_string(),
                                remediation: "Verify network connectivity or local Ollama server."
                                    .to_string(),
                            }),
                            None,
                        ),
                        ProviderError::Cancelled => (None, None),
                    };

                    let recoverable = envelope
                        .as_ref()
                        .map(|e| e.recoverable && e.category != ErrorCategory::Budget)
                        .unwrap_or_else(|| err.is_retryable());
                    // For scripted tests, preserve FT-04 exact behavior (`!tools_admitted`); for real HTTP providers,
                    // transient 429/503/timeout mid-turn IS safely retryable because the failed HTTP call produced no new tool calls!
                    let can_retry =
                        attempt < max_attempts && recoverable && (!is_scripted || !tools_admitted);

                    self.emit_progress_internal(
                        on_progress,
                        &conn_id,
                        EngineProgressPhase::Connecting,
                        EngineProgressStatus::Failed,
                        format!("Provider attempt {attempt} failed: {err}"),
                        envelope.as_ref().map(|e| e.remediation.clone()),
                        started_ms,
                        Some(EngineProgressMetadata {
                            source: Some("provider".to_string()),
                            scope: Some("turn".to_string()),
                            attempt: Some(attempt),
                            item_type: None,
                            completed_steps: Some(round),
                            total_steps: Some(max_rounds),
                        }),
                    );

                    last_err = Some((err, envelope.clone()));
                    if !can_retry {
                        break;
                    }

                    let cat = envelope
                        .as_ref()
                        .map(|e| e.category)
                        .unwrap_or(ErrorCategory::ServerTransient);
                    let delay_ms =
                        compute_retry_delay_ms(attempt, cat, retry_after_ms, is_scripted);
                    if let Some(tok) = cancel_token.as_ref() {
                        let start_sleep = tokio::time::Instant::now();
                        let target_dur = Duration::from_millis(delay_ms);
                        while start_sleep.elapsed() < target_dur {
                            if tok.is_cancelled() {
                                return Err((ProviderError::Cancelled, None));
                            }
                            let rem = target_dur
                                .saturating_sub(start_sleep.elapsed())
                                .min(Duration::from_millis(25));
                            tokio::time::sleep(rem).await;
                        }
                    } else {
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        if !is_scripted
            && !tools_admitted
            && turn_route.is_none()
            && !cfg.fallback_providers.is_empty()
        {
            let can_failover = last_err
                .as_ref()
                .map(|(err, env)| {
                    env.as_ref()
                        .map(|e| {
                            e.recoverable
                                || matches!(
                                    e.category,
                                    ErrorCategory::Budget
                                        | ErrorCategory::Authentication
                                        | ErrorCategory::Authorization
                                )
                        })
                        .unwrap_or_else(|| err.is_retryable())
                })
                .unwrap_or(false);
            if can_failover {
                let mut chain = ProviderChain::new(&cfg.provider, &cfg.fallback_providers);
                while let Some(fb_entry) = chain.advance() {
                    if cancel_token.as_ref().is_some_and(|t| t.is_cancelled()) {
                        return Err((ProviderError::Cancelled, None));
                    }
                    let (fb_prov_id, fb_model_override) = fb_entry
                        .split_once(':')
                        .map(|(p, m)| (p.trim(), Some(m.trim()).filter(|s| !s.is_empty())))
                        .unwrap_or((fb_entry.trim(), None));
                    let route = resolve_provider_route(
                        fb_prov_id,
                        fb_model_override,
                        None,
                        None,
                        None,
                        Some(&cfg.provider),
                        cfg.api_key.as_deref(),
                        &cfg.provider_keys,
                        &cfg.provider_endpoints,
                        |env_key| std::env::var(env_key).ok(),
                    );
                    let requires_key = lookup_provider_descriptor(&route.provider_id)
                        .is_some_and(|d| d.requires_api_key);
                    if requires_key && route.api_key.is_none() {
                        continue;
                    }
                    let fb_http = HttpProvider::new(
                        route.provider_id.clone(),
                        route.api_endpoint.clone(),
                        route.api_key,
                        cfg.provider_timeout_ms,
                    )
                    .with_wire_dialect(route.wire_dialect);
                    let fb_req = ProviderRequest {
                        model: route.model_name.clone(),
                        messages: messages_for_provider.to_vec(),
                        tools: openai_tools.to_vec(),
                        max_tokens: cfg.max_output_tokens,
                        temperature: cfg.temperature,
                        reasoning_effort: cfg.reasoning_effort.clone(),
                    };
                    match ILlmProvider::complete(&fb_http, fb_req, cancel_token.clone()).await {
                        Ok(resp) => {
                            *active_model = route.model_name;
                            *is_fallback_model = true;
                            *turn_route =
                                Some((Arc::new(fb_http), route.provider_id, route.api_endpoint));
                            return Ok(resp);
                        }
                        Err(ProviderError::Cancelled) => {
                            return Err((ProviderError::Cancelled, None));
                        }
                        Err(_) => {
                            if cancel_token.as_ref().is_some_and(|t| t.is_cancelled()) {
                                return Err((ProviderError::Cancelled, None));
                            }
                        }
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            (
                ProviderError::Transport("Unknown provider failure".to_string()),
                None,
            )
        }))
    }

    /// Builds and persists a prose-free `TurnReceipt` (`heav3ns.receipt/v1`) from runtime-verified CAS and execution facts.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_and_persist_turn_receipt(
        &self,
        turn_num: u64,
        frame_index: u64,
        provider_name: &str,
        api_endpoint: &str,
        active_model: &str,
        posture: ExecutionPosture,
        outcome: EngineTickOutcome,
        duration_ms: u64,
        usage: ProviderTokenUsage,
        prefix_hash: String,
        prefix_status: String,
        snap_before: Option<String>,
        tool_results: &[ToolExecutionRecord],
        ci_gate: Option<CiGateReceipt>,
    ) -> TurnReceipt {
        let snap_after = self
            .cas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .capture_snapshot("post-turn", turn_num)
            .ok();

        let file_changes = match (&snap_before, &snap_after) {
            (Some(b), Some(a)) => {
                let b_hash = b.split('_').nth(2).unwrap_or("");
                let a_hash = a.split('_').nth(2).unwrap_or("");
                if !b_hash.is_empty() && b_hash == a_hash {
                    Vec::new()
                } else {
                    self.cas
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .diff_between(b, Some(a.as_str()))
                        .map(|(changes, _)| changes)
                        .unwrap_or_default()
                }
            }
            _ => Vec::new(),
        };

        let mut commands = Vec::new();
        for rec in tool_results {
            if canonical_tool_name(&rec.name) == "run_command" {
                let cmd = rec
                    .args
                    .as_ref()
                    .and_then(|a| a.get("command").or_else(|| a.get("CommandLine")))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let sandboxed = rec
                    .output
                    .get("sandboxed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let outcome_str = if rec.denied == Some(true) {
                    "denied".to_string()
                } else if rec.success {
                    "exit_0".to_string()
                } else {
                    let code = rec
                        .output
                        .get("exitCode")
                        .and_then(Value::as_i64)
                        .unwrap_or(-1);
                    format!("exit_{code}")
                };
                commands.push(CommandRunReceipt {
                    command: cmd,
                    outcome: outcome_str,
                    sandboxed,
                    duration_ms: rec.duration_ms,
                });
            }
        }

        let approvals = self
            .tools()
            .turn_approvals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let catalog = global_pricing_registry().snapshot_catalog();
        let billing = BillingMetering::evaluate_with_catalog(
            provider_name,
            active_model,
            api_endpoint,
            &usage,
            &catalog,
        );
        let outcome_slug = match outcome {
            EngineTickOutcome::Completed => "completed",
            EngineTickOutcome::Cancelled => "cancelled",
            EngineTickOutcome::Deferred => "deferred",
            EngineTickOutcome::Failed => "failed",
        };

        let mut claim_ceiling = Vec::new();
        if file_changes.is_empty() {
            claim_ceiling.push("no_workspace_files_modified".to_string());
        } else {
            claim_ceiling.push(format!(
                "{}_files_modified_verified_by_cas",
                file_changes.len()
            ));
        }
        if commands.is_empty() {
            claim_ceiling.push("no_shell_commands_executed".to_string());
        } else {
            claim_ceiling.push(format!("{}_commands_executed", commands.len()));
        }

        let receipt = TurnReceipt {
            schema: "heav3ns.receipt/v1".to_string(),
            turn_id: format!("turn-{turn_num}"),
            frame_index,
            timestamp_ms: now_ms(),
            provider: provider_name.to_string(),
            model: active_model.to_string(),
            posture: posture.slug().to_string(),
            outcome: outcome_slug.to_string(),
            duration_ms,
            usage,
            billing_display: billing.format_honest(),
            billing_kind: billing.kind_slug().to_string(),
            prefix_hash,
            prefix_status,
            snapshot_before: snap_before,
            snapshot_after: snap_after,
            file_changes,
            commands,
            approvals,
            tools_run: tool_results.len(),
            not_recorded: vec![
                "external_network_side_effects_outside_workspace".to_string(),
                "ignored_build_directories_target_node_modules".to_string(),
                "files_larger_than_2mib".to_string(),
            ],
            claim_ceiling,
            ci_gate,
        };

        let _ = self
            .cas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .append_turn_receipt(&receipt);
        let mut list = self.turn_receipts.lock().unwrap_or_else(|e| e.into_inner());
        if list.len() >= 128 {
            list.remove(0);
        }
        list.push(receipt.clone());
        receipt
    }
}
