// [LAYER: CORE]
use crate::core::engine::AgentEngine;
use crate::core::session_store::SessionCompactor;
use crate::core::tool_specs::builtin_tool_specs;
use crate::domain::compaction::ContextBudgetInfo;
use crate::domain::prompt_spec::{ModelPromptFamily, PromptProfile, SystemPromptSpec};
use crate::domain::types::messages::{estimate_messages_tokens, Role};
use crate::domain::types::runtime::ExecutionAuthorityLevel;
use crate::infrastructure::vfs_store::{SkillsIngestor, WorkspacePromptProbe};
use crate::plumbing::text_tokens::estimate_text_tokens;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Structured telemetry snapshot of system prompt composition, KV-cache prefix, sticky tool routing, and context window headroom.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextInspectionSnapshot {
    pub profile: String,
    pub model_name: String,
    pub model_family: String,
    pub provider: String,
    pub cache_prefix_hash: String,
    pub static_prefix_tokens: usize,
    pub dynamic_workspace_tokens: usize,
    pub system_total_tokens: usize,
    pub tool_active_count: usize,
    pub tool_total_count: usize,
    pub tool_schema_tokens: usize,
    pub tool_full_schema_tokens: usize,
    pub unlocked_specialist_tools: Vec<String>,
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
        let soul = Some(self.tools().skill_catalog.lock().unwrap().active_soul.clone());
        let spec = SystemPromptSpec::for_turn(&cfg, ExecutionAuthorityLevel::Autonomous, soul);
        let facts = WorkspacePromptProbe::probe_cached(self.workspace_root());
        let skills = SkillsIngestor::discover(self.workspace_root());

        let static_prefix = spec.static_prefix(&cfg);
        let dynamic_suffix = spec.dynamic_suffix(&cfg, self.workspace_root(), &facts, &skills);
        let full_system = spec.render(&cfg, self.workspace_root(), &facts, &skills);
        let static_prefix_tokens = estimate_text_tokens(&static_prefix);
        let dynamic_workspace_tokens = estimate_text_tokens(&dynamic_suffix);
        let system_total_tokens = estimate_text_tokens(&full_system);

        let probe = if sample_prompt.trim().is_empty() { "inspect workspace" } else { sample_prompt };
        let active_schemas = self.tools().select_tool_schemas_for(probe, self.forbids_swarm_delegation());
        let full_schemas = self.tools().openai_tool_schemas();
        let tool_schema_tokens = estimate_text_tokens(&Value::Array(active_schemas.clone()).to_string());
        let tool_full_schema_tokens = estimate_text_tokens(&Value::Array(full_schemas.clone()).to_string());

        let unlocked_set: Vec<String> = {
            let guard = self.tools().unlocked_tools.lock().unwrap();
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
        let utilization_pct = if budget.available_input_tokens == 0 {
            0.0
        } else {
            ((used_input_tokens as f64 / budget.available_input_tokens as f64) * 1000.0).round() / 10.0
        };

        let model_family = ModelPromptFamily::classify(&cfg).as_str().to_string();
        let cache_prefix_hash = spec.cache_prefix_fingerprint(&cfg);

        ContextInspectionSnapshot {
            profile: spec.profile.as_str().to_string(),
            model_name: cfg.model_name,
            model_family,
            provider: cfg.provider,
            cache_prefix_hash,
            static_prefix_tokens,
            dynamic_workspace_tokens,
            system_total_tokens,
            tool_active_count: active_schemas.len(),
            tool_total_count: full_schemas.len(),
            tool_schema_tokens,
            tool_full_schema_tokens,
            unlocked_specialist_tools: unlocked_set,
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
            utilization_pct,
            compaction_trigger_tokens: budget.compaction_trigger_tokens,
            joyzoning_detected: facts.joyzoning_detected,
            manifests: facts.manifests,
            skills_discovered: skills.len(),
            governance_sources: facts.governance_sources,
            governance_warnings: facts.governance_warnings,
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
                "Model & Profile : {} ({} · {} · provider={})",
                snap.model_name, snap.model_family, snap.profile, snap.provider
            ),
            format!(
                "Cache Prefix    : fnv1a:{} ({}t static KV-prefix + {}t workspace tail)",
                snap.cache_prefix_hash, snap.static_prefix_tokens, snap.dynamic_workspace_tokens
            ),
            format!(
                "Input Budget    : [{}] {:.1}% ({}t / {}t cap · window={}t)",
                bar, snap.utilization_pct, snap.used_input_tokens, snap.available_input_tokens, snap.context_window_tokens
            ),
            "────────────────────────────────────────────────────────────────────────".to_string(),
            format!("  • System Static Prefix (KV-Cached) : {:>7}t", snap.static_prefix_tokens),
            format!("  • Workspace & Governance Tail      : {:>7}t", snap.dynamic_workspace_tokens),
            format!(
                "  • Active Tool Schemas ({:>2}/{:>2})      : {:>7}t  (saved {}t via sticky routing)",
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
                "  • Free Input Headroom              : {:>7}t  (auto-compacts at {}t)",
                snap.headroom_tokens, snap.compaction_trigger_tokens
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
        if !snap.governance_warnings.is_empty() {
            lines.push(format!("Quarantined     : {}", snap.governance_warnings.join("; ")));
        }
        lines.join("\n")
    }

    pub fn render_prompt_inspector(&self, mode: &str) -> String {
        let cfg = self.config_snapshot();
        let facts = WorkspacePromptProbe::probe_cached(self.workspace_root());
        let skills = SkillsIngestor::discover(self.workspace_root());
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
        if arg.is_empty() {
            let cfg = self.config_snapshot();
            format!(
                "Active model: {} (family: {}, profile: {}, provider: {}, fallback: {:?})",
                cfg.model_name,
                ModelPromptFamily::classify(&cfg).as_str(),
                PromptProfile::select(&cfg).as_str(),
                cfg.provider,
                cfg.fallback_model
            )
        } else {
            self.set_active_model_name(arg);
            let cfg = self.config_snapshot();
            format!(
                "Switched active model to: {} (family: {}, profile: {})",
                cfg.model_name,
                ModelPromptFamily::classify(&cfg).as_str(),
                PromptProfile::select(&cfg).as_str()
            )
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
