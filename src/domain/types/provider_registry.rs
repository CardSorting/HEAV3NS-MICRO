// [LAYER: DOMAIN]
pub use crate::domain::types::provider_catalog_data::{is_retired_provider, lookup_provider_descriptor, opencode_go_model_id, resolve_model_aware_wire_and_model, resolve_opencode_go_model, retired_provider_remediation, suggest_provider_slug, ProviderBillingClass, ProviderDescriptor, WireDialect, LEGACY_ANTIGRAVITY_TOMBSTONE_MESSAGE, OPENCODE_GO_TRANSPORTS, PROVIDER_DESCRIPTORS};
use crate::domain::types::provider_catalog_data::normalize_provider_token;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Abstract contract for sovereign multi-provider routing and failover chains (`DOMAIN`).
pub trait IProviderRouteResolver {
    fn active_provider_id(&self) -> &str;
    fn is_fallback_provider_active(&self) -> bool;
}

/// Normalized reasoning effort dial (`ReasoningPicker.swift` & Codewhale parity).
/// `None` represents explicit `"none"`/`"off"` (thinking disabled), whereas `Option::None` represents `"default"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffortLevel {
    None,
    Low,
    Medium,
    High,
    XHigh,
}

impl ReasoningEffortLevel {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
        }
    }

    pub fn parse_flexible(raw: &str) -> Result<Option<Self>, String> {
        match raw.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "" | "default" | "auto" | "clear" => Ok(None),
            "off" | "none" | "disable" | "disabled" | "false" => Ok(Some(Self::None)),
            "low" | "minimal" | "min" | "fast" => Ok(Some(Self::Low)),
            "medium" | "med" | "balanced" => Ok(Some(Self::Medium)),
            "high" | "deep" => Ok(Some(Self::High)),
            "xhigh" | "extrahigh" | "max" | "ultra" | "highest" | "ultracode" => Ok(Some(Self::XHigh)),
            other => Err(format!(
                "Invalid reasoning effort '{other}'. Expected one of: default, none/off, low, medium, high, xhigh."
            )),
        }
    }
}

/// Resolved, credential-isolated provider route ready for `AgentConfig` & `HttpProvider`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedProviderRoute {
    pub provider_id: String,
    pub display_name: String,
    pub secret_slot: String,
    pub model_name: String,
    pub api_endpoint: String,
    pub api_key: Option<String>,
    pub key_source: String,
    pub wire_dialect: WireDialect,
    pub billing_class: ProviderBillingClass,
}

/// Ordered primary + deduplicated fallback provider chain (`Codewhale provider_chain.rs` parity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderChain {
    providers: Vec<String>,
    cursor: usize,
}

impl IProviderRouteResolver for ProviderChain {
    fn active_provider_id(&self) -> &str {
        self.current()
    }

    fn is_fallback_provider_active(&self) -> bool {
        self.cursor > 0
    }
}

impl ProviderChain {
    pub fn new(primary: &str, fallbacks: &[String]) -> Self {
        let canon_primary = canonicalize_provider_id(primary);
        let mut providers = vec![canon_primary];
        for fb in fallbacks {
            let trimmed = fb.trim();
            if trimmed.is_empty() {
                continue;
            }
            let raw_prov = trimmed.split_once(':').map(|(p, _)| p).unwrap_or(trimmed);
            if is_retired_provider(raw_prov) {
                continue;
            }
            let canon = canonicalize_provider_id(raw_prov);
            if !providers.iter().any(|existing| existing == &canon) {
                providers.push(canon);
            }
        }
        Self { providers, cursor: 0 }
    }

    #[inline]
    pub fn current(&self) -> &str {
        self.providers.get(self.cursor).map(String::as_str).unwrap_or("openai")
    }

