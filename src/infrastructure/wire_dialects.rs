// [LAYER: INFRASTRUCTURE]
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::provider_registry::lookup_provider_descriptor;
use crate::domain::types::tick_result::ProviderRequest;
use crate::infrastructure::reasoning_shaper::{apply_chat_completions_reasoning_controls, apply_responses_reasoning_controls, is_google_openai_compat_route};
use crate::plumbing::schema_and_sse::{sanitize_tool_schema, sanitize_tool_schema_for_gemini, sanitize_tool_schema_for_responses};
use serde_json::{json, Value};

const SYNTHETIC_TOOL_INTERRUPT_NOTE: &str = "[interrupted: missing tool_result synthesized for protocol continuity]";

/// Repairs bidirectional tool-call / tool-result protocol continuity across all 4 wire dialects:
/// 1. Synthesizes missing `Role::Tool` outputs for dangling `Assistant.tool_calls` (after cancellation/failover).
/// 2. Converts orphaned `Role::Tool` outputs (after `/rewind` truncation or compaction) into `Role::User` context.
/// 3. Defers mid-round `Role::System` notes until pending `tool_calls` resolve so `tool` messages remain contiguous.
pub(crate) fn normalize_transcript_tool_continuity(raw: &[SessionMessage]) -> Vec<SessionMessage> {
    let mut out = Vec::with_capacity(raw.len() + 2);
    let mut pending_calls: Vec<(String, String)> = Vec::new();
    let mut deferred_system: Vec<SessionMessage> = Vec::new();

    let flush_dangling = |out: &mut Vec<SessionMessage>,
                          pending: &mut Vec<(String, String)>,
                          deferred_sys: &mut Vec<SessionMessage>| {
        for (id, name) in pending.drain(..) {
            out.push(SessionMessage::tool_result(id, name, SYNTHETIC_TOOL_INTERRUPT_NOTE));
        }
        out.append(deferred_sys);
    };

    for msg in raw {
        match msg.role {
            Role::System => {
                if !pending_calls.is_empty() {
                    deferred_system.push(msg.clone());
                } else {
                    out.push(msg.clone());
                }
            }
            Role::Tool => {
                let matched_pos = if let Some(cid) = msg.tool_call_id.as_deref().filter(|s| !s.is_empty()) {
                    pending_calls.iter().position(|(pid, _)| pid == cid)
                } else if !pending_calls.is_empty() {
                    Some(0)
                } else {
                    None
                };

                if let Some(pos) = matched_pos {
                    let (matched_id, matched_name) = pending_calls.remove(pos);
                    let mut normalized = msg.clone();
                    if normalized.tool_call_id.as_deref().unwrap_or("").is_empty() {
                        normalized.tool_call_id = Some(matched_id);
                    }
                    if normalized.name.as_deref().unwrap_or("").is_empty() {
                        normalized.name = Some(matched_name);
                    }
                    out.push(normalized);
                    if pending_calls.is_empty() && !deferred_system.is_empty() {
                        out.append(&mut deferred_system);
                    }
                } else {
                    let label = msg.name.as_deref().filter(|s| !s.is_empty()).unwrap_or("tool");
                    out.push(SessionMessage::new(
                        Role::User,
                        format!("<tool_output_context name=\"{label}\">\n{}\n</tool_output_context>", msg.content),
                    ));
                }
            }
            Role::User => {
                if !pending_calls.is_empty() {
                    flush_dangling(&mut out, &mut pending_calls, &mut deferred_system);
                }
                out.push(msg.clone());
            }
            Role::Assistant => {
                if !pending_calls.is_empty() {
                    flush_dangling(&mut out, &mut pending_calls, &mut deferred_system);
                }
                if let Some(calls) = msg.tool_calls.as_ref().filter(|c| !c.is_empty()) {
                    for (idx, tc) in calls.iter().enumerate() {
                        let cid = if tc.id.trim().is_empty() {
                            format!("call_{}", idx + 1)
                        } else {
                            tc.id.clone()
                        };
                        pending_calls.push((cid, tc.function.name.clone()));
                    }
                }
                out.push(msg.clone());
            }
        }
    }

    if !pending_calls.is_empty() {
        flush_dangling(&mut out, &mut pending_calls, &mut deferred_system);
    } else if !deferred_system.is_empty() {
        out.append(&mut deferred_system);
    }

    out
}

