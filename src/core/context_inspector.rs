// [LAYER: CORE]
use crate::core::engine::AgentEngine;
use crate::core::request_queue::{discover_skills_cached, PromptComposer};
use crate::core::session_store::SessionCompactor;
use crate::core::tool_specs::builtin_tool_specs;
use crate::domain::compaction::ContextBudgetInfo;
use crate::domain::prefix_and_manifest::{PrefixFingerprint, RequestManifest};
use crate::domain::pricing_and_taxonomy::{select_auto_routed_model, BillingMetering, ProviderTokenUsage};
use crate::domain::prompt_spec::{ModelPromptFamily, PromptProfile, SystemPromptSpec};
use crate::domain::types::messages::{estimate_messages_tokens, Role, SessionMessage};
use crate::domain::types::provider_registry::{infer_wire_dialect_from_endpoint, lookup_provider_descriptor, resolve_model_aware_wire_and_model, retired_provider_remediation, suggest_provider_slug, ProviderBillingClass, ReasoningEffortLevel, WireDialect, PROVIDER_DESCRIPTORS};
use crate::domain::types::runtime::ExecutionAuthorityLevel;
use crate::infrastructure::vfs_store::WorkspacePromptProbe;
use crate::plumbing::schema_and_sse::redact_endpoint_for_display;
use crate::plumbing::text_tokens::estimate_text_tokens;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Structured telemetry snapshot of system prompt composition, KV-cache prefix, sticky tool routing, posture, and context window headroom.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextInspectionSnapshot {
    pub profile: String,
    pub model_name: String,
    pub model_family: String,
    pub provider: String,
    pub posture: String,
    pub cache_prefix_hash: String,
    pub prefix_pin_status: String,
    pub billing_estimate: String,
    pub static_prefix_tokens: usize,
    pub dynamic_workspace_tokens: usize,
    pub system_total_tokens: usize,
    pub tool_active_count: usize,
    pub tool_total_count: usize,
    pub tool_schema_tokens: usize,
    pub tool_full_schema_tokens: usize,
    pub unlocked_specialist_tools: Vec<String>,
    pub evicted_specialist_tools: Vec<String>,
    pub deferred_specialist_tools: Vec<String>,
    pub memory_entries: usize,
    pub memory_tokens: usize,
    pub history_messages: usize,
    pub rolling_summaries: usize,
    pub history_tokens: usize,
    pub context_window_tokens: usize,
    pub available_input_tokens: usize,
    pub reserved_output_tokens: usize,
    pub safety_margin_tokens: usize,
    pub used_input_tokens: usize,
    pub headroom_tokens: usize,
    pub signed_headroom_tokens: i64,
    pub utilization_pct: f64,
    pub compaction_trigger_tokens: usize,
    pub joyzoning_detected: bool,
    pub manifests: Vec<String>,
    pub skills_discovered: usize,
    pub governance_sources: Vec<String>,
    pub governance_warnings: Vec<String>,
}

impl AgentEngine {
    pub fn inspect_context_snapshot(&self, sample_prompt: &str) -> ContextInspectionSnapshot {
        let cfg = self.config_snapshot();
        let posture = self.current_posture();
        let soul = Some(self.tools().skill_catalog.lock().unwrap().active_soul.clone());
        let spec = SystemPromptSpec::for_turn(&cfg, ExecutionAuthorityLevel::Autonomous, soul);
        let facts = WorkspacePromptProbe::probe_cached(self.workspace_root());
        let skills = discover_skills_cached(self.workspace_root());

        let static_prefix = spec.static_prefix(&cfg);
        let dynamic_suffix = spec.dynamic_suffix(&cfg, self.workspace_root(), &facts, &skills);
        let full_system = self
            .pinned_system_prompt
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| spec.render(&cfg, self.workspace_root(), &facts, &skills));
        let static_prefix_tokens = estimate_text_tokens(&static_prefix);
        let dynamic_workspace_tokens = estimate_text_tokens(&dynamic_suffix);
        let system_total_tokens = estimate_text_tokens(&full_system);

