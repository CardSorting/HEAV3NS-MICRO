// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::domain::types::provider_registry::{canonicalize_provider_id, is_retired_provider, lookup_provider_descriptor, PROVIDER_DESCRIPTORS};
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::text_tokens::mask_secret_preview;
use crate::ui::cli_presenter::Heav3nsUserConfig;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn is_valid_ipc_model_id(raw: &str) -> bool {
    if raw.is_empty() || raw.len() > 200 {
        return false;
    }
    let rest = raw.strip_prefix('~').unwrap_or(raw);
    let mut chars = rest.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '[' | ']' | '-'))
}

/// Handles `<provider>.{status,saveKey,clearKey}` and `provider.{status,saveKey,clearKey}` RPCs across all 43+ providers.
pub fn handle_provider_credential_rpc(
    method: &str,
    params: &Value,
    provider_keys: &mut BTreeMap<String, String>,
    active_provider: &str,
    active_model: &str,
    engine: &AgentEngine,
) -> Value {
    let raw_prefix = method.split('.').next().unwrap_or("openrouter");
    let action = method.split('.').nth(1).unwrap_or("status");
    let target_raw = if raw_prefix == "provider" {
        params.get("provider").and_then(Value::as_str).unwrap_or(active_provider)
    } else {
        raw_prefix
    };
    let desc = lookup_provider_descriptor(target_raw);
    let slot = desc
        .map(|d| d.secret_slot.to_string())
        .unwrap_or_else(|| canonicalize_provider_id(target_raw));
    let canon_id = desc
        .map(|d| d.id.to_string())
        .unwrap_or_else(|| slot.clone());

    let persist = params.get("persist").and_then(Value::as_bool) == Some(true);
    let active_slot = lookup_provider_descriptor(active_provider)
        .map(|d| d.secret_slot)
        .unwrap_or(active_provider);
    let affects_active = active_slot.eq_ignore_ascii_case(&slot) || active_provider.eq_ignore_ascii_case(&canon_id);

    let apply_clear = |keys: &mut BTreeMap<String, String>| {
        keys.remove(&slot);
        keys.remove(raw_prefix);
        keys.remove(&canon_id);
        if persist {
            if let Some(mut ucfg) = Heav3nsUserConfig::load() {
                ucfg.set_provider_key(&slot, String::new());
                let _ = ucfg.save();
            }
        }
        if affects_active {
            engine.switch_http_provider_with_vault(active_provider, active_model, None, Some(String::new()), keys, &BTreeMap::new());
        }
    };

    match action {
        "saveKey" => {
            if let Some(raw_k) = params.get("key").or_else(|| params.get("apiKey")).and_then(Value::as_str) {
                let clean = raw_k.trim().to_string();
                if clean.is_empty() {
                    apply_clear(provider_keys);
                } else {
                    provider_keys.insert(slot.clone(), clean.clone());
                    provider_keys.insert(raw_prefix.to_string(), clean.clone());
                    if persist {
                        let mut ucfg = Heav3nsUserConfig::load().unwrap_or_default();
                        ucfg.set_provider_key(&slot, clean.clone());
                        let _ = ucfg.save();
                    }
                    if affects_active {
                        engine.switch_http_provider_with_vault(active_provider, active_model, None, Some(clean), provider_keys, &BTreeMap::new());
                    }
                }
            }
        }
        "clearKey" => apply_clear(provider_keys),
        _ => {}
    }

    let saved_val = provider_keys
        .get(&slot)
        .or_else(|| provider_keys.get(raw_prefix))
        .cloned();
    let env_val = desc.and_then(|d| {
        d.env_vars
            .iter()
            .find_map(|ev| std::env::var(ev).ok().filter(|v| !v.trim().is_empty()))
    });
    let effective = saved_val.clone().or(env_val);
    json!({
        "provider": raw_prefix,
        "canonicalProvider": canon_id,
        "configured": effective.is_some(),
        "saved": saved_val.is_some(),
        "maskedKey": effective.as_deref().map(mask_secret_preview),
        "updatedAt": now_ms() as f64
    })
}

fn catalog_model_slugs(provider: &str, keep_vendor: bool, limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for entry in AgentEngine::model_catalog_snapshot().newest_models_for_provider(provider, limit) {
        let slug = if keep_vendor {
            entry.id.clone()
        } else {
            entry.id.split_once('/').map(|(_, s)| s.to_string()).unwrap_or_else(|| entry.id.clone())
        };
        if is_valid_ipc_model_id(&slug) && seen.insert(slug.clone()) {
            out.push(slug);
        }
    }
    if out.is_empty() {
        if let Some(d) = lookup_provider_descriptor(provider) {
            if is_valid_ipc_model_id(d.default_model) {
                out.push(d.default_model.to_string());
            }
        }
    }
    out
}

fn normalize_ipc_reasoning(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "default" => "default",
        "none" | "off" | "disabled" => "none",
        "low" | "minimal" => "low",
        "medium" | "med" => "medium",
        "high" => "high",
        "xhigh" | "max" => "xhigh",
        _ => "default",
    }
}