/// Builds an OpenAI Chat Completions (`/v1/chat/completions`) payload with schema sanitization
/// and provider-specific `max_completion_tokens` / `reasoning_effort` / `thinking` shaping.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn build_chat_completions_payload(provider_name: &str, req: &ProviderRequest) -> Value {
    let default_ep = lookup_provider_descriptor(provider_name)
        .map(|d| d.default_endpoint)
        .unwrap_or("");
    build_chat_completions_payload_with_endpoint(provider_name, default_ep, req)
}

pub(crate) fn build_chat_completions_payload_with_endpoint(
    provider_name: &str,
    endpoint: &str,
    req: &ProviderRequest,
) -> Value {
    let normalized_messages = normalize_transcript_tool_continuity(&req.messages);
    let google_openai = is_google_openai_compat_route(provider_name, endpoint);
    let messages: Vec<Value> = normalized_messages
        .iter()
        .map(|m| {
            let mut obj = json!({ "role": m.role.as_str(), "content": m.content });
            if let Some(id) = &m.tool_call_id {
                obj["tool_call_id"] = Value::String(id.clone());
            }
            if let Some(name) = &m.name {
                obj["name"] = Value::String(name.clone());
            }
            if let Some(calls) = m.tool_calls.as_ref().filter(|c| !c.is_empty()) {
                let wire_calls: Vec<Value> = calls
                    .iter()
                    .map(|tc| {
                        let mut tc_obj = json!({
                            "id": tc.id,
                            "type": tc.r#type,
                            "function": {
                                "name": tc.function.name,
                                "arguments": tc.function.arguments,
                            }
                        });
                        if google_openai {
                            if let Some(sig) = tc.thought_signature.as_deref().filter(|s| !s.is_empty()) {
                                tc_obj["extra_content"] = json!({ "google": { "thought_signature": sig } });
                            }
                        }
                        tc_obj
                    })
                    .collect();
                obj["tool_calls"] = Value::Array(wire_calls);
            }
            obj
        })
        .collect();

    let model_lower = req.model.to_ascii_lowercase();
    let prov_lower = provider_name.to_ascii_lowercase();
    let is_xai = prov_lower == "xai" || prov_lower == "x-ai" || model_lower.starts_with("grok");
    let is_o_series = model_lower.starts_with("o1")
        || model_lower.starts_with("o3")
        || model_lower.starts_with("o4");
    let is_reasoning_or_modern = is_o_series
        || model_lower.starts_with("gpt-6")
        || model_lower.starts_with("gpt-5")
        || model_lower.contains("terra")
        || model_lower.contains("sol")
        || model_lower.contains("luna")
        || model_lower.contains("astra")
        || model_lower.contains("reasoning");

    let mut payload = json!({
        "model": req.model,
        "messages": messages,
    });

    if is_reasoning_or_modern {
        payload["max_completion_tokens"] = json!(req.max_tokens);
        if !is_o_series {
            payload["temperature"] = json!(req.temperature);
        }
    } else {
        payload["max_tokens"] = json!(req.max_tokens);
        payload["temperature"] = json!(req.temperature);
    }

    if !req.tools.is_empty() {
        let sanitized_tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| {
                let mut cloned = t.clone();
                if let Some(params) = cloned.get_mut("function").and_then(|f| f.get_mut("parameters")) {
                    if is_xai {
                        sanitize_tool_schema_for_responses(params);
                    } else {
                        sanitize_tool_schema(params);
                    }
                }
                cloned
            })
            .collect();
        payload["tools"] = Value::Array(sanitized_tools);
        payload["tool_choice"] = Value::String("auto".to_string());
    }

    apply_chat_completions_reasoning_controls(
        &mut payload,
        provider_name,
        endpoint,
        &req.model,
        req.reasoning_effort.as_deref(),
        !req.tools.is_empty(),
    );

    payload
}

