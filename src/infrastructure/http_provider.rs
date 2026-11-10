// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::{BoxFuture, ILlmProvider};
use crate::domain::types::messages::{Role, ToolCall, ToolCallFunction};
use crate::domain::types::runtime::CancellationToken;
use crate::domain::types::tick_result::{finalize_provider_response, is_retryable_http_status, ProviderError, ProviderRequest, ProviderResponse, ProviderTokenUsage};
use crate::plumbing::json_repair::{accumulate_streaming_tool_name, compute_streaming_arguments_delta};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Duration;

/// Fast non-blocking probe (`150ms` timeout) for local Ollama chat models (`Codewhale zero-config discovery`).
pub async fn probe_local_ollama_model() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(150))
        .build()
        .ok()?;
    let resp = client.get("http://127.0.0.1:11434/api/tags").send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: Value = resp.json().await.ok()?;
    body.get("models")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(|m| m.get("name"))
        .and_then(Value::as_str)
        .map(String::from)
}

/// Parses SSE (`data: ...`) streams with cumulative delta deduplication and token usage capture.
pub fn parse_sse_stream(raw_sse: &str, provider_label: &str) -> Result<ProviderResponse, ProviderError> {
    let mut text_acc = String::new();
    let mut tool_map: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
    let mut latest_usage: Option<ProviderTokenUsage> = None;

    for line in raw_sse.lines() {
        let trimmed = line.trim();
        let Some(data) = trimmed.strip_prefix("data:").map(str::trim) else { continue };
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else { continue };
        if let Some(u) = extract_openai_usage(&chunk) {
            latest_usage = Some(u);
        }
        if let Some(choice) = chunk.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first()) {
            let delta = choice.get("delta").or_else(|| choice.get("message")).unwrap_or(choice);
            if let Some(txt) = delta.get("content").and_then(|v| v.as_str()) {
                text_acc.push_str(txt);
            }
            if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                for (i, tc) in tcs.iter().enumerate() {
                    let idx = tc.get("index").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(i);
                    let entry = tool_map
                        .entry(idx)
                        .or_insert_with(|| (format!("call_{}", idx + 1), String::new(), String::new()));
                    if let Some(id) = tc.get("id").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                        entry.0 = id.to_string();
                    }
                    if let Some(func) = tc.get("function") {
                        if let Some(name_chunk) = func.get("name").and_then(|v| v.as_str()) {
                            entry.1 = accumulate_streaming_tool_name(&entry.1, name_chunk);
                        }
                        if let Some(arg_chunk) = func.get("arguments").and_then(|v| v.as_str()) {
                            let (_, next_acc) = compute_streaming_arguments_delta(&entry.2, arg_chunk);
                            entry.2 = next_acc;
                        }
                    }
                }
            }
        }
    }

    let tool_calls = tool_map
        .into_values()
        .filter(|(_, name, _)| !name.is_empty())
        .map(|(id, name, args)| ToolCall {
            id,
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name,
                arguments: if args.is_empty() { "{}".to_string() } else { args },
            },
        })
        .collect();

    let mut resp = finalize_provider_response(Some(text_acc), tool_calls, provider_label);
    resp.usage = latest_usage;
    Ok(resp)
}

