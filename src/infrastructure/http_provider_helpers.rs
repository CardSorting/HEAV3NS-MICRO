// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::ProgressCallback;
use crate::domain::types::messages::{ToolCall, ToolCallFunction};
use crate::domain::types::progress::{EngineProgressEvent, EngineProgressMetadata, EngineProgressPhase, EngineProgressStatus};
use crate::domain::types::tick_result::{finalize_provider_response, ProviderError, ProviderResponse, ProviderTokenUsage};
use crate::infrastructure::sse_and_response_parsers::{classify_stream_or_response_error, parse_multidialect_sse_stream, IncrementalSseAggregator};
use crate::plumbing::hash_and_math::now_ms;
use serde_json::Value;
use std::time::{Duration, Instant};

const MAX_DISCOVERY_BODY_BYTES: usize = 2 * 1024 * 1024;

pub(crate) async fn read_bounded_response_text(
    mut resp: reqwest::Response,
    max_bytes: usize,
) -> Result<String, ProviderError> {
    let mut buf = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| ProviderError::Transport(e.without_url().to_string()))?
    {
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
pub async fn probe_openai_compatible_models(
    endpoint: &str,
    api_key: Option<&str>,
    timeout_ms: u64,
) -> Vec<String> {
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
    let has_key = api_key.is_some_and(|s| !s.trim().is_empty());
    if has_key && is_non_loopback_cleartext_http(endpoint) {
        return Vec::new();
    }
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
pub fn parse_sse_stream(
    raw_sse: &str,
    provider_label: &str,
) -> Result<ProviderResponse, ProviderError> {
    parse_multidialect_sse_stream(raw_sse, provider_label, extract_openai_usage)
}

pub(crate) fn extract_openai_usage(body: &Value) -> Option<ProviderTokenUsage> {
    let u = body.get("usage")?;
    let prompt_tokens = u.get("prompt_tokens").and_then(Value::as_u64)? as usize;
    let completion_tokens = u
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
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

pub(crate) fn parse_retry_after_header(headers: &reqwest::header::HeaderMap) -> Option<u64> {
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

pub(crate) fn is_event_stream_content_type(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|ct| ct.to_ascii_lowercase().contains("text/event-stream"))
}

pub(crate) fn looks_like_sse(text: &str, content_type: Option<&str>) -> bool {
    if is_event_stream_content_type(content_type) {
        return true;
    }
    let trimmed = text.trim_start();
    trimmed.starts_with("data:")
        || trimmed.starts_with("event:")
        || (trimmed.starts_with(':')
            && (trimmed.contains("\ndata:") || trimmed.contains("\nevent:")))
}

pub(crate) fn is_non_loopback_cleartext_http(endpoint: &str) -> bool {
    let low = endpoint.trim().to_ascii_lowercase();
    let Some(rest) = low.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, hp)| hp)
        .unwrap_or(authority);
    let host = if host_port.starts_with('[') {
        host_port
            .split(']')
            .next()
            .unwrap_or("")
            .trim_start_matches('[')
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    !matches!(host, "localhost" | "127.0.0.1" | "::1")
}

pub(crate) fn emit_stream_progress(
    on_progress: Option<&ProgressCallback>,
    phase: EngineProgressPhase,
    message: String,
    detail: Option<String>,
    sequence: u64,
) {
    let Some(cb) = on_progress else { return };
    let activity_id = match phase {
        EngineProgressPhase::Thinking => "llm-stream-thinking",
        EngineProgressPhase::Connecting => "llm-stream-connecting",
        _ => "llm-stream-responding",
    };
    let preview_text = detail.clone();
    cb(EngineProgressEvent {
        activity_id: activity_id.to_string(),
        phase,
        status: EngineProgressStatus::InProgress,
        message,
        detail,
        timestamp: now_ms(),
        elapsed_ms: None,
        sequence,
        metadata: Some(EngineProgressMetadata {
            source: Some("provider".to_string()),
            scope: Some("turn".to_string()),
            attempt: None,
            item_type: Some("llm".to_string()),
            completed_steps: None,
            total_steps: None,
            preview_kind: preview_text.as_ref().map(|_| "message".to_string()),
            preview_text,
            ..Default::default()
        }),
    });
}

pub(crate) fn format_stream_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{} KB", (bytes + 512) / 1024)
    }
}

pub(crate) async fn stream_incremental_sse_response(
    mut resp: reqwest::Response,
    provider_label: &str,
    max_bytes: usize,
    on_progress: Option<&ProgressCallback>,
) -> Result<ProviderResponse, ProviderError> {
    let mut agg = IncrementalSseAggregator::new(provider_label, extract_openai_usage, max_bytes);
    let mut seq: u64 = 1;
    let mut bytes_received: usize = 0;
    let mut last_emit = on_progress.map(|_| Instant::now());
    if on_progress.is_some() {
        emit_stream_progress(
            on_progress,
            EngineProgressPhase::Thinking,
            "Thinking…".to_string(),
            Some("Model stream opened".to_string()),
            seq,
        );
    }
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| ProviderError::Transport(e.without_url().to_string()))?
    {
        bytes_received = bytes_received.saturating_add(chunk.len());
        agg.push_bytes(&chunk)?;
        if let Some(prev) = last_emit {
            let (text_preview, tool_name, tool_bytes) = agg.stream_snapshot();
            let first_content = seq == 1 && (text_preview.is_some() || tool_name.is_some());
            if first_content || prev.elapsed() >= Duration::from_millis(120) {
                seq = seq.saturating_add(1);
                let (msg, detail) = if let Some(ref tname) = tool_name {
                    (
                        format!("Drafting {tname} ({})…", format_stream_bytes(tool_bytes)),
                        text_preview.or_else(|| Some(format!("Streaming arguments for {tname}…"))),
                    )
                } else {
                    (
                        format!("Receiving model response ({})…", format_stream_bytes(bytes_received)),
                        text_preview,
                    )
                };
                emit_stream_progress(
                    on_progress,
                    EngineProgressPhase::Responding,
                    msg,
                    detail,
                    seq,
                );
                last_emit = Some(Instant::now());
            }
        }
    }
    agg.finish()
}

