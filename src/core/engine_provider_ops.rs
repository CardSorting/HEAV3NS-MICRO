// [LAYER: CORE]
use crate::core::engine::AgentEngine;
use crate::domain::interfaces::{
    IModelPricingRegistry, IWorkspaceSnapshotCas,
};
use crate::domain::pricing_and_taxonomy::{
    select_auto_routed_model_with_catalog, AutoRouteDecision,
};
use crate::domain::turn_receipt::{FileChangeReceipt, TurnReceipt};
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::model_catalog::ModelPricingCatalog;
use crate::domain::types::provider_registry::{
    canonicalize_provider_id, lookup_provider_descriptor, resolve_provider_route,
    WireDialect,
};
use crate::domain::types::tick_result::{ProviderError, ProviderRequest};
use crate::infrastructure::http_provider::{
    probe_local_ollama_model, probe_local_ollama_models, probe_openai_compatible_models,
    HttpProvider,
};
use crate::infrastructure::pricing_updater::global_pricing_registry;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

impl AgentEngine {
    pub fn receipts_snapshot(&self) -> Vec<TurnReceipt> {
        self.turn_receipts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn latest_receipt(&self) -> Option<TurnReceipt> {
        self.turn_receipts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last()
            .cloned()
    }

    pub async fn probe_ollama_default_model() -> Option<String> {
        probe_local_ollama_model().await
    }

    pub async fn probe_ollama_models() -> Vec<String> {
        probe_local_ollama_models().await
    }

    pub async fn probe_provider_models_live(
        &self,
        provider_id: &str,
        saved_keys: &BTreeMap<String, String>,
        include_cloud: bool,
    ) -> Vec<String> {
        let canon = lookup_provider_descriptor(provider_id)
            .map(|d| d.id)
            .unwrap_or(provider_id);
        if canon.eq_ignore_ascii_case("ollama") {
            return probe_local_ollama_models().await;
        }
        let is_local_gateway = matches!(canon, "vllm" | "sglang" | "lmstudio" | "custom");
        if !is_local_gateway && !include_cloud {
            return Vec::new();
        }
        let cfg = self.config_snapshot();
        let mut merged_keys = cfg.provider_keys.clone();
        for (k, v) in saved_keys {
            if !v.trim().is_empty() {
                merged_keys.insert(k.clone(), v.clone());
            }
        }
        let route = resolve_provider_route(
            canon,
            None,
            None,
            None,
            None,
            Some(&cfg.provider),
            cfg.api_key.as_deref(),
            &merged_keys,
            &cfg.provider_endpoints,
            |env_key| std::env::var(env_key).ok(),
        );
        if !matches!(
            route.wire_dialect,
            WireDialect::ChatCompletions | WireDialect::OpenAiResponses
        ) {
            return Vec::new();
        }
        let requires_key =
            lookup_provider_descriptor(&route.provider_id).is_some_and(|d| d.requires_api_key);
        if requires_key && route.api_key.is_none() {
            return Vec::new();
        }
        probe_openai_compatible_models(&route.api_endpoint, route.api_key.as_deref(), 800).await
    }

    pub fn model_catalog_snapshot() -> ModelPricingCatalog {
        global_pricing_registry().snapshot_catalog()
    }

    pub fn trigger_model_catalog_auto_refresh() {
        global_pricing_registry().ensure_fresh_background();
    }

    pub async fn refresh_model_catalog() -> Result<ModelPricingCatalog, String> {
        global_pricing_registry().refresh_now().await
    }

    pub fn route_turn_model(
        provider: &str,
        configured_model: &str,
        prompt: &str,
    ) -> AutoRouteDecision {
        if !provider.eq_ignore_ascii_case("scripted") {
            global_pricing_registry().ensure_fresh_background();
        }
        let catalog = global_pricing_registry().snapshot_catalog();
        select_auto_routed_model_with_catalog(provider, configured_model, prompt, &catalog)
    }

    /// Hot-swaps the active provider, model, endpoint, and API key on `AgentEngine` with strict `secret_slot`
    /// isolation (preventing cross-provider secret leakage) and 4-dialect wire selection.
    pub fn switch_http_provider(
        &self,
        provider_id: &str,
        model_name: &str,
        endpoint_override: Option<&str>,
        api_key_override: Option<String>,
    ) {
        let snap = self.config_snapshot();
        self.switch_http_provider_with_vault(
            provider_id,
            model_name,
            endpoint_override,
            api_key_override,
            &snap.provider_keys,
            &snap.provider_endpoints,
        );
    }

    /// Hot-swaps the active provider using optional persisted vault maps (`provider_keys` & `provider_endpoints`).
    pub fn switch_http_provider_with_vault(
        &self,
        provider_id: &str,
        model_name: &str,
        endpoint_override: Option<&str>,
        api_key_override: Option<String>,
        saved_keys: &BTreeMap<String, String>,
        saved_endpoints: &BTreeMap<String, String>,
    ) {
        let mut cfg = self.config_snapshot();
        for (k, v) in saved_keys {
            if !v.trim().is_empty() {
                cfg.provider_keys.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in saved_endpoints {
            if !v.trim().is_empty() {
                cfg.provider_endpoints.insert(k.clone(), v.clone());
            }
        }

        let prev_prov = cfg.provider.clone();
        let prev_key = cfg.api_key.clone();
        let explicit_key_cleared = api_key_override
            .as_ref()
            .is_some_and(|k| k.trim().is_empty());
        if explicit_key_cleared {
            let slot = lookup_provider_descriptor(provider_id)
                .map(|d| d.secret_slot.to_string())
                .unwrap_or_else(|| provider_id.trim().to_ascii_lowercase());
            cfg.provider_keys.remove(&slot);
            cfg.provider_keys.remove(provider_id);
        } else if let Some(explicit_key) = api_key_override
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
        {
            let slot = lookup_provider_descriptor(provider_id)
                .map(|d| d.secret_slot.to_string())
                .unwrap_or_else(|| provider_id.trim().to_ascii_lowercase());
            cfg.provider_keys.insert(slot, explicit_key.to_string());
        }
        if let Some(ep) = endpoint_override.map(str::trim).filter(|e| !e.is_empty()) {
            let pid = lookup_provider_descriptor(provider_id)
                .map(|d| d.id.to_string())
                .unwrap_or_else(|| provider_id.trim().to_ascii_lowercase());
            cfg.provider_endpoints.insert(pid, ep.to_string());
        }

        let changed_provider =
            canonicalize_provider_id(&prev_prov) != canonicalize_provider_id(provider_id);
        let crossing_family = lookup_provider_descriptor(&prev_prov).map(|d| d.catalog_vendor)
            != lookup_provider_descriptor(provider_id).map(|d| d.catalog_vendor);
        if crossing_family {
            cfg.fallback_model = None;
        }
        if changed_provider {
            cfg.wire_dialect = None;
        }

        let route = resolve_provider_route(
            provider_id,
            Some(model_name).filter(|m| !m.trim().is_empty()),
            endpoint_override,
            api_key_override.as_deref().filter(|k| !k.trim().is_empty()),
            cfg.wire_dialect.as_deref(),
            if explicit_key_cleared {
                None
            } else {
                Some(prev_prov.as_str())
            },
            if explicit_key_cleared {
                None
            } else {
                prev_key.as_deref()
            },
            &cfg.provider_keys,
            &cfg.provider_endpoints,
            |env_key| std::env::var(env_key).ok(),
        );

        cfg.provider = route.provider_id.clone();
        cfg.model_name = route.model_name;
        cfg.api_endpoint = route.api_endpoint.clone();
        cfg.api_key = if explicit_key_cleared {
            None
        } else {
            route.api_key
        };

        let mut http = HttpProvider::new(
            route.provider_id,
            cfg.api_endpoint.clone(),
            cfg.api_key.clone(),
            cfg.provider_timeout_ms,
        );
        if let Some(dialect) = cfg
            .wire_dialect
            .as_deref()
            .and_then(WireDialect::parse_override)
        {
            http = http
                .with_wire_dialect(dialect)
                .with_explicit_wire_override(true);
        } else {
            http = http.with_wire_dialect(route.wire_dialect);
        }
        self.swap_provider_and_config(cfg, Arc::new(http));
    }

    pub fn cas_diff_between(
        &self,
        before_snap: &str,
        after_snap: Option<&str>,
    ) -> Result<(Vec<FileChangeReceipt>, String), String> {
        self.cas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .diff_between(before_snap, after_snap)
    }

    pub fn cas_snapshot_id_for_game_snapshot(&self, snapshot_id: &str) -> Option<String> {
        let snaps = self.snapshots_list();
        let idx = snaps.iter().position(|s| s.snapshot_id == snapshot_id)?;
        let map = self.snapshot_cas_map_clone();
        map.get(&idx).cloned()
    }

    pub fn undo_last_turn(&self, force: bool) -> Result<Value, String> {
        let last = self
            .turn_receipts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last()
            .cloned();
        if let Some(receipt) = last {
            if let (Some(before), Some(after)) = (&receipt.snapshot_before, &receipt.snapshot_after)
            {
                if !receipt.file_changes.is_empty() {
                    let actions = self
                        .cas
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .undo_turn_changes(before, after, force)?;
                    self.turn_receipts
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .pop();
                    let journal_turn_id = receipt.turn_id.replace('-', "_");
                    self.tools()
                        .journal
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .discard_turn(&journal_turn_id);
                    self.tools()
                        .cache
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clear();
                    self.tools()
                        .read_tracker
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .reset_stamps();
                    return Ok(json!({
                        "restoredCount": actions.len(),
                        "actions": actions,
                        "turnId": receipt.turn_id,
                    }));
                } else {
                    self.turn_receipts
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .pop();
                    return Ok(json!({
                        "restoredCount": 0,
                        "actions": Vec::<String>::new(),
                        "turnId": receipt.turn_id,
                    }));
                }
            }
            self.turn_receipts
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop();
        }
        let fallback = self
            .tools()
            .journal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rollback_last();
        self.tools()
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.tools()
            .read_tracker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reset_stamps();
        serde_json::to_value(fallback).map_err(|e| e.to_string())
    }

    pub async fn ask_side_question(&self, question: &str) -> Result<String, ProviderError> {
        let cfg = self.config_snapshot();
        let facts = crate::infrastructure::vfs_store::WorkspacePromptProbe::probe_cached(
            self.workspace_root(),
        );
        let side_sys = crate::domain::prompt_spec::SystemPromptSpec::for_side_question().render(
            &cfg,
            self.workspace_root(),
            &facts,
            &[],
        );
        let req = ProviderRequest {
            model: cfg.model_name,
            messages: vec![
                SessionMessage::new(Role::System, side_sys),
                SessionMessage::new(Role::User, question),
            ],
            tools: Vec::new(),
            max_tokens: cfg.max_output_tokens.min(2048),
            temperature: cfg.temperature,
            reasoning_effort: cfg.reasoning_effort.clone(),
        };
        let resp = self.provider_arc().complete(req, None).await?;
        Ok(resp.content.unwrap_or_default())
    }

}
