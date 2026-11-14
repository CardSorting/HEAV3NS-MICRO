// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::{BoxFuture, IHttpProvider, ILlmProvider};
use crate::domain::types::messages::{ToolCall, ToolCallFunction};
use crate::domain::types::provider_registry::{canonicalize_provider_id, infer_wire_dialect_from_endpoint, lookup_provider_descriptor, opencode_go_model_id, resolve_model_aware_wire_and_model, retired_provider_remediation, WireDialect};
use crate::domain::types::runtime::CancellationToken;
use crate::domain::types::tick_result::{finalize_provider_response, is_retryable_http_status, ProviderError, ProviderRequest, ProviderResponse, ProviderTokenUsage};
use crate::infrastructure::codex_oauth::{build_codex_oauth_post, resolve_codex_base_url, CodexAccountStore};
use crate::infrastructure::sse_and_response_parsers::{parse_multidialect_sse_stream, parse_responses_json, IncrementalSseAggregator};
use crate::infrastructure::wire_dialects::{build_anthropic_messages_payload, build_chat_completions_payload_with_endpoint, build_google_gemini_payload, build_openai_responses_payload};
use serde_json::Value;
use std::time::Duration;

/// Hard cap (`16 MiB`) on buffered HTTP/SSE response bodies to prevent OOM from runaway upstream streams.
pub const MAX_HTTP_RESPONSE_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_DISCOVERY_BODY_BYTES: usize = 2 * 1024 * 1024;

async fn read_bounded_response_text(mut resp: reqwest::Response, max_bytes: usize) -> Result<String, ProviderError> {
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| ProviderError::Transport(e.to_string()))? {
        if buf.len().saturating_add(chunk.len()) > max_bytes {
            return Err(ProviderError::Transport(format!(
                "HTTP response body exceeded {max_bytes}-byte safety cap"
            )));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Fast non-blocking probe (`150ms` timeout) returning all local Ollama model tags.
pub async fn probe_local_ollama_models() -> Vec<String> {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_millis(150))
        .build()
    else {
        return Vec::new();
    };
    let Ok(resp) = client.get("http://127.0.0.1:11434/api/tags").send().await else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let Ok(raw) = read_bounded_response_text(resp, MAX_DISCOVERY_BODY_BYTES).await else {
        return Vec::new();
    };
    let Ok(body) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    body.get("models")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Fast non-blocking probe (`150ms` timeout) for the primary local Ollama chat model.
pub async fn probe_local_ollama_model() -> Option<String> {
    probe_local_ollama_models().await.into_iter().next()
}

/// Bounded live `GET /models` discovery probe for OpenAI-compatible gateways (vLLM, LM Studio, Groq, Mistral, etc.).
pub async fn probe_openai_compatible_models(endpoint: &str, api_key: Option<&str>, timeout_ms: u64) -> Vec<String> {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms.clamp(100, 5_000)))
        .build()
    else {
        return Vec::new();
    };
    let base = endpoint.trim_end_matches('/');
    let url = if base.ends_with("/models") {
        base.to_string()
    } else {
        format!("{base}/models")
    };
    let mut req = client.get(&url);
    if let Some(k) = api_key.filter(|s| !s.trim().is_empty()) {
        req = req.bearer_auth(k);
    }
    let Ok(resp) = req.send().await else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let Ok(raw) = read_bounded_response_text(resp, MAX_DISCOVERY_BODY_BYTES).await else {
        return Vec::new();
    };
    let Ok(body) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    body.get("data")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(Value::as_str).map(String::from))
                .take(64)
                .collect()
        })
        .unwrap_or_default()
}

/// Parses SSE streams across OpenAI Chat, OpenAI Responses, Anthropic Messages, and Gemini dialects.
pub fn parse_sse_stream(raw_sse: &str, provider_label: &str) -> Result<ProviderResponse, ProviderError> {
    parse_multidialect_sse_stream(raw_sse, provider_label, extract_openai_usage)
}

pub(crate) fn extract_openai_usage(body: &Value) -> Option<ProviderTokenUsage> {
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

fn is_event_stream_content_type(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|ct| ct.to_ascii_lowercase().contains("text/event-stream"))
}

fn looks_like_sse(text: &str, content_type: Option<&str>) -> bool {
    if is_event_stream_content_type(content_type) {
        return true;
    }
    let trimmed = text.trim_start();
    trimmed.starts_with("data:")
        || trimmed.starts_with("event:")
        || (trimmed.starts_with(':') && (trimmed.contains("\ndata:") || trimmed.contains("\nevent:")))
}

