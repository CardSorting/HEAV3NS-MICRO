// [LAYER: INFRASTRUCTURE]
use crate::domain::types::provider_registry::{canonicalize_provider_id, lookup_provider_descriptor};
use serde_json::{json, Value};

/// Returns `true` when a Chat Completions route targets Google's OpenAI-compatible Gemini endpoint.
pub(crate) fn is_google_openai_compat_route(provider_name: &str, endpoint: &str) -> bool {
    let ep_low = endpoint.trim().to_ascii_lowercase();
    let path_part = ep_low.split(['?', '#']).next().unwrap_or(&ep_low).trim_end_matches('/');
    (ep_low.contains("generativelanguage.googleapis.com")
        && (path_part.ends_with("/openai") || path_part.contains("/openai/")))
        || canonicalize_provider_id(provider_name) == "gemini"
}

fn is_official_alibaba_host(endpoint: &str) -> bool {
    let ep_low = endpoint.trim().to_ascii_lowercase();
    ep_low.is_empty()
        || ep_low.contains("aliyuncs.com")
        || ep_low.contains("alibabacloud.com")
        || ep_low.contains("dashscope.com")
}

/// Applies vendor-aware Chat Completions reasoning controls (`Codewhale reasoning_effort.rs` & `deepseek_effort.rs` parity).
pub(crate) fn apply_chat_completions_reasoning_controls(
    payload: &mut Value,
    provider_name: &str,
    endpoint: &str,
    model: &str,
    raw_effort: Option<&str>,
    has_tools: bool,
) {
    let canon = canonicalize_provider_id(provider_name);
    let model_low = model.trim().to_ascii_lowercase();
    let ep_low = endpoint.trim().to_ascii_lowercase();
    let google_openai = is_google_openai_compat_route(&canon, endpoint);

    let Some(raw) = raw_effort.map(str::trim).filter(|s| !s.is_empty()) else {
        if (canon == "openai" || canon.is_empty())
            && has_tools
            && (model_low.contains("terra") || model_low.starts_with("gpt-6") || model_low.starts_with("gpt-5"))
        {
            payload["reasoning_effort"] = json!("none");
        }
        return;
    };

    let eff = raw.to_ascii_lowercase().replace(['-', '_'], "");
    if matches!(eff.as_str(), "default" | "auto" | "clear") {
        if (canon == "openai" || canon.is_empty())
            && has_tools
            && (model_low.contains("terra") || model_low.starts_with("gpt-6") || model_low.starts_with("gpt-5"))
        {
            payload["reasoning_effort"] = json!("none");
        }
        return;
    }

    let is_off = matches!(eff.as_str(), "off" | "none" | "disable" | "disabled" | "false");
    let is_max = matches!(eff.as_str(), "xhigh" | "extrahigh" | "max" | "ultra" | "highest" | "ultracode");
    let is_high = matches!(eff.as_str(), "high" | "deep");
    let is_med = matches!(eff.as_str(), "medium" | "med" | "mid" | "balanced");
    let is_min = eff == "minimal";

    // 1. Gateways & providers without a documented Chat Completions reasoning dialect: omit undocumented fields.
    if matches!(
        canon.as_str(),
        "telecomjs"
            | "edenai"
            | "zenmux"
            | "csdn"
            | "codewhale"
            | "wanjie-ark"
            | "qianfan"
            | "sakana"
            | "longcat"
            | "opencode"
            | "opencode-go"
            | "concentrate"
            | "minimax"
    ) {
        return;
    }

    // 2. Google OpenAI-compatible `/v1beta/openai`
    if google_openai {
        if is_off {
            let off_effort = if model_low.contains("2.5-flash") { "none" } else { "minimal" };
            payload["reasoning_effort"] = json!(off_effort);
        } else {
            let mapped = if is_max || is_high {
                "high"
            } else if is_med {
                "medium"
            } else {
                "low"
            };
            payload["reasoning_effort"] = json!(mapped);
        }
        return;
    }

    // 3. DeepSeek official & custom `deepseek-reasoner` / `deepseek-r1` / `deepseek-v4`
    if canon == "deepseek"
        || (canon == "custom"
            && (model_low.starts_with("deepseek-reasoner")
                || model_low.starts_with("deepseek-r1")
                || model_low.starts_with("deepseek-v4")))
    {
        if is_off {
            payload["thinking"] = json!({ "type": "disabled" });
        } else {
            let ds_effort = if is_max {
                "max"
            } else if is_med || is_high {
                "high"
            } else {
                "low"
            };
            payload["reasoning_effort"] = json!(ds_effort);
            payload["thinking"] = json!({ "type": "enabled" });
        }
        return;
    }

    // 4. Hosted DeepSeek-style cloud gateways (`siliconflow`, `siliconflow-CN`, `sglang`, `volcengine`, `deepinfra`, `atlascloud`)
    if matches!(
        canon.as_str(),
        "siliconflow" | "siliconflow-CN" | "sglang" | "volcengine" | "deepinfra" | "atlascloud"
    ) {
        if is_off {
            payload["thinking"] = json!({ "type": "disabled" });
        } else {
            let cloud_eff = if is_max { "max" } else { "high" };
            payload["reasoning_effort"] = json!(cloud_eff);
            payload["thinking"] = json!({ "type": "enabled" });
        }
        return;
    }

    // 5. Unified-effort multi-vendor routers (`openrouter`, `orcarouter`, `novita`, `together`)
    if matches!(canon.as_str(), "openrouter" | "orcarouter" | "novita" | "together") {
        if is_off {
            payload["thinking"] = json!({ "type": "disabled" });
        } else {
            let unified = if is_max {
                "xhigh"
            } else if is_high {
                "high"
            } else if is_med {
                "medium"
            } else {
                "low"
            };
            payload["reasoning_effort"] = json!(unified);
            payload["thinking"] = json!({ "type": "enabled" });
        }
        return;
    }

    // 6. Toggle-only `thinking: {"type": "enabled"|"disabled"}` vendors (`xiaomi-mimo`, `zai`, `moonshot`)
    if matches!(canon.as_str(), "xiaomi-mimo" | "zai" | "moonshot") || model_low.starts_with("glm-") {
        payload["thinking"] = json!({ "type": if is_off { "disabled" } else { "enabled" } });
        return;
    }

    // 7. StepFun model-specific effort matrix (`step-5-preview`, `step-3.7-flash`, `step-3.5-flash-2603`)
    if canon == "stepfun" {
        if is_off {
            return;
        }
        let step_eff = match model_low.as_str() {
            "step-5-preview" | "step-3.7-flash" => {
                if is_max || is_high {
                    Some("high")
                } else if is_med {
                    Some("medium")
                } else {
                    Some("low")
                }
            }
            "step-3.5-flash-2603" => {
                if is_max || is_high || is_med {
                    Some("high")
                } else {
                    Some("low")
                }
            }
            _ => None,
        };
        if let Some(se) = step_eff {
            payload["reasoning_effort"] = json!(se);
        }
        return;
    }

    // 8. vLLM (`chat_template_kwargs.enable_thinking` + `reasoning_effort`)
    if canon == "vllm" {
        if is_off {
            payload["chat_template_kwargs"] = json!({ "enable_thinking": false });
        } else {
            let v_eff = if is_max || is_high {
                "high"
            } else if is_med {
                "medium"
            } else {
                "low"
            };
            payload["chat_template_kwargs"] = json!({ "enable_thinking": true });
            payload["reasoning_effort"] = json!(v_eff);
        }
        return;
    }

    // 9. NVIDIA NIM (`chat_template_kwargs.thinking`)
    if canon == "nvidia-nim" {
        if is_off {
            payload["chat_template_kwargs"] = json!({ "thinking": false });
        } else {
            let n_eff = if is_max { "max" } else { "high" };
            payload["chat_template_kwargs"] = json!({ "thinking": true, "reasoning_effort": n_eff });
        }
        return;
    }

    // 10. Local Ollama (`think: bool`) vs Ollama Cloud (`reasoning_effort`)
    if canon == "ollama" {
        payload["think"] = json!(!is_off);
        return;
    }
    if canon == "ollama-cloud" {
        let oc_eff = if is_off {
            "none"
        } else if is_max {
            "max"
        } else if is_high {
            "high"
        } else if is_med {
            "medium"
        } else {
            "low"
        };
        payload["reasoning_effort"] = json!(oc_eff);
        return;
    }

    // 11. Alibaba Qwen / Model Studio (`enable_thinking` + thinking-only exclusions)
    if matches!(canon.as_str(), "qwen" | "modelstudio-token-plan" | "modelstudio-coding-plan")
        && is_official_alibaba_host(&ep_low)
    {
        if matches!(
            model_low.as_str(),
            "qwen3.8-max" | "qwen3.8-max-preview" | "kimi-k2.7-code" | "kimi-k2.5"
        ) {
            return;
        }
        if is_off {
            payload["enable_thinking"] = json!(false);
        } else {
            payload["enable_thinking"] = json!(true);
            if model_low.starts_with("deepseek-v4") || model_low.starts_with("glm-5") {
                payload["reasoning_effort"] = json!(if is_max { "max" } else { "high" });
            }
        }
        return;
    }

    // 12. Arcee / Hugging Face / ModelScope (`minimal|low|medium|high`, omit on `off`)
    if matches!(canon.as_str(), "arcee" | "huggingface" | "modelscope") {
        if !is_off {
            let mapped = if is_max || is_high {
                "high"
            } else if is_med {
                "medium"
            } else if is_min {
                "minimal"
            } else {
                "low"
            };
            payload["reasoning_effort"] = json!(mapped);
        }
        return;
    }

    // 13. Fireworks (`high|max`, omit on `off`)
    if canon == "fireworks" {
        if !is_off {
            payload["reasoning_effort"] = json!(if is_max { "max" } else { "high" });
        }
        return;
    }

    // 14. Meta Model API (`muse-spark*`: `none|minimal|low|medium|high|xhigh|ultra`)
    if canon == "meta" {
        let meta_eff = if is_off {
            "none"
        } else if matches!(eff.as_str(), "ultra" | "max" | "highest" | "ultracode") {
            "ultra"
        } else if is_max {
            "xhigh"
        } else if is_high {
            "high"
        } else if is_med {
            "medium"
        } else if is_min {
            "minimal"
        } else {
            "low"
        };
        payload["reasoning_effort"] = json!(meta_eff);
        return;
    }

    // 15. Mistral (`api.mistral.ai` adjustable `mistral-medium*`/`mistral-small*`)
    if canon == "mistral" {
        if is_off {
            payload["reasoning_effort"] = json!("none");
        } else if is_max || is_high {
            payload["reasoning_effort"] = json!("high");
        }
        return;
    }

    // 16. xAI Grok (`grok-4.3` omits; `grok-3-mini*` -> `low|high`; `grok-4.5` caps at `high`; `grok-4.6+` supports `xhigh`)
    if canon == "xai" || model_low.starts_with("grok") {
        if is_off || model_low == "grok-4.3" {
            return;
        }
        let grok_eff = if model_low.starts_with("grok-3-mini") {
            if is_max || is_high { "high" } else { "low" }
        } else if model_low.starts_with("grok-4.5") {
            if is_max || is_high {
                "high"
            } else if is_med {
                "medium"
            } else {
                "low"
            }
        } else if is_max {
            "xhigh"
        } else if is_high {
            "high"
        } else if is_med {
            "medium"
        } else {
            "low"
        };
        payload["reasoning_effort"] = json!(grok_eff);
        return;
    }

    // 17. Standard OpenAI & compatible Chat Completions models
    let is_o_series = model_low.starts_with("o1") || model_low.starts_with("o3") || model_low.starts_with("o4");
    if is_off {
        if model_low.starts_with("gpt-5")
            || model_low.starts_with("gpt-6")
            || model_low.contains("terra")
            || model_low.contains("sol")
            || model_low.contains("luna")
        {
            payload["reasoning_effort"] = json!("none");
        }
    } else {
        let mapped = if is_o_series && is_max {
            "high"
        } else if is_max {
            "xhigh"
        } else if is_high {
            "high"
        } else if is_med {
            "medium"
        } else {
            "low"
        };
        payload["reasoning_effort"] = json!(mapped);
    }
}