    #[inline]
    pub fn primary(&self) -> &str {
        self.providers.first().map(String::as_str).unwrap_or("openai")
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    #[inline]
    pub fn is_fallback_active(&self) -> bool {
        self.cursor > 0
    }

    pub fn advance(&mut self) -> Option<&str> {
        if self.cursor + 1 < self.providers.len() {
            self.cursor += 1;
            Some(self.current())
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    pub fn providers(&self) -> &[String] {
        &self.providers
    }
}

/// Returns the canonical provider ID if recognized, or the normalized token for custom/scripted providers.
pub fn canonicalize_provider_id(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("scripted") {
        return "scripted".to_string();
    }
    lookup_provider_descriptor(trimmed)
        .map(|d| d.id.to_string())
        .unwrap_or_else(|| normalize_provider_token(trimmed))
}

/// Pure, zero-I/O route & credential resolver with strict `secret_slot` isolation and per-model wire resolution.
#[allow(clippy::too_many_arguments)]
pub fn resolve_provider_route<F>(
    target_provider: &str,
    explicit_model: Option<&str>,
    explicit_endpoint: Option<&str>,
    explicit_api_key: Option<&str>,
    explicit_wire: Option<&str>,
    previous_provider: Option<&str>,
    previous_api_key: Option<&str>,
    saved_keys: &BTreeMap<String, String>,
    saved_endpoints: &BTreeMap<String, String>,
    env_lookup: F,
) -> ResolvedProviderRoute
where
    F: Fn(&str) -> Option<String>,
{
    let desc = lookup_provider_descriptor(target_provider);
    let provider_id = desc.map(|d| d.id.to_string()).unwrap_or_else(|| canonicalize_provider_id(target_provider));
    let display_name = desc.map(|d| d.display_name.to_string()).unwrap_or_else(|| provider_id.clone());
    let secret_slot = desc.map(|d| d.secret_slot.to_string()).unwrap_or_else(|| provider_id.clone());
    let billing_class = desc.map(|d| d.billing_class).unwrap_or(ProviderBillingClass::Metered);

    let api_endpoint = explicit_endpoint
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| desc.and_then(|d| d.endpoint_env).and_then(&env_lookup).filter(|s| !s.trim().is_empty()))
        .or_else(|| saved_endpoints.get(&provider_id).or_else(|| saved_endpoints.get(&secret_slot)).cloned().filter(|s| !s.trim().is_empty()))
        .or_else(|| desc.map(|d| d.default_endpoint.to_string()))
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

    let raw_model = explicit_model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| desc.map(|d| d.default_model.to_string()))
        .unwrap_or_else(|| "gpt-5.4".to_string());

    let base_wire = desc
        .map(|d| d.wire_dialect)
        .unwrap_or_else(|| infer_wire_dialect_from_endpoint(&api_endpoint));
    let (model_aware_wire, model_name) =
        resolve_model_aware_wire_and_model(&provider_id, &raw_model, &api_endpoint, base_wire);
    let wire_dialect = explicit_wire
        .and_then(WireDialect::parse_override)
        .unwrap_or(model_aware_wire);

    let prev_same_slot = previous_provider
        .map(|prev| {
            let prev_slot = lookup_provider_descriptor(prev)
                .map(|d| d.secret_slot)
                .unwrap_or(prev);
            prev_slot.eq_ignore_ascii_case(&secret_slot)
                || (secret_slot == "openai-codex" && prev_slot == "openai")
                || (secret_slot == "openai" && prev_slot == "openai-codex")
        })
        .unwrap_or(false);

    let mut api_key: Option<String> = None;
    let mut key_source = "none".to_string();

    if let Some(k) = explicit_api_key.map(str::trim).filter(|s| !s.is_empty()) {
        api_key = Some(k.to_string());
        key_source = "explicit".to_string();
    } else if let Some(d) = desc {
        for &env_name in d.env_vars {
            if let Some(val) = env_lookup(env_name).map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
                api_key = Some(val);
                key_source = format!("env:{env_name}");
                break;
            }
        }
    }
    if api_key.is_none() {
        if let Some(saved) = saved_keys
            .get(&secret_slot)
            .or_else(|| saved_keys.get(&provider_id))
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
        {
            api_key = Some(saved);
            key_source = format!("vault:{secret_slot}");
        } else if prev_same_slot {
            if let Some(prev_key) = previous_api_key.map(str::trim).filter(|s| !s.is_empty()) {
                api_key = Some(prev_key.to_string());
                key_source = "session_same_slot".to_string();
            }
        }
    }

    ResolvedProviderRoute {
        provider_id,
        display_name,
        secret_slot,
        model_name,
        api_endpoint,
        api_key,
        key_source,
        wire_dialect,
        billing_class,
    }
}

/// Clears an inherited config `wire_dialect` override when `--provider` switches away from the configured provider.
pub fn resolve_effective_wire_override(
    cli_wire: Option<&str>,
    cfg_wire: Option<&str>,
    cfg_provider: Option<&str>,
    selected_provider: &str,
) -> Option<String> {
    if let Some(w) = cli_wire.map(str::trim).filter(|s| !s.is_empty()) {
        return Some(w.to_string());
    }
    let same_provider = cfg_provider
        .map(canonicalize_provider_id)
        .is_some_and(|up| up == canonicalize_provider_id(selected_provider));
    if same_provider {
        cfg_wire.map(str::trim).filter(|s| !s.is_empty()).map(String::from)
    } else {
        None
    }
}

/// Infers [`WireDialect`] from an endpoint URL when no explicit override or descriptor applies.
/// Notably classifies Google's OpenAI-compatible path (`/v1beta/openai`) as [`WireDialect::ChatCompletions`].
pub fn infer_wire_dialect_from_endpoint(endpoint: &str) -> WireDialect {
    let lower = endpoint.trim().to_ascii_lowercase();
    let path_part = lower.split(['?', '#']).next().unwrap_or(&lower).trim_end_matches('/');
    if path_part.ends_with("/openai") || path_part.contains("/openai/") || path_part.ends_with("/chat/completions") {
        WireDialect::ChatCompletions
    } else if lower.contains("anthropic") || path_part.ends_with("/messages") {
        WireDialect::AnthropicMessages
    } else if lower.contains("generativelanguage.googleapis.com")
        || path_part.contains(":generatecontent")
        || path_part.contains(":streamgeneratecontent")
    {
        WireDialect::GoogleGemini
    } else if path_part.ends_with("/responses") {
        WireDialect::OpenAiResponses
    } else {
        WireDialect::ChatCompletions
    }
}