/// Builds a native OpenAI Responses API (`/v1/responses`) payload (`openai-codex`, `concentrate`).
pub(crate) fn build_openai_responses_payload(provider_name: &str, req: &ProviderRequest) -> Value {
    let normalized_messages = normalize_transcript_tool_continuity(&req.messages);
    let mut leading_system = Vec::new();
    let mut input: Vec<Value> = Vec::new();
    let mut conversation_started = false;

    for m in &normalized_messages {
        match m.role {
            Role::System => {
                if m.content.trim().is_empty() {
                    continue;
                }
                if !conversation_started {
                    leading_system.push(m.content.clone());
                } else {
                    input.push(json!({
                        "type": "message",
                        "role": "user",
                        "content": [{ "type": "input_text", "text": format!("<system_note>\n{}\n</system_note>", m.content) }]
                    }));
                }
            }
            Role::User => {
                conversation_started = true;
                input.push(json!({
                    "type": "message",
                    "role": "user",
                    "content": [{ "type": "input_text", "text": m.content }]
                }));
            }
            Role::Assistant => {
                conversation_started = true;
                if !m.content.trim().is_empty() {
                    input.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": m.content }]
                    }));
                }
                if let Some(calls) = &m.tool_calls {
                    for tc in calls {
                        input.push(json!({
                            "type": "function_call",
                            "call_id": tc.id,
                            "name": tc.function.name,
                            "arguments": tc.function.arguments
                        }));
                    }
                }
            }
            Role::Tool => {
                conversation_started = true;
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": m.tool_call_id.clone().unwrap_or_else(|| "call_1".to_string()),
                    "output": m.content
                }));
            }
        }
    }

    let instructions = if leading_system.is_empty() {
        "You are HEAV3NS-MICRO, a sovereign software engineering agent.".to_string()
    } else {
        leading_system.join("\n\n")
    };

    let profile = lookup_provider_descriptor(provider_name)
        .and_then(|d| d.responses_profile)
        .unwrap_or("standard");

    let mut payload = json!({
        "model": req.model,
        "store": false,
        "instructions": instructions,
        "input": input,
        "max_output_tokens": req.max_tokens,
    });
    if profile != "codex" {
        payload["temperature"] = json!(req.temperature);
    }

    let tools: Vec<Value> = req
        .tools
        .iter()
        .filter_map(|t| {
            let f = t.get("function")?;
            let name = f.get("name")?.as_str()?;
            let mut params = f.get("parameters").cloned().unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
            sanitize_tool_schema_for_responses(&mut params);
            Some(json!({
                "type": "function",
                "name": name,
                "description": f.get("description").and_then(Value::as_str).unwrap_or(""),
                "parameters": params
            }))
        })
        .collect();
    if !tools.is_empty() {
        payload["tools"] = Value::Array(tools);
        payload["tool_choice"] = json!("auto");
        payload["parallel_tool_calls"] = json!(true);
    }

    apply_responses_reasoning_controls(&mut payload, provider_name, req.reasoning_effort.as_deref());

    payload
}