/// Applies Responses API (`/v1/responses` & DeepSeek `/responses`) profile-specific shaping.
pub(crate) fn apply_responses_reasoning_controls(payload: &mut Value, provider_name: &str, raw_effort: Option<&str>) {
    let profile = lookup_provider_descriptor(provider_name)
        .and_then(|d| d.responses_profile)
        .unwrap_or("standard");

    if profile == "deepseek_stateless_plain_reasoning" {
        if let Some(obj) = payload.as_object_mut() {
            obj.remove("store");
        }
    }

    let Some(raw) = raw_effort.map(str::trim).filter(|s| !s.is_empty()) else {
        return;
    };
    let eff = raw.to_ascii_lowercase().replace(['-', '_'], "");
    if matches!(eff.as_str(), "default" | "auto" | "clear") {
        return;
    }
    let is_off = matches!(eff.as_str(), "off" | "none" | "disable" | "disabled" | "false");
    let is_max = matches!(eff.as_str(), "xhigh" | "extrahigh" | "max" | "ultra" | "highest" | "ultracode");
    let is_high = matches!(eff.as_str(), "high" | "deep");
    let is_med = matches!(eff.as_str(), "medium" | "med" | "mid" | "balanced");

    match profile {
        "deepseek_stateless_plain_reasoning" => {
            let ds_eff = if is_off {
                "none"
            } else if is_max {
                "max"
            } else if is_high || is_med {
                "high"
            } else {
                "low"
            };
            payload["reasoning"] = json!({ "effort": ds_eff });
        }
        "concentrate" => {
            if !is_off {
                let mapped = if is_max {
                    "xhigh"
                } else if is_high {
                    "high"
                } else if is_med {
                    "medium"
                } else {
                    "low"
                };
                payload["reasoning"] = json!({ "effort": mapped });
            }
        }
        _ => {
            if is_off {
                if profile != "codex" {
                    payload["reasoning"] = json!({ "effort": "none" });
                }
            } else {
                let mapped = if is_max {
                    "xhigh"
                } else if is_high {
                    "high"
                } else if is_med {
                    "medium"
                } else {
                    "low"
                };
                payload["reasoning"] = json!({ "effort": mapped, "summary": "auto" });
            }
        }
    }
}