/// Builds the synchronized `models.list` catalog across all 65 active canonical providers (`WorkspaceModelCatalog` contract).
pub fn build_sovereign_model_catalog(provider: &str, model: &str, reasoning: &str, revision: i64, local_ollama: &[String]) -> Value {
    build_sovereign_model_catalog_with_recent(provider, model, reasoning, revision, local_ollama, &[])
}

pub fn build_sovereign_model_catalog_with_recent(
    provider: &str,
    model: &str,
    reasoning: &str,
    revision: i64,
    local_ollama: &[String],
    recent_entries: &[(String, String, String)],
) -> Value {
    let active_prov = canonicalize_provider_id(provider);
    let safe_prov = if active_prov.is_empty() || active_prov == "scripted" || is_retired_provider(&active_prov) {
        "openai-codex".to_string()
    } else {
        active_prov
    };
    let safe_reasoning = normalize_ipc_reasoning(reasoning);
    let ollama_models: Vec<String> = {
        let mut dedup = Vec::new();
        for m in local_ollama {
            let trimmed = m.trim();
            if is_valid_ipc_model_id(trimmed) && !dedup.iter().any(|x| x == trimmed) {
                dedup.push(trimmed.to_string());
            }
        }
        if dedup.is_empty() {
            vec!["qwen2.5-coder:14b".to_string(), "deepseek-r1:14b".to_string(), "llama3.2".to_string()]
        } else {
            dedup
        }
    };

    let curated: &[(&str, &str, &str, bool)] = &[
        ("openai-codex", "OpenAI Codex", "OpenAI Responses (/v1/responses) reasoning & tool execution", false),
        ("openai", "OpenAI API", "Direct OpenAI Chat Completions frontier models", false),
        ("claude-subscription-directsdk-experimental", "Claude Code", "Anthropic Claude subscription & OAuth direct SDK", false),
        ("anthropic", "Anthropic Claude", "Native Anthropic Messages (/v1/messages) with KV-pinning", false),
        ("gemini", "Google Gemini", "Native Gemini v1beta high-context multimodal synthesis", false),
        ("deepseek", "DeepSeek", "DeepSeek V4 Pro / V4.1 Flash / Reasoner", false),
        ("xai", "xAI Grok", "Grok 4.7 & Grok Build fast engineering models", false),
        ("groq", "Groq", "Ultra-low-latency LPU inference gateway", false),
        ("mistral", "Mistral AI", "Mistral Large & Codestral code specialists", false),
        ("qwen", "Alibaba Qwen", "DashScope Qwen 3.8 Max Prime & Coder", false),
        ("moonshot", "Moonshot Kimi", "Kimi K3.1 Pro & K2.7 Code 1M-context models", false),
        ("zai", "Z.ai GLM", "GLM 5.3 FlashX & GLM 5.1 reasoning suite", false),
        ("nvidia-nim", "NVIDIA NIM", "NVIDIA Nemotron & enterprise NIM endpoints", true),
        ("openrouter", "OpenRouter", "Live auto-refreshed 460+ multi-vendor gateway", true),
        ("orcarouter", "OrcaRouter", "Unified multi-vendor reasoning & coding router", true),
        ("codewhale", "Codewhale", "Model-aware multi-dialect cloud gateway", true),
        ("cline-pass", "Cline Pass", "Cline multi-vendor coding gateway", true),
        ("nous", "Nous Research", "Hermes 4 70B & 405B open-weights frontier", false),
        ("opencode", "OpenCode Zen", "OpenCode Zen model-aware coding router", false),
        ("opencode-go", "OpenCode Go", "OpenCode Go multi-dialect subscription roster", false),
        ("ollama", "Local Ollama (Zero-Key)", "100% local private inference on localhost:11434", false),
        ("apple-playground", "On this Mac (Playground)", "Local Apple Foundation Models on-device loopback", false),
    ];

    let mut seen_ids = BTreeSet::new();
    let mut providers_json = Vec::with_capacity(PROVIDER_DESCRIPTORS.len());
    for &(id, label, detail, keep_vendor) in curated {
        seen_ids.insert(id.to_string());
        let models = match id {
            "ollama" => ollama_models.clone(),
            "apple-playground" => vec![
                "apple-foundation-local".to_string(),
                "apple-foundation-model".to_string(),
                "apple-playground-coding".to_string(),
            ],
            _ => catalog_model_slugs(id, keep_vendor, 6),
        };
        providers_json.push(json!({ "id": id, "label": label, "detail": detail, "models": models }));
    }

    for d in PROVIDER_DESCRIPTORS {
        if d.retired {
            continue;
        }
        if seen_ids.insert(d.id.to_string()) {
            let keep_vendor = matches!(
                d.id,
                "together"
                    | "fireworks"
                    | "deepinfra"
                    | "huggingface"
                    | "modelscope"
                    | "atlascloud"
                    | "siliconflow"
                    | "siliconflow-CN"
                    | "baseten"
                    | "novita"
                    | "edenai"
                    | "zenmux"
            );
            providers_json.push(json!({
                "id": d.id,
                "label": d.display_name,
                "detail": format!("{} ({})", d.display_name, d.wire_dialect.as_str()),
                "models": catalog_model_slugs(d.id, keep_vendor, 6)
            }));
        }
    }

    if !seen_ids.contains(&safe_prov) {
        let (lbl, def_m) = lookup_provider_descriptor(&safe_prov)
            .map(|d| (d.display_name, d.default_model))
            .unwrap_or(("Custom Gateway", model));
        seen_ids.insert(safe_prov.clone());
        providers_json.push(json!({
            "id": safe_prov,
            "label": lbl,
            "detail": "Sovereign configured provider route",
            "models": [def_m]
        }));
    }

    let mut recent_json = Vec::new();
    let mut seen_recent = BTreeSet::new();
    for (rp, rm, rr) in recent_entries {
        let canon_rp = canonicalize_provider_id(rp);
        if seen_ids.contains(&canon_rp)
            && is_valid_ipc_model_id(rm)
            && seen_recent.insert((canon_rp.clone(), rm.clone()))
        {
            recent_json.push(json!({
                "provider": canon_rp,
                "model": rm,
                "reasoningEffort": normalize_ipc_reasoning(rr),
            }));
            if recent_json.len() >= 8 {
                break;
            }
        }
    }

    json!({
        "provider": safe_prov,
        "model": model,
        "revision": revision,
        "reasoningEffort": safe_reasoning,
        "reasoningOptions": ["none", "low", "medium", "high", "xhigh"],
        "busy": false,
        "blocked": null,
        "providers": providers_json,
        "recent": recent_json
    })
}