/// Builds an Anthropic `/v1/messages` payload with:
/// 1. Leading `Role::System` in top-level `system` (`cache_control: ephemeral`); mid-transcript `System` projected in-place onto `user`!
/// 2. Consecutive `Role::Tool` coalescing into a single `user` `tool_result` array!
/// 3. Automatic dangling `tool_use` repair (preventing HTTP 400 after cancelled/compacted turns)!
/// 4. Adaptive/budget `thinking` + `output_config.effort` when `reasoning_effort` is configured!
pub(crate) fn build_anthropic_messages_payload(provider_name: &str, endpoint: &str, req: &ProviderRequest) -> Value {
    let normalized_messages = normalize_transcript_tool_continuity(&req.messages);
    let mut system_parts = Vec::new();
    let mut messages: Vec<Value> = Vec::new();
    let mut conversation_started = false;

    for m in &normalized_messages {
        match m.role {
            Role::System if !m.content.trim().is_empty() => {
                if !conversation_started {
                    system_parts.push(m.content.clone());
                } else {
                    let note_block = json!({ "type": "text", "text": format!("<system_note>\n{}\n</system_note>", m.content) });
                    push_or_coalesce_anthropic_user_block(&mut messages, note_block);
                }
            }
            Role::User => {
                conversation_started = true;
                let text_block = json!({ "type": "text", "text": m.content });
                push_or_coalesce_anthropic_user_block(&mut messages, text_block);
            }
            Role::Assistant => {
                conversation_started = true;
                let mut blocks = Vec::new();
                if !m.content.trim().is_empty() {
                    blocks.push(json!({ "type": "text", "text": m.content }));
                }
                if let Some(calls) = &m.tool_calls {
                    for tc in calls {
                        let input: Value = serde_json::from_str(&tc.function.arguments).unwrap_or_else(|_| json!({}));
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.function.name,
                            "input": input
                        }));
                    }
                }
                if !blocks.is_empty() {
                    if messages.is_empty() && m.tool_calls.as_ref().is_some_and(|c| !c.is_empty()) {
                        messages.push(json!({ "role": "user", "content": [{ "type": "text", "text": "[resumed session context]" }] }));
                    }
                    if !messages.is_empty() {
                        push_or_coalesce_anthropic_assistant_blocks(&mut messages, blocks);
                    }
                }
            }
            Role::Tool => {
                conversation_started = true;
                let mut tr_block = json!({
                    "type": "tool_result",
                    "tool_use_id": m.tool_call_id.clone().unwrap_or_else(|| "tool-call".to_string()),
                    "content": m.content
                });
                if m.content == SYNTHETIC_TOOL_INTERRUPT_NOTE {
                    tr_block["is_error"] = Value::Bool(true);
                }
                push_or_coalesce_anthropic_user_block(&mut messages, tr_block);
            }
            _ => {}
        }
    }

    repair_dangling_anthropic_tool_uses(&mut messages);

    let mut tools: Vec<Value> = req
        .tools
        .iter()
        .filter_map(|t| {
            let f = t.get("function")?;
            let mut schema = f.get("parameters").cloned().unwrap_or_else(|| json!({"type": "object", "properties": {}}));
            sanitize_tool_schema(&mut schema);
            Some(json!({
                "name": f.get("name")?.as_str()?,
                "description": f.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                "input_schema": schema,
            }))
        })
        .collect();
    if let Some(last_tool) = tools.last_mut() {
        last_tool["cache_control"] = json!({ "type": "ephemeral" });
    }

    let mut payload = json!({
        "model": req.model,
        "max_tokens": req.max_tokens,
        "temperature": req.temperature,
        "messages": messages
    });
    if !system_parts.is_empty() {
        payload["system"] = json!([{
            "type": "text",
            "text": system_parts.join("\n\n"),
            "cache_control": { "type": "ephemeral" }
        }]);
    }
    if !tools.is_empty() {
        payload["tools"] = Value::Array(tools);
    }

    apply_anthropic_reasoning_shaping(&mut payload, provider_name, endpoint, req);
    payload
}

fn push_or_coalesce_anthropic_user_block(messages: &mut Vec<Value>, block: Value) {
    let is_tool_result = block.get("type").and_then(Value::as_str) == Some("tool_result");
    if let Some(last) = messages.last_mut().filter(|v| v.get("role") == Some(&Value::String("user".into()))) {
        if let Some(arr) = last.get_mut("content").and_then(Value::as_array_mut) {
            if is_tool_result {
                let insert_pos = arr
                    .iter()
                    .position(|b| b.get("type").and_then(Value::as_str) != Some("tool_result"))
                    .unwrap_or(arr.len());
                arr.insert(insert_pos, block);
            } else {
                arr.push(block);
            }
            return;
        }
    }
    messages.push(json!({ "role": "user", "content": [block] }));
}