fn is_non_loopback_cleartext_http(endpoint: &str) -> bool {
    let low = endpoint.trim().to_ascii_lowercase();
    let Some(rest) = low.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit_once('@').map(|(_, hp)| hp).unwrap_or(authority);
    let host = if host_port.starts_with('[') {
        host_port.split(']').next().unwrap_or("").trim_start_matches('[')
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    !matches!(host, "localhost" | "127.0.0.1" | "::1")
}

async fn stream_incremental_sse_response(
    mut resp: reqwest::Response,
    provider_label: &str,
    max_bytes: usize,
) -> Result<ProviderResponse, ProviderError> {
    let mut agg = IncrementalSseAggregator::new(provider_label, extract_openai_usage, max_bytes);
    while let Some(chunk) = resp.chunk().await.map_err(|e| ProviderError::Transport(e.to_string()))? {
        agg.push_bytes(&chunk)?;
    }
    agg.finish()
}

/// Sovereign 4-dialect HTTP LLM adapter supporting OpenAI Chat Completions, OpenAI `/v1/responses`,
/// Anthropic `/v1/messages`, and Google Gemini `:generateContent` across 65+ providers.
pub struct HttpProvider {
    name: String,
    endpoint: String,
    api_key: Option<String>,
    timeout_ms: u64,
    wire_dialect: WireDialect,
    explicit_wire_override: bool,
    client: reqwest::Client,
}

impl HttpProvider {
    pub extern "Rust" fn new(
        name: impl Into<String>,
        endpoint: impl Into<String>,
        api_key: Option<String>,
        timeout_ms: u64,
    ) -> Self {
        let name_str = name.into();
        let endpoint_str = endpoint.into();
        let wire_dialect = lookup_provider_descriptor(&name_str)
            .map(|d| d.wire_dialect)
            .unwrap_or_else(|| infer_wire_dialect_from_endpoint(&endpoint_str));
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(8)
            .tcp_nodelay(true)
            .build()
            .unwrap_or_default();
        Self {
            name: name_str,
            endpoint: endpoint_str,
            api_key,
            timeout_ms: timeout_ms.max(1_000),
            wire_dialect,
            explicit_wire_override: false,
            client,
        }
    }

    pub fn with_wire_dialect(mut self, wire_dialect: WireDialect) -> Self {
        self.wire_dialect = wire_dialect;
        self
    }

    pub fn with_explicit_wire_override(mut self, explicit: bool) -> Self {
        self.explicit_wire_override = explicit;
        self
    }

    #[inline]
    pub const fn wire_dialect(&self) -> WireDialect {
        self.wire_dialect
    }

    pub(crate) fn resolve_request_route(&self, raw_model: &str) -> (WireDialect, String) {
        let (model_wire, normalized_model) =
            resolve_model_aware_wire_and_model(&self.name, raw_model, &self.endpoint, self.wire_dialect);
        if self.explicit_wire_override {
            (self.wire_dialect, normalized_model)
        } else {
            (model_wire, normalized_model)
        }
    }

    #[allow(dead_code)]
    pub(crate) fn resolve_url(&self, model: &str) -> String {
        let (dialect, wire_model) = self.resolve_request_route(model);
        self.resolve_url_for(dialect, &wire_model)
    }

    pub(crate) fn resolve_url_for(&self, dialect: WireDialect, model: &str) -> String {
        let trimmed = self.endpoint.trim_end_matches('/');
        match dialect {
            WireDialect::AnthropicMessages => {
                if trimmed.ends_with("/messages") {
                    trimmed.to_string()
                } else if trimmed.ends_with("/v1") {
                    format!("{trimmed}/messages")
                } else {
                    format!("{trimmed}/v1/messages")
                }
            }
            WireDialect::GoogleGemini => {
                if trimmed.contains(":generateContent") {
                    trimmed.to_string()
                } else {
                    format!("{trimmed}/models/{model}:generateContent")
                }
            }
            WireDialect::OpenAiResponses => {
                if trimmed.ends_with("/responses") {
                    trimmed.to_string()
                } else if canonicalize_provider_id(&self.name) == "deepseek"
                    && matches!(
                        trimmed.to_ascii_lowercase().as_str(),
                        "https://api.deepseek.com"
                            | "https://api.deepseek.com/v1"
                            | "https://api.deepseek.com/beta"
                            | "https://api.deepseeki.com"
                            | "https://api.deepseeki.com/v1"
                            | "https://api.deepseeki.com/beta"
                    )
                {
                    let base = trimmed
                        .strip_suffix("/v1")
                        .or_else(|| trimmed.strip_suffix("/beta"))
                        .unwrap_or(trimmed);
                    format!("{base}/responses")
                } else if trimmed.ends_with("/v1") {
                    format!("{trimmed}/responses")
                } else {
                    format!("{trimmed}/v1/responses")
                }
            }
            WireDialect::ChatCompletions => {
                if trimmed.ends_with("/chat/completions") {
                    trimmed.to_string()
                } else {
                    format!("{trimmed}/chat/completions")
                }
            }
        }
    }

    pub(crate) fn build_openai_payload(&self, req: &ProviderRequest) -> Value {
        build_chat_completions_payload_with_endpoint(&self.name, &self.endpoint, req)
    }

    pub(crate) fn build_responses_payload(&self, req: &ProviderRequest) -> Value {
        build_openai_responses_payload(&self.name, req)
    }

    pub(crate) fn build_anthropic_payload(&self, req: &ProviderRequest) -> Value {
        build_anthropic_messages_payload(&self.name, &self.endpoint, req)
    }

    pub(crate) fn build_gemini_payload(&self, req: &ProviderRequest) -> Value {
        build_google_gemini_payload(req)
    }

    fn apply_auth_headers(&self, builder: reqwest::RequestBuilder, dialect: WireDialect) -> reqwest::RequestBuilder {
        let mut builder = builder;
        let canon = canonicalize_provider_id(&self.name);
        if canon == "openrouter" {
            builder = builder
                .header("HTTP-Referer", "https://github.com/heav3ns/heav3ns-micro")
                .header("X-Title", "HEAV3NS-MICRO");
        }
        let Some(ref key) = self.api_key else {
            return builder;
        };
        match dialect {
            WireDialect::AnthropicMessages => {
                builder = builder.header("anthropic-version", "2023-06-01");
                if canon == "codewhale" {
                    return builder.bearer_auth(key);
                }
                let is_oauth = canon.contains("subscription") || key.starts_with("sk-ant-oat");
                if is_oauth {
                    builder = builder
                        .bearer_auth(key)
                        .header("anthropic-beta", "oauth-2025-04-20");
                } else {
                    builder = builder.header("x-api-key", key);
                    if !self.endpoint.to_ascii_lowercase().contains("api.anthropic.com") {
                        builder = builder.bearer_auth(key);
                    }
                }
                builder
            }
            WireDialect::GoogleGemini => builder.header("x-goog-api-key", key),
            WireDialect::OpenAiResponses | WireDialect::ChatCompletions => builder.bearer_auth(key),
        }
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
                    let sig = tc
                        .pointer("/extra_content/google/thought_signature")
                        .or_else(|| tc.pointer("/extra_content/google/thoughtSignature"))
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .map(String::from);
                    if !name.is_empty() {
                        tool_calls.push(ToolCall {
                            id,
                            r#type: "function".to_string(),
                            function: ToolCallFunction { name, arguments: raw_args },
                            thought_signature: sig,
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
                            function: ToolCallFunction { name, arguments: input },
                            thought_signature: None,
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
            .ok_or_else(|| ProviderError::Transport("Missing candidates[0].content.parts in Gemini response".to_string()))?;
        let mut text_parts = Vec::new();
        let mut thought_parts = Vec::new();
        let mut tool_calls = Vec::new();
        for (idx, part) in parts.iter().enumerate() {
            let is_thought = part.get("thought").and_then(Value::as_bool) == Some(true);
            if let Some(t) = part.get("text").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                if is_thought {
                    thought_parts.push(t.to_string());
                } else {
                    text_parts.push(t.to_string());
                }
            }
            if let Some(fc) = part.get("functionCall") {
                let name = fc.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let args = fc.get("args").map(|v| v.to_string()).unwrap_or_else(|| "{}".to_string());
                let sig = part
                    .get("thoughtSignature")
                    .or_else(|| part.get("thought_signature"))
                    .or_else(|| fc.get("thoughtSignature"))
                    .or_else(|| fc.get("thought_signature"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                if !name.is_empty() {
                    tool_calls.push(ToolCall {
                        id: format!("gemini_call_{}", idx + 1),
                        r#type: "function".to_string(),
                        function: ToolCallFunction { name, arguments: args },
                        thought_signature: sig,
                    });
                }
            }
        }
        if text_parts.is_empty() && tool_calls.is_empty() && !thought_parts.is_empty() {
            text_parts = thought_parts;
        }
        let mut resp = finalize_provider_response(Some(text_parts.join("\n")), tool_calls, "gemini");
        if let Some(um) = body.get("usageMetadata") {
            resp.usage = Some(ProviderTokenUsage {
                prompt_tokens: um.get("promptTokenCount").and_then(Value::as_u64).unwrap_or(0) as usize,
                completion_tokens: um.get("candidatesTokenCount").and_then(Value::as_u64).unwrap_or(0) as usize,
                cached_prompt_tokens: um.get("cachedContentTokenCount").and_then(Value::as_u64).unwrap_or(0) as usize,
                is_exact: true,
            });
        }
        Ok(resp)
    }
}

impl IHttpProvider for HttpProvider {}

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
            if let Some(retired_msg) = retired_provider_remediation(&self.name) {
                return Err(ProviderError::HttpStatus {
                    status: 400,
                    body: retired_msg.to_string(),
                    retryable: false,
                    retry_after_ms: None,
                });
            }
            let canon_provider = canonicalize_provider_id(&self.name);
            if canon_provider == "opencode-go" && opencode_go_model_id(&req.model).is_none() {
                return Err(ProviderError::HttpStatus {
                    status: 400,
                    body: format!(
                        "Model '{}' is not in OpenCode Go's supported roster; check `heav3ns-micro providers` or select a documented OpenCode Go model.",
                        req.model
                    ),
                    retryable: false,
                    retry_after_ms: None,
                });
            }
            if canon_provider == "codewhale" && is_non_loopback_cleartext_http(&self.endpoint) {
                return Err(ProviderError::HttpStatus {
                    status: 400,
                    body: "Refusing to transmit CODEWHALE_API_KEY over non-loopback cleartext HTTP; use https://api.codewhale.net/v1.".to_string(),
                    retryable: false,
                    retry_after_ms: None,
                });
            }

            let (effective_wire, wire_model) = self.resolve_request_route(&req.model);
            let mut effective_req = req;
            effective_req.model = wire_model;
            let mut url = self.resolve_url_for(effective_wire, &effective_req.model);
            let mut payload = match effective_wire {
                WireDialect::AnthropicMessages => self.build_anthropic_payload(&effective_req),
                WireDialect::GoogleGemini => self.build_gemini_payload(&effective_req),
                WireDialect::OpenAiResponses => self.build_responses_payload(&effective_req),
                WireDialect::ChatCompletions => self.build_openai_payload(&effective_req),
            };

            let has_explicit_sk = self.api_key.as_deref().is_some_and(|k| k.trim().starts_with("sk-"));
            let codex_oauth = if canon_provider == "openai-codex"
                && !has_explicit_sk
                && self.endpoint.trim_end_matches('/') == "https://api.openai.com/v1"
            {
                CodexAccountStore::default_store()
                    .get_valid_active_credentials()
                    .await
                    .map_err(ProviderError::Transport)?
            } else {
                None
            };

            let builder = if let Some((ref oauth_token, ref account_id)) = codex_oauth {
                url = format!("{}/responses", resolve_codex_base_url());
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("stream".to_string(), Value::Bool(true));
                }
                build_codex_oauth_post(&self.client, &url, oauth_token, account_id.as_deref())
            } else {
                self.apply_auth_headers(
                    self.client.post(&url).header("content-type", "application/json"),
                    effective_wire,
                )
            };

            let http_fut = async {
                let resp = builder
                    .json(&payload)
                    .send()
                    .await
                    .map_err(|e| ProviderError::Transport(e.to_string()))?;
                let status = resp.status().as_u16();
                let retry_after_ms = parse_retry_after_header(resp.headers());
                let content_type = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).map(String::from);

                if (200..300).contains(&status) && is_event_stream_content_type(content_type.as_deref()) {
                    return stream_incremental_sse_response(resp, &self.name, MAX_HTTP_RESPONSE_BODY_BYTES).await;
                }

                let text = read_bounded_response_text(resp, MAX_HTTP_RESPONSE_BODY_BYTES).await?;

                if !(200..300).contains(&status) {
                    if effective_wire == WireDialect::ChatCompletions && status == 400 {
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
                            let retry_builder = self.apply_auth_headers(
                                self.client.post(&url).header("content-type", "application/json"),
                                effective_wire,
                            );
                            if let Ok(retry_resp) = retry_builder.json(&healed_payload).send().await {
                                let retry_status = retry_resp.status().as_u16();
                                let retry_ct = retry_resp.headers().get("content-type").and_then(|v| v.to_str().ok()).map(String::from);
                                if (200..300).contains(&retry_status) && is_event_stream_content_type(retry_ct.as_deref()) {
                                    return stream_incremental_sse_response(retry_resp, &self.name, MAX_HTTP_RESPONSE_BODY_BYTES).await;
                                }
                                let retry_text = read_bounded_response_text(retry_resp, MAX_HTTP_RESPONSE_BODY_BYTES).await?;
                                if (200..300).contains(&retry_status) {
                                    if looks_like_sse(&retry_text, retry_ct.as_deref()) {
                                        return parse_sse_stream(&retry_text, &self.name);
                                    }
                                    let json_body: Value = serde_json::from_str(&retry_text)
                                        .map_err(|e| ProviderError::Transport(format!("Invalid JSON from provider: {e}")))?;
                                    return Self::parse_openai_response(&json_body);
                                }
                            }
                        }
                    }

                    if codex_oauth.is_some() && status == 429 {
                        CodexAccountStore::default_store().record_rate_limit(retry_after_ms);
                    }
                    if let Some((ref rejected_tok, _)) = codex_oauth {
                        if status == 401 {
                            if let Ok((new_token, new_account_id)) = CodexAccountStore::default_store().force_refresh_active(rejected_tok).await {
                                let retry_b = build_codex_oauth_post(&self.client, &url, &new_token, new_account_id.as_deref());
                                if let Ok(retry_resp) = retry_b.json(&payload).send().await {
                                    let retry_status = retry_resp.status().as_u16();
                                    let retry_ct = retry_resp.headers().get("content-type").and_then(|v| v.to_str().ok()).map(String::from);
                                    if (200..300).contains(&retry_status) && is_event_stream_content_type(retry_ct.as_deref()) {
                                        return stream_incremental_sse_response(retry_resp, &self.name, MAX_HTTP_RESPONSE_BODY_BYTES).await;
                                    }
                                    let retry_text = read_bounded_response_text(retry_resp, MAX_HTTP_RESPONSE_BODY_BYTES).await?;
                                    if (200..300).contains(&retry_status) {
                                        if looks_like_sse(&retry_text, retry_ct.as_deref()) {
                                            return parse_sse_stream(&retry_text, &self.name);
                                        }
                                        let json_body: Value = serde_json::from_str(&retry_text)
                                            .map_err(|e| ProviderError::Transport(format!("Invalid JSON from provider: {e}")))?;
                                        return parse_responses_json(&json_body, &self.name);
                                    }
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

                if looks_like_sse(&text, content_type.as_deref()) {
                    return parse_sse_stream(&text, &self.name);
                }
                let json_body: Value = serde_json::from_str(&text)
                    .map_err(|e| ProviderError::Transport(format!("Invalid JSON from provider: {e}")))?;
                match effective_wire {
                    WireDialect::AnthropicMessages => Self::parse_anthropic_response(&json_body),
                    WireDialect::GoogleGemini => Self::parse_gemini_response(&json_body),
                    WireDialect::OpenAiResponses => parse_responses_json(&json_body, &self.name),
                    WireDialect::ChatCompletions => Self::parse_openai_response(&json_body),
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
    use crate::domain::types::messages::Role;

    #[test]
    fn test_build_openai_payload_modern_and_reasoning_models_use_max_completion_tokens() {
        let provider = HttpProvider::new("openai", "https://api.openai.com/v1", None, 30_000);
        let req = ProviderRequest {
            model: "gpt-5.6-terra".to_string(),
            messages: vec![crate::domain::types::messages::SessionMessage::new(Role::User, "Hello")],
            tools: Vec::new(),
            max_tokens: 4096,
            temperature: 0.0,
            reasoning_effort: None,
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
            reasoning_effort: None,
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
                reasoning_effort: None,
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
            reasoning_effort: None,
        };
        let payload = provider.build_openai_payload(&req);
        assert_eq!(payload["max_tokens"], 1024);
        assert!(payload.get("max_completion_tokens").is_none());
        assert_eq!(payload["temperature"], 0.0);
    }
}