fn extract_openai_usage(body: &Value) -> Option<ProviderTokenUsage> {
    let u = body.get("usage")?;
    let prompt_tokens = u.get("prompt_tokens").and_then(Value::as_u64)? as usize;
    let completion_tokens = u.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
    let cached_prompt_tokens = u
        .get("prompt_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .or_else(|| u.get("prompt_cache_hit_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    Some(ProviderTokenUsage {
        prompt_tokens,
        completion_tokens,
        cached_prompt_tokens,
        is_exact: true,
    })
}

fn parse_retry_after_header(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    if let Some(ms_val) = headers.get("retry-after-ms").and_then(|v| v.to_str().ok()) {
        if let Ok(ms) = ms_val.trim().parse::<u64>() {
            return Some(ms);
        }
    }
    if let Some(sec_val) = headers.get("retry-after").and_then(|v| v.to_str().ok()) {
        if let Ok(secs) = sec_val.trim().parse::<f64>() {
            if secs.is_finite() && secs >= 0.0 {
                return Some((secs * 1000.0) as u64);
            }
        }
    }
    None
}

/// Multi-protocol HTTP LLM adapter supporting OpenAI, DeepSeek, OpenRouter, Ollama, Anthropic, and Gemini wire formats.
pub struct HttpProvider {
    name: String,
    endpoint: String,
    api_key: Option<String>,
    timeout_ms: u64,
    client: reqwest::Client,
}

impl HttpProvider {
    pub extern "Rust" fn new(
        name: impl Into<String>,
        endpoint: impl Into<String>,
        api_key: Option<String>,
        timeout_ms: u64,
    ) -> Self {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(8)
            .tcp_nodelay(true)
            .build()
            .unwrap_or_default();
        Self {
            name: name.into(),
            endpoint: endpoint.into(),
            api_key,
            timeout_ms: timeout_ms.max(1_000),
            client,
        }
    }

    fn resolve_url(&self, model: &str) -> String {
        let trimmed = self.endpoint.trim_end_matches('/');
        if self.name.eq_ignore_ascii_case("anthropic") {
            if trimmed.ends_with("/messages") {
                trimmed.to_string()
            } else if trimmed.ends_with("/v1") {
                format!("{trimmed}/messages")
            } else {
                format!("{trimmed}/v1/messages")
            }
        } else if self.name.eq_ignore_ascii_case("gemini") || self.name.eq_ignore_ascii_case("google") {
            if trimmed.contains(":generateContent") {
                trimmed.to_string()
            } else {
                format!("{trimmed}/models/{model}:generateContent")
            }
        } else if trimmed.ends_with("/chat/completions") {
            trimmed.to_string()
        } else {
            format!("{trimmed}/chat/completions")
        }
    }

    fn build_openai_payload(&self, req: &ProviderRequest) -> Value {
        let messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| {
                let mut obj = json!({ "role": m.role.as_str(), "content": m.content });
                if let Some(id) = &m.tool_call_id {
                    obj["tool_call_id"] = Value::String(id.clone());
                }
                if let Some(name) = &m.name {
                    obj["name"] = Value::String(name.clone());
                }
                if let Some(calls) = &m.tool_calls.as_ref().filter(|c| !c.is_empty()) {
                    obj["tool_calls"] = serde_json::to_value(calls).unwrap_or(Value::Null);
                }
                obj
            })
            .collect();
        let model_lower = req.model.to_ascii_lowercase();
        let is_o_series = model_lower.starts_with("o1")
            || model_lower.starts_with("o3")
            || model_lower.starts_with("o4");
        let is_reasoning_or_modern = is_o_series
            || model_lower.starts_with("gpt-5")
            || model_lower.contains("terra")
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
            payload["tools"] = Value::Array(req.tools.clone());
            payload["tool_choice"] = Value::String("auto".to_string());
            if model_lower.contains("terra") || model_lower.starts_with("gpt-5") {
                payload["reasoning_effort"] = Value::String("none".to_string());
            }
        }
        payload
    }

    /// Builds an Anthropic `/v1/messages` payload with:
    /// 1. Consecutive `Role::Tool` coalescing into a single `role: "user"` `tool_result` block array!
    /// 2. Strict `user`-first message alternation!
    /// 3. Ephemeral `cache_control` breakpoints on the frozen system prompt and last tool schema!
    pub(crate) fn build_anthropic_payload(&self, req: &ProviderRequest) -> Value {
        let mut system_parts = Vec::new();
        let mut messages: Vec<Value> = Vec::new();
        for m in &req.messages {
            match m.role {
                Role::System if !m.content.trim().is_empty() => system_parts.push(m.content.clone()),
                Role::User => {
                    let text_block = json!({ "type": "text", "text": m.content });
                    if let Some(last) = messages.last_mut().filter(|v| v.get("role") == Some(&Value::String("user".into()))) {
                        if let Some(arr) = last.get_mut("content").and_then(Value::as_array_mut) {
                            arr.push(text_block);
                            continue;
                        }
                    }
                    messages.push(json!({ "role": "user", "content": [text_block] }));
                }
                Role::Assistant => {
                    let mut blocks = Vec::new();
                    if !m.content.trim().is_empty() {
                        blocks.push(json!({ "type": "text", "text": m.content }));
                    }
                    if let Some(calls) = &m.tool_calls {
                        for tc in calls {
                            let input: Value =
                                serde_json::from_str(&tc.function.arguments).unwrap_or_else(|_| json!({}));
                            blocks.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.function.name,
                                "input": input
                            }));
                        }
                    }
                    if !blocks.is_empty() && !messages.is_empty() {
                        messages.push(json!({ "role": "assistant", "content": blocks }));
                    }
                }
                Role::Tool => {
                    let tr_block = json!({
                        "type": "tool_result",
                        "tool_use_id": m.tool_call_id.clone().unwrap_or_else(|| "tool-call".to_string()),
                        "content": m.content
                    });
                    if let Some(last) = messages.last_mut().filter(|v| v.get("role") == Some(&Value::String("user".into()))) {
                        if let Some(arr) = last.get_mut("content").and_then(Value::as_array_mut) {
                            arr.push(tr_block);
                            continue;
                        }
                    }
                    messages.push(json!({ "role": "user", "content": [tr_block] }));
                }
                _ => {}
            }
        }

        let mut tools: Vec<Value> = req
            .tools
            .iter()
            .filter_map(|t| {
                let f = t.get("function")?;
                Some(json!({
                    "name": f.get("name")?.as_str()?,
                    "description": f.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                    "input_schema": f.get("parameters").cloned().unwrap_or_else(|| json!({"type": "object"})),
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
        payload
    }

    /// Builds a Gemini `:generateContent` payload coalescing consecutive `Role::Tool` `functionResponse` parts.
    pub(crate) fn build_gemini_payload(&self, req: &ProviderRequest) -> Value {
        let mut system_parts = Vec::new();
        let mut contents: Vec<Value> = Vec::new();
        for m in &req.messages {
            match m.role {
                Role::System if !m.content.trim().is_empty() => system_parts.push(json!({ "text": m.content })),
                Role::User => contents.push(json!({ "role": "user", "parts": [{ "text": m.content }] })),
                Role::Assistant => {
                    let mut parts = Vec::new();
                    if !m.content.is_empty() {
                        parts.push(json!({ "text": m.content }));
                    }
                    if let Some(calls) = &m.tool_calls {
                        for tc in calls {
                            let args: Value =
                                serde_json::from_str(&tc.function.arguments).unwrap_or_else(|_| json!({}));
                            parts.push(json!({ "functionCall": { "name": tc.function.name, "args": args } }));
                        }
                    }
                    if !parts.is_empty() {
                        contents.push(json!({ "role": "model", "parts": parts }));
                    }
                }
                Role::Tool => {
                    let fr_part = json!({
                        "functionResponse": {
                            "name": m.name.clone().unwrap_or_else(|| "tool".to_string()),
                            "response": { "result": m.content }
                        }
                    });
                    if let Some(last) = contents.last_mut().filter(|v| v.get("role") == Some(&Value::String("user".into()))) {
                        if let Some(arr) = last.get_mut("parts").and_then(Value::as_array_mut) {
                            let has_fr = arr.iter().any(|p| p.get("functionResponse").is_some());
                            if has_fr {
                                arr.push(fr_part);
                                continue;
                            }
                        }
                    }
                    contents.push(json!({ "role": "user", "parts": [fr_part] }));
                }
                _ => {}
            }
        }
        let decls: Vec<Value> = req.tools.iter().filter_map(|t| t.get("function").cloned()).collect();
        let mut payload = json!({
            "contents": contents,
            "generationConfig": { "maxOutputTokens": req.max_tokens, "temperature": req.temperature }
        });
        if !system_parts.is_empty() {
            payload["systemInstruction"] = json!({ "parts": system_parts });
        }
        if !decls.is_empty() {
            payload["tools"] = json!([{ "functionDeclarations": decls }]);
        }
        payload
    }

    fn parse_openai_response(body: &Value) -> Result<ProviderResponse, ProviderError> {
        let choice = body
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| ProviderError::Transport("Missing choices[0] in OpenAI response".to_string()))?;
        let msg = choice.get("message").unwrap_or(choice);
        let content = msg.get("content").and_then(|c| c.as_str()).map(String::from);
        let mut tool_calls = Vec::new();
        if let Some(raw_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
            for (idx, tc) in raw_calls.iter().enumerate() {
                let id = tc
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| format!("call_{}", idx + 1));
                if let Some(func) = tc.get("function") {
                    let name = func.get("name").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
                    let raw_args = match func.get("arguments") {
                        Some(Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                        None => "{}".to_string(),
                    };
                    if !name.is_empty() {
                        tool_calls.push(ToolCall {
                            id,
                            r#type: "function".to_string(),
                            function: ToolCallFunction {
                                name,
                                arguments: raw_args,
                            },
                        });
                    }
                }
            }
        }
        let mut resp = finalize_provider_response(content, tool_calls, "openai");
        resp.usage = extract_openai_usage(body);
        Ok(resp)
    }

    fn parse_anthropic_response(body: &Value) -> Result<ProviderResponse, ProviderError> {
        let blocks = body
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| ProviderError::Transport("Missing content array in Anthropic response".to_string()))?;
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        for (idx, block) in blocks.iter().enumerate() {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(t) = block.get("text").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                        text_parts.push(t.to_string());
                    }
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .unwrap_or_else(|| format!("toolu_{}", idx + 1));
                    let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let input = block.get("input").map(|v| v.to_string()).unwrap_or_else(|| "{}".to_string());
                    if !name.is_empty() {
                        tool_calls.push(ToolCall {
                            id,
                            r#type: "function".to_string(),
                            function: ToolCallFunction {
                                name,
                                arguments: input,
                            },
                        });
                    }
                }
                _ => {}
            }
        }
        let mut resp = finalize_provider_response(Some(text_parts.join("\n")), tool_calls, "anthropic");
        if let Some(u) = body.get("usage") {
            let uncached = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
            let cache_read = u.get("cache_read_input_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
            let cache_create = u.get("cache_creation_input_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
            let out_tok = u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
            resp.usage = Some(ProviderTokenUsage {
                prompt_tokens: uncached + cache_read + cache_create,
                completion_tokens: out_tok,
                cached_prompt_tokens: cache_read,
                is_exact: true,
            });
        }
        Ok(resp)
    }

    fn parse_gemini_response(body: &Value) -> Result<ProviderResponse, ProviderError> {
        let parts = body
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
            .ok_or_else(|| {
                ProviderError::Transport("Missing candidates[0].content.parts in Gemini response".to_string())
            })?;
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        for (idx, part) in parts.iter().enumerate() {
            if let Some(t) = part.get("text").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                text_parts.push(t.to_string());
            }
            if let Some(fc) = part.get("functionCall") {
                let name = fc.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let args = fc.get("args").map(|v| v.to_string()).unwrap_or_else(|| "{}".to_string());
                if !name.is_empty() {
                    tool_calls.push(ToolCall {
                        id: format!("gemini_call_{}", idx + 1),
                        r#type: "function".to_string(),
                        function: ToolCallFunction { name, arguments: args },
                    });
                }
            }
        }
        let mut resp = finalize_provider_response(Some(text_parts.join("\n")), tool_calls, "gemini");
        if let Some(um) = body.get("usageMetadata") {
            let prompt_tokens = um.get("promptTokenCount").and_then(Value::as_u64).unwrap_or(0) as usize;
            let completion_tokens = um.get("candidatesTokenCount").and_then(Value::as_u64).unwrap_or(0) as usize;
            let cached_prompt_tokens = um.get("cachedContentTokenCount").and_then(Value::as_u64).unwrap_or(0) as usize;
            resp.usage = Some(ProviderTokenUsage {
                prompt_tokens,
                completion_tokens,
                cached_prompt_tokens,
                is_exact: true,
            });
        }
        Ok(resp)
    }
}