fn push_or_coalesce_anthropic_assistant_blocks(messages: &mut Vec<Value>, mut blocks: Vec<Value>) {
    if let Some(last) = messages.last_mut().filter(|v| v.get("role") == Some(&Value::String("assistant".into()))) {
        if let Some(arr) = last.get_mut("content").and_then(Value::as_array_mut) {
            arr.append(&mut blocks);
            return;
        }
    }
    messages.push(json!({ "role": "assistant", "content": blocks }));
}

fn repair_dangling_anthropic_tool_uses(messages: &mut Vec<Value>) {
    let mut idx = 0usize;
    while idx < messages.len() {
        let required_ids: Vec<String> = if messages[idx].get("role").and_then(Value::as_str) == Some("assistant") {
            messages[idx]
                .get("content")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
                        .filter_map(|b| b.get("id").and_then(Value::as_str).map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        if !required_ids.is_empty() {
            let next_is_user = messages
                .get(idx + 1)
                .and_then(|m| m.get("role"))
                .and_then(Value::as_str)
                == Some("user");
            if next_is_user {
                if let Some(arr) = messages[idx + 1].get_mut("content").and_then(Value::as_array_mut) {
                    let mut missing_blocks = Vec::new();
                    for req_id in &required_ids {
                        let found = arr.iter().any(|b| {
                            b.get("type").and_then(Value::as_str) == Some("tool_result")
                                && b.get("tool_use_id").and_then(Value::as_str) == Some(req_id.as_str())
                        });
                        if !found {
                            missing_blocks.push(json!({
                                "type": "tool_result",
                                "tool_use_id": req_id,
                                "is_error": true,
                                "content": "[interrupted: missing tool_result synthesized for protocol continuity]"
                            }));
                        }
                    }
                    if !missing_blocks.is_empty() {
                        missing_blocks.append(arr);
                        *arr = missing_blocks;
                    }
                }
            } else {
                let synthetic: Vec<Value> = required_ids
                    .iter()
                    .map(|req_id| {
                        json!({
                            "type": "tool_result",
                            "tool_use_id": req_id,
                            "is_error": true,
                            "content": "[interrupted: missing tool_result synthesized for protocol continuity]"
                        })
                    })
                    .collect();
                messages.insert(idx + 1, json!({ "role": "user", "content": synthetic }));
            }
        }
        idx += 1;
    }
}

fn apply_anthropic_reasoning_shaping(payload: &mut Value, provider_name: &str, endpoint: &str, req: &ProviderRequest) {
    let Some(raw) = req.reasoning_effort.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return;
    };
    let eff = raw.to_ascii_lowercase();
    let model_low = req.model.to_ascii_lowercase();
    let prov_low = provider_name.to_ascii_lowercase();
    let ep_low = endpoint.to_ascii_lowercase();

    if matches!(eff.as_str(), "off" | "none" | "disabled") {
        if prov_low.contains("deepseek") || prov_low.contains("minimax") || prov_low.contains("qwen") {
            payload["thinking"] = json!({ "type": "disabled" });
        }
        return;
    }

    let is_adaptive_claude = (ep_low.contains("api.anthropic.com") || prov_low == "anthropic" || prov_low.contains("claude"))
        && (model_low.contains("4.6")
            || model_low.contains("4.7")
            || model_low.contains("4.8")
            || model_low.contains("-5")
            || model_low.contains("fable"));
    if is_adaptive_claude {
        let mapped = match eff.as_str() {
            "low" | "minimal" => "low",
            "medium" | "med" => "medium",
            "xhigh" | "max" => "max",
            _ => "high",
        };
        payload["thinking"] = json!({ "type": "adaptive" });
        payload["output_config"] = json!({ "effort": mapped });
        if let Some(obj) = payload.as_object_mut() {
            obj.remove("temperature");
        }
    } else if req.max_tokens >= 2048 {
        let desired: usize = match eff.as_str() {
            "low" | "minimal" => 1024,
            "medium" | "med" => 4096,
            "xhigh" | "max" => 16_384,
            _ => 8192,
        };
        let budget = desired.min(req.max_tokens.saturating_sub(512)).max(1024);
        payload["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
        if let Some(obj) = payload.as_object_mut() {
            obj.remove("temperature");
        }
    }
}

/// Builds a Google Gemini (`:generateContent`) payload with sanitized schemas, `thoughtSignature`
/// preservation, in-place mid-transcript `<system_note>` positioning, and `thinkingConfig`.
pub(crate) fn build_google_gemini_payload(req: &ProviderRequest) -> Value {
    let normalized_messages = normalize_transcript_tool_continuity(&req.messages);
    let mut system_parts = Vec::new();
    let mut contents: Vec<Value> = Vec::new();
    let mut conversation_started = false;
    let push_gemini_parts = |contents: &mut Vec<Value>, role: &str, mut parts: Vec<Value>| {
        if parts.is_empty() {
            return;
        }
        if let Some(last) = contents.last_mut().filter(|v| v.get("role").and_then(Value::as_str) == Some(role)) {
            if let Some(arr) = last.get_mut("parts").and_then(Value::as_array_mut) {
                arr.append(&mut parts);
                return;
            }
        }
        contents.push(json!({ "role": role, "parts": parts }));
    };

    for m in &normalized_messages {
        match m.role {
            Role::System if !m.content.trim().is_empty() => {
                if !conversation_started {
                    system_parts.push(json!({ "text": m.content }));
                } else {
                    push_gemini_parts(
                        &mut contents,
                        "user",
                        vec![json!({ "text": format!("<system_note>\n{}\n</system_note>", m.content) })],
                    );
                }
            }
            Role::User => {
                conversation_started = true;
                push_gemini_parts(&mut contents, "user", vec![json!({ "text": m.content })]);
            }
            Role::Assistant => {
                conversation_started = true;
                let mut parts = Vec::new();
                if !m.content.is_empty() {
                    parts.push(json!({ "text": m.content }));
                }
                if let Some(calls) = &m.tool_calls {
                    for tc in calls {
                        let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or_else(|_| json!({}));
                        let mut fc_part = json!({ "functionCall": { "name": tc.function.name, "args": args } });
                        if let Some(sig) = tc.thought_signature.as_deref().filter(|s| !s.is_empty()) {
                            fc_part["thoughtSignature"] = Value::String(sig.to_string());
                        }
                        parts.push(fc_part);
                    }
                }
                push_gemini_parts(&mut contents, "model", parts);
            }
            Role::Tool => {
                conversation_started = true;
                let fr_part = json!({
                    "functionResponse": {
                        "name": m.name.clone().unwrap_or_else(|| "tool".to_string()),
                        "response": { "result": m.content }
                    }
                });
                push_gemini_parts(&mut contents, "user", vec![fr_part]);
            }
            _ => {}
        }
    }

    let decls: Vec<Value> = req
        .tools
        .iter()
        .filter_map(|t| {
            let mut f = t.get("function")?.clone();
            if let Some(params) = f.get_mut("parameters") {
                sanitize_tool_schema_for_gemini(params);
            }
            Some(f)
        })
        .collect();

    let mut gen_cfg = json!({
        "maxOutputTokens": req.max_tokens,
        "temperature": req.temperature
    });
    if let Some(raw_eff) = req.reasoning_effort.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let eff = raw_eff.to_ascii_lowercase();
        if matches!(eff.as_str(), "off" | "none" | "disabled") {
            let model_low = req.model.to_ascii_lowercase();
            if !model_low.contains("2.5-pro") && !model_low.starts_with("gemini-3") {
                gen_cfg["thinkingConfig"] = json!({ "thinkingBudget": 0 });
            }
        } else {
            let level = match eff.as_str() {
                "low" | "minimal" => "LOW",
                "medium" | "med" => "MEDIUM",
                _ => "HIGH",
            };
            gen_cfg["thinkingConfig"] = json!({ "thinkingLevel": level });
        }
    }

    let mut payload = json!({
        "contents": contents,
        "generationConfig": gen_cfg
    });
    if !system_parts.is_empty() {
        payload["systemInstruction"] = json!({ "parts": system_parts });
    }
    if !decls.is_empty() {
        payload["tools"] = json!([{ "functionDeclarations": decls }]);
    }
    payload
}