        let probe = if sample_prompt.trim().is_empty() { "inspect workspace" } else { sample_prompt };
        let active_schemas = self.tools().peek_tool_schemas_for(probe, self.forbids_swarm_delegation());
        let full_schemas = self.tools().openai_tool_schemas();
        let tool_schema_tokens = estimate_text_tokens(&Value::Array(active_schemas.clone()).to_string());
        let tool_full_schema_tokens = estimate_text_tokens(&Value::Array(full_schemas.clone()).to_string());

        let unlocked_set: Vec<String> = {
            let guard = self.tools().unlocked_lru.lock().unwrap();
            guard.iter().map(|s| (*s).to_string()).collect()
        };
        let evicted_set: Vec<String> = {
            let guard = self.tools().evicted_tools.lock().unwrap();
            guard.iter().map(|s| (*s).to_string()).collect()
        };
        let mut deferred = Vec::new();
        for s in self.tools().list_specs() {
            if !s.is_core && !unlocked_set.iter().any(|u| u == s.name) {
                deferred.push(s.name.to_string());
            }
        }

        let (memory_entries, memory_tokens) = {
            let mem = self.memory().lock().unwrap();
            let count = mem.list_memories().len();
            let toks = mem.format_memory_envelope().map(|e| estimate_text_tokens(&e)).unwrap_or(0);
            (count, toks)
        };

        let (history_messages, rolling_summaries, history_tokens) = {
            let sess = self.session().lock().unwrap();
            let non_sys: Vec<_> = sess.messages().iter().filter(|m| m.role != Role::System).cloned().collect();
            let sums = non_sys.iter().filter(|m| SessionCompactor::is_checkpoint(m)).count();
            let toks = estimate_messages_tokens(&non_sys);
            (non_sys.len(), sums, toks)
        };

        let budget = ContextBudgetInfo::calculate(cfg.context_window_tokens, cfg.max_output_tokens);
        let used_input_tokens = system_total_tokens + tool_schema_tokens + memory_tokens + history_tokens;
        let headroom_tokens = budget.available_input_tokens.saturating_sub(used_input_tokens);
        let signed_headroom_tokens = (budget.available_input_tokens as i64) - (used_input_tokens as i64);
        let utilization_pct = if budget.available_input_tokens == 0 {
            0.0
        } else {
            ((used_input_tokens as f64 / budget.available_input_tokens as f64) * 1000.0).round() / 10.0
        };

        let model_family = ModelPromptFamily::classify(&cfg).as_str().to_string();
        let fp = PrefixFingerprint::from_wire_prefix(&full_system, &active_schemas);
        let cache_prefix_hash = fp.combined_hash;
        let prefix_pin_status = self.last_prefix_status.lock().unwrap().clone();
        let sample_usage = ProviderTokenUsage {
            prompt_tokens: used_input_tokens,
            completion_tokens: 0,
            cached_prompt_tokens: 0,
            is_exact: false,
        };
        let billing_estimate =
            BillingMetering::evaluate(&cfg.provider, &cfg.model_name, &cfg.api_endpoint, &sample_usage).format_honest();