impl ILlmProvider for HttpProvider {
    fn provider_name(&self) -> &str {
        &self.name
    }

    fn complete<'a>(
        &'a self,
        req: ProviderRequest,
        cancel: Option<CancellationToken>,
    ) -> BoxFuture<'a, Result<ProviderResponse, ProviderError>> {
        Box::pin(async move {
            if cancel.as_ref().map(|t| t.is_cancelled()).unwrap_or(false) {
                return Err(ProviderError::Cancelled);
            }
            let is_anthropic = self.name.eq_ignore_ascii_case("anthropic");
            let is_gemini = self.name.eq_ignore_ascii_case("gemini") || self.name.eq_ignore_ascii_case("google");
            let url = self.resolve_url(&req.model);
            let payload = if is_anthropic {
                self.build_anthropic_payload(&req)
            } else if is_gemini {
                self.build_gemini_payload(&req)
            } else {
                self.build_openai_payload(&req)
            };

            let mut builder = self.client.post(&url).header("content-type", "application/json");
            if let Some(ref key) = self.api_key {
                if is_anthropic {
                    builder = builder.header("x-api-key", key).header("anthropic-version", "2023-06-01");
                } else if is_gemini {
                    builder = builder.header("x-goog-api-key", key);
                } else {
                    builder = builder.bearer_auth(key);
                }
            }

            let http_fut = async {
                let resp = builder
                    .json(&payload)
                    .send()
                    .await
                    .map_err(|e| ProviderError::Transport(e.to_string()))?;
                let status = resp.status().as_u16();
                let retry_after_ms = parse_retry_after_header(resp.headers());
                let text = resp.text().await.map_err(|e| ProviderError::Transport(e.to_string()))?;
                if !(200..300).contains(&status) {
                    if !is_anthropic && !is_gemini && status == 400 {
                        let mut needs_retry = false;
                        let mut healed_payload = payload.clone();

                        if text.contains("max_completion_tokens") && healed_payload.get("max_tokens").is_some() {
                            if let Some(tokens) = healed_payload.as_object_mut().and_then(|o| o.remove("max_tokens")) {
                                healed_payload["max_completion_tokens"] = tokens;
                                needs_retry = true;
                            }
                        }
                        if text.contains("reasoning_effort") && text.contains("set reasoning_effort to 'none'") {
                            healed_payload["reasoning_effort"] = Value::String("none".to_string());
                            needs_retry = true;
                        }
                        if text.contains("Unsupported parameter: 'temperature'") {
                            if let Some(obj) = healed_payload.as_object_mut() {
                                obj.remove("temperature");
                                needs_retry = true;
                            }
                        }

                        if needs_retry {
                            let mut retry_builder = self.client.post(&url).header("content-type", "application/json");
                            if let Some(ref key) = self.api_key {
                                retry_builder = retry_builder.bearer_auth(key);
                            }
                            if let Ok(retry_resp) = retry_builder.json(&healed_payload).send().await {
                                let retry_status = retry_resp.status().as_u16();
                                let retry_text = retry_resp.text().await.map_err(|e| ProviderError::Transport(e.to_string()))?;
                                if (200..300).contains(&retry_status) {
                                    if retry_text.trim_start().starts_with("data:") {
                                        return parse_sse_stream(&retry_text, &self.name);
                                    }
                                    let json_body: Value = serde_json::from_str(&retry_text)
                                        .map_err(|e| ProviderError::Transport(format!("Invalid JSON from provider: {e}")))?;
                                    return Self::parse_openai_response(&json_body);
                                }
                            }
                        }
                    }

                    return Err(ProviderError::HttpStatus {
                        status,
                        body: text,
                        retryable: is_retryable_http_status(status),
                        retry_after_ms,
                    });
                }
                if text.trim_start().starts_with("data:") {
                    return parse_sse_stream(&text, &self.name);
                }
                let json_body: Value = serde_json::from_str(&text)
                    .map_err(|e| ProviderError::Transport(format!("Invalid JSON from provider: {e}")))?;
                if is_anthropic {
                    Self::parse_anthropic_response(&json_body)
                } else if is_gemini {
                    Self::parse_gemini_response(&json_body)
                } else {
                    Self::parse_openai_response(&json_body)
                }
            };

            let timeout_dur = Duration::from_millis(self.timeout_ms);
            if let Some(token) = cancel {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => Err(ProviderError::Cancelled),
                    res = tokio::time::timeout(timeout_dur, http_fut) => res.unwrap_or(Err(ProviderError::Timeout)),
                }
            } else {
                tokio::time::timeout(timeout_dur, http_fut)
                    .await
                    .unwrap_or(Err(ProviderError::Timeout))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_openai_payload_modern_and_reasoning_models_use_max_completion_tokens() {
        let provider = HttpProvider::new("openai", "https://api.openai.com/v1", None, 30_000);
        let req = ProviderRequest {
            model: "gpt-5.6-terra".to_string(),
            messages: vec![crate::domain::types::messages::SessionMessage::new(Role::User, "Hello")],
            tools: Vec::new(),
            max_tokens: 4096,
            temperature: 0.0,
        };
        let payload = provider.build_openai_payload(&req);
        assert_eq!(payload["max_completion_tokens"], 4096);
        assert!(payload.get("max_tokens").is_none());
        assert_eq!(payload["temperature"], 0.0);

        let req_with_tools = ProviderRequest {
            model: "gpt-5.6-terra".to_string(),
            messages: vec![crate::domain::types::messages::SessionMessage::new(Role::User, "List files")],
            tools: vec![serde_json::json!({
                "type": "function",
                "function": { "name": "list_files", "parameters": {} }
            })],
            max_tokens: 4096,
            temperature: 0.0,
        };
        let payload_tools = provider.build_openai_payload(&req_with_tools);
        assert_eq!(payload_tools["reasoning_effort"], "none");

        for reasoning_model in ["o1-mini", "o3-mini", "o4-mini"] {
            let req_o = ProviderRequest {
                model: reasoning_model.to_string(),
                messages: vec![crate::domain::types::messages::SessionMessage::new(Role::User, "Hello")],
                tools: Vec::new(),
                max_tokens: 2048,
                temperature: 1.0,
            };
            let payload_o = provider.build_openai_payload(&req_o);
            assert_eq!(payload_o["max_completion_tokens"], 2048);
            assert!(payload_o.get("max_tokens").is_none());
            assert!(payload_o.get("temperature").is_none());
        }
    }

    #[test]
    fn test_build_openai_payload_legacy_models_use_max_tokens() {
        let provider = HttpProvider::new("openai", "https://api.openai.com/v1", None, 30_000);
        let req = ProviderRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![crate::domain::types::messages::SessionMessage::new(Role::User, "Hello")],
            tools: Vec::new(),
            max_tokens: 1024,
            temperature: 0.0,
        };
        let payload = provider.build_openai_payload(&req);
        assert_eq!(payload["max_tokens"], 1024);
        assert!(payload.get("max_completion_tokens").is_none());
        assert_eq!(payload["temperature"], 0.0);
    }
}