pub(crate) fn parse_openai_response(body: &Value) -> Result<ProviderResponse, ProviderError> {
        if let Some(err) = classify_stream_or_response_error(body, "", &body.to_string()) {
            return Err(err);
        }
        let choice = body
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| {
                ProviderError::Transport("Missing choices[0] in OpenAI response".to_string())
            })?;
        let msg = choice.get("message").unwrap_or(choice);
        let content = msg
            .get("content")
            .and_then(|c| c.as_str())
            .map(String::from);
        let mut tool_calls = Vec::new();
        if let Some(raw_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
            for (idx, tc) in raw_calls.iter().enumerate() {
                let id = tc
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| format!("call_{}", idx + 1));
                if let Some(func) = tc.get("function") {
                    let name = func
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
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
                            function: ToolCallFunction {
                                name,
                                arguments: raw_args,
                            },
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

pub(crate) fn parse_anthropic_response(body: &Value) -> Result<ProviderResponse, ProviderError> {
        if let Some(err) = classify_stream_or_response_error(body, "", &body.to_string()) {
            return Err(err);
        }
        let blocks = body
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                ProviderError::Transport("Missing content array in Anthropic response".to_string())
            })?;
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        for (idx, block) in blocks.iter().enumerate() {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(t) = block
                        .get("text")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        text_parts.push(t.to_string());
                    }
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .unwrap_or_else(|| format!("toolu_{}", idx + 1));
                    let name = block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = block
                        .get("input")
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "{}".to_string());
                    if !name.is_empty() {
                        tool_calls.push(ToolCall {
                            id,
                            r#type: "function".to_string(),
                            function: ToolCallFunction {
                                name,
                                arguments: input,
                            },
                            thought_signature: None,
                        });
                    }
                }
                _ => {}
            }
        }
        let mut resp =
            finalize_provider_response(Some(text_parts.join("\n")), tool_calls, "anthropic");
        if let Some(u) = body.get("usage") {
            let uncached = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
            let cache_read = u
                .get("cache_read_input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let cache_create = u
                .get("cache_creation_input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
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

pub(crate) fn parse_gemini_response(body: &Value) -> Result<ProviderResponse, ProviderError> {
        if let Some(err) = classify_stream_or_response_error(body, "", &body.to_string()) {
            return Err(err);
        }
        let candidate = body
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first());
        let raw_parts = candidate
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array());
        if raw_parts.is_none() {
            if let Some(reason) = candidate
                .and_then(|c| c.get("finishReason"))
                .and_then(Value::as_str)
            {
                if !matches!(reason, "STOP" | "MAX_TOKENS") {
                    return Err(ProviderError::HttpStatus {
                        status: 200,
                        body: format!("Gemini candidate blocked: finishReason = {reason}"),
                        retryable: false,
                        retry_after_ms: None,
                    });
                }
            }
        }
        let parts = raw_parts.ok_or_else(|| {
            ProviderError::Transport(
                "Missing candidates[0].content.parts in Gemini response".to_string(),
            )
        })?;
        let mut text_parts = Vec::new();
        let mut thought_parts = Vec::new();
        let mut tool_calls = Vec::new();
        for (idx, part) in parts.iter().enumerate() {
            let is_thought = part.get("thought").and_then(Value::as_bool) == Some(true);
            if let Some(t) = part
                .get("text")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                if is_thought {
                    thought_parts.push(t.to_string());
                } else {
                    text_parts.push(t.to_string());
                }
            }
            if let Some(fc) = part.get("functionCall") {
                let name = fc
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let args = fc
                    .get("args")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "{}".to_string());
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
                        function: ToolCallFunction {
                            name,
                            arguments: args,
                        },
                        thought_signature: sig,
                    });
                }
            }
        }
        if text_parts.is_empty() && tool_calls.is_empty() && !thought_parts.is_empty() {
            text_parts = thought_parts;
        }
        let mut resp =
            finalize_provider_response(Some(text_parts.join("\n")), tool_calls, "gemini");
        if let Some(um) = body.get("usageMetadata") {
            resp.usage = Some(ProviderTokenUsage {
                prompt_tokens: um
                    .get("promptTokenCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize,
                completion_tokens: um
                    .get("candidatesTokenCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize,
                cached_prompt_tokens: um
                    .get("cachedContentTokenCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize,
                is_exact: true,
            });
        }
        Ok(resp)
    }