/// Builds `models.discover` payload with deduplicated, validated model IDs for any canonical provider.
pub fn build_sovereign_model_discovery(provider: &str, live_probed: &[String]) -> Value {
    let valid_provider = lookup_provider_descriptor(provider)
        .filter(|d| !d.retired)
        .map(|d| d.id)
        .unwrap_or("openai-codex");
    let cat = AgentEngine::model_catalog_snapshot();
    let models: Vec<Value> = match valid_provider {
        "ollama" => {
            let mut seen = BTreeSet::new();
            let live: Vec<Value> = live_probed
                .iter()
                .map(|m| m.trim())
                .filter(|m| is_valid_ipc_model_id(m) && seen.insert((*m).to_string()))
                .map(|m| json!({ "id": m, "label": m, "detail": "Live local Ollama tag (/api/tags)" }))
                .collect();
            if live.is_empty() {
                vec![
                    json!({ "id": "qwen2.5-coder:14b", "label": "Qwen 2.5 Coder 14B", "detail": "Local code specialist" }),
                    json!({ "id": "llama3.2", "label": "Llama 3.2 (Local)", "detail": "Zero-key local Ollama inference" }),
                ]
            } else {
                live
            }
        }
        "apple-playground" => vec![
            json!({ "id": "apple-foundation-local", "label": "Apple Foundation Local", "detail": "On-device Apple Intelligence loopback" }),
            json!({ "id": "apple-foundation-model", "label": "Apple On-Device Model", "detail": "Local macOS Foundation Models adapter" }),
        ],
        _ => {
            let keep_vendor = matches!(
                valid_provider,
                "openrouter"
                    | "orcarouter"
                    | "codewhale"
                    | "cline-pass"
                    | "nvidia-nim"
                    | "together"
                    | "fireworks"
                    | "deepinfra"
                    | "huggingface"
                    | "modelscope"
                    | "atlascloud"
                    | "siliconflow"
                    | "siliconflow-CN"
                    | "baseten"
                    | "novita"
                    | "edenai"
                    | "zenmux"
            );
            let mut seen = BTreeSet::new();
            let mut list = Vec::new();
            for m in live_probed {
                let id = m.trim();
                if is_valid_ipc_model_id(id) && seen.insert(id.to_string()) {
                    list.push(json!({ "id": id, "label": id, "detail": "Live gateway /v1/models discovery" }));
                }
            }
            for e in cat.newest_models_for_provider(valid_provider, 12) {
                let id = if keep_vendor {
                    e.id.clone()
                } else {
                    e.id.split_once('/').map(|(_, s)| s.to_string()).unwrap_or_else(|| e.id.clone())
                };
                if is_valid_ipc_model_id(&id) && seen.insert(id.clone()) {
                    list.push(json!({
                        "id": id,
                        "label": e.display_name,
                        "detail": format!("{}k ctx · ${:.2}/${:.2} per MTok", e.context_length / 1000, e.input_per_million, e.output_per_million)
                    }));
                }
            }
            if list.is_empty() {
                if let Some(d) = lookup_provider_descriptor(valid_provider) {
                    if is_valid_ipc_model_id(d.default_model) {
                        list.push(json!({
                            "id": d.default_model,
                            "label": d.default_model,
                            "detail": format!("{} default route ({})", d.display_name, d.wire_dialect.as_str())
                        }));
                    }
                }
            }
            list
        }
    };
    json!({
        "provider": valid_provider,
        "state": "ready",
        "source": format!("HEAV3NS-MICRO Catalog ({})", cat.source),
        "message": "Available reasoning and code orchestration models verified.",
        "checkedAt": now_ms() as f64,
        "limited": false,
        "models": models
    })
}