        ContextInspectionSnapshot {
            profile: spec.profile.as_str().to_string(),
            model_name: cfg.model_name,
            model_family,
            provider: cfg.provider,
            posture: posture.slug().to_string(),
            cache_prefix_hash,
            prefix_pin_status,
            billing_estimate,
            static_prefix_tokens,
            dynamic_workspace_tokens,
            system_total_tokens,
            tool_active_count: active_schemas.len(),
            tool_total_count: full_schemas.len(),
            tool_schema_tokens,
            tool_full_schema_tokens,
            unlocked_specialist_tools: unlocked_set,
            evicted_specialist_tools: evicted_set,
            deferred_specialist_tools: deferred,
            memory_entries,
            memory_tokens,
            history_messages,
            rolling_summaries,
            history_tokens,
            context_window_tokens: budget.context_window_tokens,
            available_input_tokens: budget.available_input_tokens,
            reserved_output_tokens: budget.reserved_output_tokens,
            safety_margin_tokens: budget.safety_margin_tokens,
            used_input_tokens,
            headroom_tokens,
            signed_headroom_tokens,
            utilization_pct,
            compaction_trigger_tokens: budget.compaction_trigger_tokens,
            joyzoning_detected: facts.joyzoning_detected,
            manifests: facts.manifests,
            skills_discovered: skills.len(),
            governance_sources: facts.governance_sources,
            governance_warnings: facts.governance_warnings,
        }
    }

    /// Builds a zero-side-effect `RequestManifest` (`heav3ns.request_manifest/v1`) without mutating session or unlocking tools!
    pub fn build_request_manifest(&self, sample_prompt: &str) -> RequestManifest {
        let cfg = self.config_snapshot();
        let posture = self.current_posture();
        let soul = Some(self.tools().skill_catalog.lock().unwrap().active_soul.clone());
        let spec = SystemPromptSpec::for_turn(&cfg, ExecutionAuthorityLevel::Autonomous, soul);

        // Read-only tool schema projection (does not mutate unlocked_lru)
        let unlocked = self.tools().unlocked_tools.lock().unwrap().clone();
        let tools_wire: Vec<Value> = self
            .tools()
            .list_specs()
            .into_iter()
            .filter(|s| (!self.forbids_swarm_delegation() || s.name != "engage_swarm") && (s.is_core || unlocked.contains(s.name)))
            .map(|s| s.to_openai_tool())
            .collect();
        let tool_toks = estimate_text_tokens(&Value::Array(tools_wire.clone()).to_string());
        let pinned = self.pinned_system_prompt.lock().unwrap().clone();

        let mut msgs = {
            let sess = self.session().lock().unwrap();
            let mem = self.memory().lock().unwrap();
            let (m, _) = PromptComposer::compose_with_posture(
                &cfg,
                self.workspace_root(),
                &sess,
                &mem,
                tool_toks,
                &spec,
                posture,
                pinned.as_deref(),
            );
            m
        };
        if !sample_prompt.trim().is_empty() {
            msgs.push(SessionMessage::new(Role::User, sample_prompt.trim()));
        }

        let base_wire = lookup_provider_descriptor(&cfg.provider)
            .map(|d| d.wire_dialect)
            .unwrap_or_else(|| infer_wire_dialect_from_endpoint(&cfg.api_endpoint));
        let (model_wire, wire_model) =
            resolve_model_aware_wire_and_model(&cfg.provider, &cfg.model_name, &cfg.api_endpoint, base_wire);
        let active_wire = cfg
            .wire_dialect
            .as_deref()
            .and_then(WireDialect::parse_override)
            .unwrap_or(model_wire);
        let status = self.last_prefix_status.lock().unwrap().clone();
        RequestManifest::build(&wire_model, posture, &msgs, &tools_wire, cfg.context_window_tokens, &status)
            .with_route_metadata(&cfg.provider, active_wire.as_str(), &cfg.api_endpoint, cfg.reasoning_effort.as_deref())
    }

    pub fn render_request_manifest(&self, arg: &str) -> String {
        let trimmed = arg.trim();
        let (as_json, sample) = if trimmed == "json" || trimmed == "--json" {
            (true, "")
        } else if let Some(rest) = trimmed.strip_prefix("--json ") {
            (true, rest.trim())
        } else {
            (false, trimmed)
        };
        let manifest = self.build_request_manifest(sample);
        if as_json {
            serde_json::to_string_pretty(&manifest).unwrap_or_default()
        } else {
            manifest.render_human_readable()
        }
    }

    pub fn render_context_hud(&self, arg: &str) -> String {
        let trimmed = arg.trim();
        let snap = self.inspect_context_snapshot("");
        if matches!(trimmed, "json" | "--json") {
            return serde_json::to_string_pretty(&snap).unwrap_or_default();
        }
        let filled = ((snap.utilization_pct / 100.0) * 20.0).round().clamp(0.0, 20.0) as usize;
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(20usize.saturating_sub(filled)));
        let saved_tool_toks = snap.tool_full_schema_tokens.saturating_sub(snap.tool_schema_tokens);
        let gov_label = if snap.governance_sources.is_empty() {
            "none".to_string()
        } else {
            snap.governance_sources.join(", ")
        };
        let unlocked_label = if snap.unlocked_specialist_tools.is_empty() {
            format!("none (core {} active)", snap.tool_active_count)
        } else {
            snap.unlocked_specialist_tools.join(", ")
        };
        let mut lines = vec![
            "HEAV3NS-MICRO Context & Prompt Inspector".to_string(),
            "────────────────────────────────────────────────────────────────────────".to_string(),
            format!(
                "Model & Posture : {} ({} · {} · provider={} · mode={} · est={})",
                snap.model_name, snap.model_family, snap.profile, snap.provider, snap.posture, snap.billing_estimate
            ),
            format!(
                "Cache Prefix    : fnv1a:{} [{}] ({}t static KV-prefix + {}t workspace tail)",
                snap.cache_prefix_hash, snap.prefix_pin_status, snap.static_prefix_tokens, snap.dynamic_workspace_tokens
            ),
            format!(
                "Input Budget    : [{}] {:.1}% ({}t / {}t cap · window={}t)",
                bar, snap.utilization_pct, snap.used_input_tokens, snap.available_input_tokens, snap.context_window_tokens
            ),
            "────────────────────────────────────────────────────────────────────────".to_string(),
            format!("  • System Static Prefix (KV-Cached) : {:>7}t", snap.static_prefix_tokens),
            format!("  • Workspace & Governance Tail      : {:>7}t", snap.dynamic_workspace_tokens),
            format!(
                "  • Active Tool Schemas ({:>2}/{:>2})      : {:>7}t  (saved {}t via sticky 8-LRU routing)",
                snap.tool_active_count, snap.tool_total_count, snap.tool_schema_tokens, saved_tool_toks
            ),
            format!(
                "  • Persistent LUMI-MEMORY/1 ({:>2})    : {:>7}t",
                snap.memory_entries, snap.memory_tokens
            ),
            format!(
                "  • Retained History ({:>2} msgs, {} sum): {:>7}t",
                snap.history_messages, snap.rolling_summaries, snap.history_tokens
            ),
            format!(
                "  • Signed Input Headroom            : {:>+7}t  (auto-compacts at {}t)",
                snap.signed_headroom_tokens, snap.compaction_trigger_tokens
            ),
            "────────────────────────────────────────────────────────────────────────".to_string(),
            format!(
                "Workspace       : Joyzoning={} | Rules={} | Skills={} | Manifests={}",
                if snap.joyzoning_detected { "yes" } else { "no" },
                gov_label,
                snap.skills_discovered,
                if snap.manifests.is_empty() { "none".to_string() } else { snap.manifests.join(", ") }
            ),
            format!("Unlocked Tools  : {unlocked_label}"),
            format!("On-Demand Tools : {}", snap.deferred_specialist_tools.join(", ")),
        ];
        if !snap.evicted_specialist_tools.is_empty() {
            lines.push(format!("Evicted (LRU-8) : {}", snap.evicted_specialist_tools.join(", ")));
        }
        if !snap.governance_warnings.is_empty() {
            lines.push(format!("Quarantined     : {}", snap.governance_warnings.join("; ")));
        }
        lines.join("\n")
    }

    pub fn render_prompt_inspector(&self, mode: &str) -> String {
        let cfg = self.config_snapshot();
        let facts = WorkspacePromptProbe::probe_cached(self.workspace_root());
        let skills = discover_skills_cached(self.workspace_root());
        let soul = Some(self.tools().skill_catalog.lock().unwrap().active_soul.clone());
        let trimmed = mode.trim().to_ascii_lowercase();

        let spec = match trimmed.as_str() {
            "compact" => SystemPromptSpec {
                profile: PromptProfile::Compact,
                authority: ExecutionAuthorityLevel::Autonomous,
                active_soul: soul,
                swarm_specialist: None,
            },
            "specialist" | "swarm" => SystemPromptSpec::for_specialist(
                "Core Implementation Specialist",
                "Inspect and verify assigned module",
                vec!["src/core/engine.rs".to_string()],
            ),
            "side" | "side_question" => SystemPromptSpec::for_side_question(),
            _ => SystemPromptSpec::for_turn(&cfg, ExecutionAuthorityLevel::Autonomous, soul),
        };

        if trimmed == "prefix" {
            return spec.static_prefix(&cfg);
        }
        let rendered = spec.render(&cfg, self.workspace_root(), &facts, &skills);
        if trimmed == "json" || trimmed == "--json" {
            return serde_json::to_string_pretty(&serde_json::json!({
                "profile": spec.profile.as_str(),
                "modelFamily": ModelPromptFamily::classify(&cfg).as_str(),
                "cachePrefixHash": spec.cache_prefix_fingerprint(&cfg),
                "staticPrefixTokens": estimate_text_tokens(&spec.static_prefix(&cfg)),
                "totalTokens": estimate_text_tokens(&rendered),
                "systemPrompt": rendered,
            }))
            .unwrap_or_default();
        }
        format!(
            "[profile={} · family={} · prefix_hash={} · tokens={}t]\n\n{}",
            spec.profile.as_str(),
            ModelPromptFamily::classify(&cfg).as_str(),
            spec.cache_prefix_fingerprint(&cfg),
            estimate_text_tokens(&rendered),
            rendered
        )
    }

    pub fn render_tools_status(&self) -> String {
        let specs = builtin_tool_specs();
        let unlocked = self.tools().unlocked_tools.lock().unwrap().clone();
        let snap = self.inspect_context_snapshot("");
        let mut lines = vec![format!(
            "Registered Built-in Tools ({} available · {} active [{}t] / {}t full):",
            specs.len(),
            snap.tool_active_count,
            snap.tool_schema_tokens,
            snap.tool_full_schema_tokens
        )];
        for s in specs {
            let badge = if s.is_core {
                "[CORE]    "
            } else if unlocked.contains(s.name) {
                "[UNLOCKED]"
            } else {
                "[DEFERRED]"
            };
            lines.push(format!("  {badge} {:26} - {}", s.name, s.description));
        }
        lines.join("\n")
    }

    pub fn inspect_or_switch_model(&self, arg: &str) -> String {
        let trimmed = arg.trim();
        if trimmed.is_empty() {
            let cfg = self.config_snapshot();
            format!(
                "Active model: {} (family: {}, profile: {}, provider: {}, fallback: {:?})",
                cfg.model_name,
                ModelPromptFamily::classify(&cfg).as_str(),
                PromptProfile::select(&cfg).as_str(),
                cfg.provider,
                cfg.fallback_model
            )
        } else if trimmed.eq_ignore_ascii_case("auto") {
            self.set_active_model_name("auto");
            let cfg = self.config_snapshot();
            let sample = select_auto_routed_model(&cfg.provider, "auto", "architectural refactor");
            format!(
                "Enabled `/model auto` same-provider routing on '{}' (strong tier -> {}, fast tier -> fast model).",
                cfg.provider, sample.selected_model
            )
        } else if let Some((prov_part, model_part)) = trimmed.split_once(':') {
            if let Some(retired_msg) = retired_provider_remediation(prov_part) {
                return retired_msg.to_string();
            }
            if lookup_provider_descriptor(prov_part).is_some() && !model_part.trim().is_empty() {
                self.switch_http_provider(prov_part.trim(), model_part.trim(), None, None);
                let cfg = self.config_snapshot();
                return format!(
                    "Switched active provider & model to: {} / {} (endpoint: {}, KV-prefix repin declared)",
                    cfg.provider,
                    cfg.model_name,
                    redact_endpoint_for_display(&cfg.api_endpoint)
                );
            }
            self.set_active_model_name(trimmed);
            let cfg = self.config_snapshot();
            format!(
                "Switched active model to: {} (family: {}, profile: {}, KV-prefix repin declared)",
                cfg.model_name,
                ModelPromptFamily::classify(&cfg).as_str(),
                PromptProfile::select(&cfg).as_str()
            )
        } else {
            self.set_active_model_name(trimmed);
            let cfg = self.config_snapshot();
            format!(
                "Switched active model to: {} (family: {}, profile: {}, KV-prefix repin declared)",
                cfg.model_name,
                ModelPromptFamily::classify(&cfg).as_str(),
                PromptProfile::select(&cfg).as_str()
            )
        }
    }

    pub fn inspect_or_switch_provider(&self, arg: &str) -> String {
        let trimmed = arg.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("status") {
            let cfg = self.config_snapshot();
            let desc = lookup_provider_descriptor(&cfg.provider);
            let base_wire = desc
                .map(|d| d.wire_dialect)
                .unwrap_or_else(|| infer_wire_dialect_from_endpoint(&cfg.api_endpoint));
            let (model_wire, _) =
                resolve_model_aware_wire_and_model(&cfg.provider, &cfg.model_name, &cfg.api_endpoint, base_wire);
            let wire = cfg
                .wire_dialect
                .as_deref()
                .and_then(WireDialect::parse_override)
                .unwrap_or(model_wire);
            let key_ready = if cfg.api_key.as_ref().is_some_and(|k| !k.trim().is_empty()) {
                "configured"
            } else if desc.is_some_and(|d| !d.requires_api_key) {
                "local (no key required)"
            } else {
                "missing"
            };
            let fb_str = if cfg.fallback_providers.is_empty() {
                "none".to_string()
            } else {
                cfg.fallback_providers.join(" -> ")
            };
            return format!(
                "Active provider: {} ({})\n  Model      : {}\n  Wire       : {}\n  Endpoint   : {}\n  Reasoning  : {}\n  Fallbacks  : {}\n  Credential : {}",
                cfg.provider,
                desc.map(|d| d.display_name).unwrap_or("Custom/Scripted"),
                cfg.model_name,
                wire.as_str(),
                redact_endpoint_for_display(&cfg.api_endpoint),
                cfg.reasoning_effort.as_deref().unwrap_or("default"),
                fb_str,
                key_ready
            );
        }
        if trimmed.eq_ignore_ascii_case("list") || trimmed.eq_ignore_ascii_case("all") {
            let cfg = self.config_snapshot();
            let active = cfg.provider.as_str();
            let active_count = PROVIDER_DESCRIPTORS.iter().filter(|d| !d.retired).count();
            let mut lines = vec![format!(
                "Supported Sovereign Providers ({active_count} canonical routes):"
            )];
            for d in PROVIDER_DESCRIPTORS {
                if d.retired {
                    continue;
                }
                let marker = if d.id == active { "▸" } else { " " };
                let vault_ready = cfg.provider_keys.get(d.secret_slot).is_some_and(|v| !v.trim().is_empty())
                    || (d.id == active && cfg.api_key.as_ref().is_some_and(|v| !v.trim().is_empty()));
                let ready = if d.billing_class == ProviderBillingClass::LocalNoBill || !d.requires_api_key {
                    "LOCAL"
                } else if vault_ready || d.env_vars.iter().any(|ev| std::env::var(ev).ok().is_some_and(|v| !v.trim().is_empty())) {
                    "READY"
                } else {
                    "  -- "
                };
                let env_hint = d.env_vars.first().copied().unwrap_or("(none)");
                lines.push(format!(
                    " {marker} [{ready}] {:20} {:18} model={:26} env={}",
                    d.id,
                    d.wire_dialect.as_str(),
                    d.default_model,
                    env_hint
                ));
            }
            return lines.join("\n");
        }
        if let Some(rest) = trimmed.strip_prefix("fallback").or_else(|| trimmed.strip_prefix("fallbacks")) {
            let arg_fb = rest.trim();
            if arg_fb.is_empty() {
                let fbs = self.config_snapshot().fallback_providers;
                return if fbs.is_empty() {
                    "Fallback provider chain: none (set via `/provider fallback anthropic,deepseek` or `clear`).".to_string()
                } else {
                    format!("Fallback provider chain: {}", fbs.join(" -> "))
                };
            }
            if matches!(arg_fb.to_ascii_lowercase().as_str(), "clear" | "off" | "none") {
                self.set_fallback_providers(Vec::new());
                return "Cleared fallback provider chain.".to_string();
            }
            let parsed: Vec<String> = arg_fb
                .split(|c: char| c == ',' || c.is_whitespace())
                .map(str::trim)
                .filter(|s| !s.is_empty() && retired_provider_remediation(s.split(':').next().unwrap_or(s)).is_none())
                .map(String::from)
                .collect();
            self.set_fallback_providers(parsed.clone());
            return format!("Updated fallback provider chain -> {}", parsed.join(" -> "));
        }

        let mut parts = trimmed.split_whitespace();
        let target_prov = parts.next().unwrap_or("");
        let target_model = parts.next().unwrap_or("");
        if let Some(retired_msg) = retired_provider_remediation(target_prov) {
            return retired_msg.to_string();
        }
        if lookup_provider_descriptor(target_prov).is_none() && !target_prov.eq_ignore_ascii_case("custom") {
            let active_count = PROVIDER_DESCRIPTORS.iter().filter(|d| !d.retired).count();
            if let Some(suggested) = suggest_provider_slug(target_prov) {
                return format!(
                    "Unknown provider '{target_prov}'. Did you mean '{suggested}'? Run `/provider {suggested}` or `/provider list` ({active_count} supported providers)."
                );
            }
            return format!(
                "Unknown provider '{target_prov}'. Run `/provider list` to inspect all {active_count} supported providers."
            );
        }
        self.switch_http_provider(target_prov, target_model, None, None);
        let cfg = self.config_snapshot();
        let base_wire = lookup_provider_descriptor(&cfg.provider)
            .map(|d| d.wire_dialect)
            .unwrap_or_else(|| infer_wire_dialect_from_endpoint(&cfg.api_endpoint));
        let (wire, _) =
            resolve_model_aware_wire_and_model(&cfg.provider, &cfg.model_name, &cfg.api_endpoint, base_wire);
        format!(
            "Switched provider -> {} (model: {}, wire: {}, endpoint: {}, key: {})",
            cfg.provider,
            cfg.model_name,
            wire.as_str(),
            redact_endpoint_for_display(&cfg.api_endpoint),
            if cfg.api_key.is_some() { "configured" } else { "unset" }
        )
    }

    pub fn inspect_or_switch_reasoning(&self, arg: &str) -> String {
        let trimmed = arg.trim();
        if trimmed.is_empty() {
            let cfg = self.config_snapshot();
            return format!(
                "Active reasoning effort: {} (options: default | none/off | low | medium | high | xhigh)",
                cfg.reasoning_effort.as_deref().unwrap_or("default")
            );
        }
        match ReasoningEffortLevel::parse_flexible(trimmed) {
            Ok(Some(ReasoningEffortLevel::None)) => {
                self.set_reasoning_effort(Some("none".to_string()));
                "Reasoning effort set to 'none' (thinking disabled) for subsequent turns.".to_string()
            }
            Ok(Some(level)) => {
                self.set_reasoning_effort(Some(level.as_str().to_string()));
                format!("Reasoning effort set to '{}' for subsequent turns.", level.as_str())
            }
            Ok(None) => {
                self.set_reasoning_effort(None);
                "Reasoning effort cleared (using model/provider default).".to_string()
            }
            Err(msg) => msg,
        }
    }

    pub(crate) fn format_snapshots_list(&self) -> String {
        let snaps = self.snapshots_list();
        if snaps.is_empty() {
            "No snapshots recorded.".to_string()
        } else {
            snaps
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    format!(
                        "#{i}: id={} frame={} msgs={} arena={}B",
                        s.snapshot_id,
                        s.frame_index,
                        s.messages.len(),
                        s.slab_snapshot.allocated_bytes
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}
