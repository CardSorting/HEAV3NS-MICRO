// [LAYER: INFRASTRUCTURE]
use crate::domain::types::messages::{ToolCall, ToolCallFunction};
use crate::domain::types::tick_result::{finalize_provider_response, is_retryable_http_status, ProviderError, ProviderResponse, ProviderTokenUsage};
use crate::plumbing::json_repair::{accumulate_streaming_tool_name, compute_streaming_arguments_delta};
use crate::plumbing::schema_and_sse::MAX_SSE_EVENT_DATA_BYTES;
use serde::de::IgnoredAny;
use serde_json::Value;
use std::collections::BTreeMap;

fn classify_stream_or_response_error(chunk: &Value, ev_type: &str, raw_body: &str) -> Option<ProviderError> {
    let err_obj = chunk
        .get("error")
        .filter(|v| !v.is_null())
        .or_else(|| chunk.get("response").and_then(|r| r.get("error")).filter(|v| !v.is_null()));

    let is_explicit_error_event = ev_type == "error"
        || ev_type == "response.failed"
        || chunk.get("status").and_then(Value::as_str) == Some("failed");
    let is_top_level_error_chunk = err_obj.is_some()
        && chunk.get("choices").is_none()
        && chunk.get("candidates").is_none();

    if !is_explicit_error_event && !is_top_level_error_chunk {
        return None;
    }

    let explicit_status = err_obj
        .and_then(|e| e.get("status").or_else(|| e.get("code")))
        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
        .filter(|s| (400..=599).contains(s))
        .map(|s| s as u16);

    let code_or_type = err_obj
        .and_then(|e| {
            e.get("type")
                .or_else(|| e.get("code"))
                .and_then(Value::as_str)
                .or_else(|| e.as_str())
        })
        .unwrap_or("")
        .to_ascii_lowercase();

    let status = explicit_status.unwrap_or_else(|| {
        if code_or_type.contains("overloaded") {
            529
        } else if code_or_type.contains("rate_limit") || code_or_type.contains("too_many_requests") {
            429
        } else if code_or_type.contains("insufficient_quota")
            || code_or_type.contains("billing")
            || code_or_type.contains("credit_balance")
        {
            402
        } else if code_or_type.contains("authentication") || code_or_type.contains("invalid_api_key") {
            401
        } else if code_or_type.contains("permission") || code_or_type.contains("forbidden") {
            403
        } else if code_or_type.contains("invalid_request") || code_or_type.contains("bad_request") {
            400
        } else {
            502
        }
    });

    Some(ProviderError::HttpStatus {
        status,
        body: raw_body.to_string(),
        retryable: is_retryable_http_status(status),
        retry_after_ms: None,
    })
}

/// Parses a non-streaming OpenAI `/v1/responses` JSON payload.
pub(crate) fn parse_responses_json(body: &Value, provider_label: &str) -> Result<ProviderResponse, ProviderError> {
    let root = body.get("response").unwrap_or(body);
    if let Some(err) = classify_stream_or_response_error(root, "", &body.to_string()) {
        return Err(err);
    }
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();

    if let Some(output) = root.get("output").and_then(Value::as_array) {
        for (idx, item) in output.iter().enumerate() {
            match item.get("type").and_then(Value::as_str) {
                Some("message") => {
                    if let Some(content_arr) = item.get("content").and_then(Value::as_array) {
                        for part in content_arr {
                            if let Some(t) = part.get("text").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                                text_parts.push(t.to_string());
                            }
                        }
                    }
                }
                Some("function_call") => {
                    let id = item
                        .get("call_id")
                        .or_else(|| item.get("id"))
                        .and_then(Value::as_str)
                        .map(String::from)
                        .unwrap_or_else(|| format!("call_{}", idx + 1));
                    let name = item.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string();
                    let arguments = item.get("arguments").and_then(Value::as_str).unwrap_or("{}").to_string();
                    if !name.is_empty() {
                        tool_calls.push(ToolCall {
                            id,
                            r#type: "function".to_string(),
                            function: ToolCallFunction { name, arguments },
                            thought_signature: None,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    if text_parts.is_empty() {
        if let Some(ot) = root.get("output_text").and_then(Value::as_str).filter(|s| !s.is_empty()) {
            text_parts.push(ot.to_string());
        }
    }

    let mut resp = finalize_provider_response(Some(text_parts.join("")), tool_calls, provider_label);
    resp.usage = extract_responses_usage(root);
    Ok(resp)
}

pub(crate) fn extract_responses_usage(root: &Value) -> Option<ProviderTokenUsage> {
    let u = root.get("usage")?;
    let prompt_tokens = u.get("input_tokens").or_else(|| u.get("prompt_tokens")).and_then(Value::as_u64)? as usize;
    let completion_tokens = u.get("output_tokens").or_else(|| u.get("completion_tokens")).and_then(Value::as_u64).unwrap_or(0) as usize;
    let cached_prompt_tokens = u
        .get("input_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .or_else(|| u.get("prompt_tokens_details").and_then(|d| d.get("cached_tokens")))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    Some(ProviderTokenUsage {
        prompt_tokens,
        completion_tokens,
        cached_prompt_tokens,
        is_exact: true,
    })
}

type ToolSlot = (String, String, String, Option<String>);

/// Incremental byte-framed SSE state machine supporting OpenAI Chat Completions, OpenAI `/v1/responses`,
/// Anthropic `/v1/messages`, and Google Gemini SSE streams without buffering the full HTTP body first.
/// Frames raw bytes on `0x0A` (`\n`) so multi-byte UTF-8 codepoints split across TCP chunks never corrupt.
pub(crate) struct IncrementalSseAggregator {
    provider_label: String,
    extract_openai_usage_fn: fn(&Value) -> Option<ProviderTokenUsage>,
    max_total_bytes: usize,
    total_bytes: usize,
    line_buf: Vec<u8>,
    current_event: Option<String>,
    data_buf: String,
    has_data: bool,
    overflowed: bool,
    text_acc: String,
    thought_fallback: String,
    tool_map: BTreeMap<usize, ToolSlot>,
    item_id_to_index: BTreeMap<String, usize>,
    latest_usage: Option<ProviderTokenUsage>,
    anthropic_in_tokens: usize,
    anthropic_cache_read: usize,
    anthropic_out_tokens: usize,
    saw_anthropic_usage: bool,
    completed_override: Option<ProviderResponse>,
}

impl IncrementalSseAggregator {
    pub(crate) fn new(
        provider_label: &str,
        extract_openai_usage_fn: fn(&Value) -> Option<ProviderTokenUsage>,
        max_total_bytes: usize,
    ) -> Self {
        Self {
            provider_label: provider_label.to_string(),
            extract_openai_usage_fn,
            max_total_bytes,
            total_bytes: 0,
            line_buf: Vec::new(),
            current_event: None,
            data_buf: String::new(),
            has_data: false,
            overflowed: false,
            text_acc: String::new(),
            thought_fallback: String::new(),
            tool_map: BTreeMap::new(),
            item_id_to_index: BTreeMap::new(),
            latest_usage: None,
            anthropic_in_tokens: 0,
            anthropic_cache_read: 0,
            anthropic_out_tokens: 0,
            saw_anthropic_usage: false,
            completed_override: None,
        }
    }

    pub(crate) fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), ProviderError> {
        self.total_bytes = self.total_bytes.saturating_add(bytes.len());
        if self.total_bytes > self.max_total_bytes {
            return Err(ProviderError::Transport(format!(
                "HTTP response body exceeded {}-byte safety cap",
                self.max_total_bytes
            )));
        }

        let mut start = 0usize;
        for (idx, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                let slice = &bytes[start..idx];
                if self.line_buf.is_empty() {
                    self.process_raw_line(slice)?;
                } else {
                    self.line_buf.extend_from_slice(slice);
                    let taken = std::mem::take(&mut self.line_buf);
                    self.process_raw_line(&taken)?;
                }
                start = idx + 1;
            }
        }
        if start < bytes.len() {
            self.line_buf.extend_from_slice(&bytes[start..]);
        }
        Ok(())
    }

    fn process_raw_line(&mut self, raw: &[u8]) -> Result<(), ProviderError> {
        let trimmed_cr = raw.strip_suffix(b"\r").unwrap_or(raw);
        match std::str::from_utf8(trimmed_cr) {
            Ok(line) => self.process_line(line),
            Err(_) => {
                let lossy = String::from_utf8_lossy(trimmed_cr);
                self.process_line(&lossy)
            }
        }
    }

    fn process_line(&mut self, line: &str) -> Result<(), ProviderError> {
        if line.is_empty() {
            return self.flush_event();
        }
        if line.starts_with(':') {
            return Ok(());
        }
        let (field, raw_val) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => {
                if self.has_data || self.overflowed {
                    self.flush_event()?;
                }
                self.current_event = Some(raw_val.trim().to_string());
            }
            "data" => {
                if raw_val.trim() == "[DONE]" {
                    return self.flush_event();
                }
                if self.has_data
                    && !self.overflowed
                    && serde_json::from_str::<IgnoredAny>(self.data_buf.trim()).is_ok()
                {
                    self.flush_event()?;
                }
                if self.overflowed {
                    return Ok(());
                }
                let needed = raw_val.len() + usize::from(self.has_data);
                if self.data_buf.len() + needed > MAX_SSE_EVENT_DATA_BYTES {
                    self.overflowed = true;
                    self.data_buf.clear();
                    return Ok(());
                }
                if self.has_data {
                    self.data_buf.push('\n');
                }
                self.data_buf.push_str(raw_val);
                self.has_data = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn flush_event(&mut self) -> Result<(), ProviderError> {
        let event = self.current_event.take();
        let had_data = self.has_data;
        let overflowed = self.overflowed;
        self.has_data = false;
        self.overflowed = false;

        if !had_data || overflowed {
            self.data_buf.clear();
            return Ok(());
        }
        let raw_data = std::mem::take(&mut self.data_buf);
        let trimmed = raw_data.trim();
        if trimmed.is_empty() || trimmed == "[DONE]" {
            return Ok(());
        }
        self.ingest_frame(event.as_deref(), &raw_data)
    }

    fn ingest_frame(&mut self, frame_event: Option<&str>, raw_data: &str) -> Result<(), ProviderError> {
        if self.completed_override.is_some() {
            return Ok(());
        }
        let Ok(chunk) = serde_json::from_str::<Value>(raw_data) else {
            return Ok(());
        };
        let ev_type = chunk
            .get("type")
            .and_then(Value::as_str)
            .or(frame_event)
            .unwrap_or("");

        if let Some(err) = classify_stream_or_response_error(&chunk, ev_type, raw_data) {
            return Err(err);
        }

        // 1. Anthropic Messages SSE
        if matches!(ev_type, "message_start" | "content_block_start" | "content_block_delta" | "message_delta") {
            match ev_type {
                "message_start" => {
                    if let Some(u) = chunk.get("message").and_then(|m| m.get("usage")) {
                        let uncached = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
                        let cr = u.get("cache_read_input_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
                        let cc = u.get("cache_creation_input_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
                        self.anthropic_in_tokens = uncached + cr + cc;
                        self.anthropic_cache_read = cr;
                        self.anthropic_out_tokens = u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
                        self.saw_anthropic_usage = true;
                    }
                }
                "content_block_start" => {
                    let idx = chunk.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                    if let Some(cb) = chunk.get("content_block") {
                        match cb.get("type").and_then(Value::as_str) {
                            Some("tool_use") => {
                                let id = cb.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                                let name = cb.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                                let entry = self.tool_map.entry(idx).or_insert_with(|| {
                                    (format!("toolu_{}", idx + 1), String::new(), String::new(), None)
                                });
                                if !id.is_empty() {
                                    entry.0 = id;
                                }
                                if !name.is_empty() {
                                    entry.1 = name;
                                }
                            }
                            Some("text") => {
                                if let Some(t) = cb.get("text").and_then(Value::as_str) {
                                    self.text_acc.push_str(t);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "content_block_delta" => {
                    let idx = chunk.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                    if let Some(delta) = chunk.get("delta") {
                        match delta.get("type").and_then(Value::as_str) {
                            Some("text_delta") => {
                                if let Some(t) = delta.get("text").and_then(Value::as_str) {
                                    self.text_acc.push_str(t);
                                }
                            }
                            Some("input_json_delta") => {
                                if let Some(pj) = delta.get("partial_json").and_then(Value::as_str) {
                                    let entry = self.tool_map.entry(idx).or_insert_with(|| {
                                        (format!("toolu_{}", idx + 1), String::new(), String::new(), None)
                                    });
                                    entry.2.push_str(pj);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "message_delta" => {
                    if let Some(u) = chunk.get("usage") {
                        if let Some(out) = u.get("output_tokens").and_then(Value::as_u64) {
                            self.anthropic_out_tokens = out as usize;
                            self.saw_anthropic_usage = true;
                        }
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        // 2. OpenAI Responses API SSE (`response.*`)
        if ev_type.starts_with("response.") {
            match ev_type {
                "response.output_text.delta" => {
                    if let Some(d) = chunk.get("delta").and_then(Value::as_str) {
                        self.text_acc.push_str(d);
                    }
                }
                "response.output_item.added" | "response.output_item.done" => {
                    let idx = chunk.get("output_index").and_then(Value::as_u64).unwrap_or(0) as usize;
                    if let Some(item) = chunk.get("item") {
                        if item.get("type").and_then(Value::as_str) == Some("function_call") {
                            let call_id = item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or("").to_string();
                            if let Some(item_id) = item.get("id").and_then(Value::as_str) {
                                self.item_id_to_index.insert(item_id.to_string(), idx);
                            }
                            let name = item.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                            let entry = self.tool_map.entry(idx).or_insert_with(|| {
                                (format!("call_{}", idx + 1), String::new(), String::new(), None)
                            });
                            if !call_id.is_empty() {
                                entry.0 = call_id;
                            }
                            if !name.is_empty() {
                                entry.1 = name;
                            }
                            if let Some(args) = item.get("arguments").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                                entry.2 = args.to_string();
                            }
                        }
                    }
                }
                "response.function_call_arguments.delta" => {
                    let idx = chunk
                        .get("output_index")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize)
                        .or_else(|| chunk.get("item_id").and_then(Value::as_str).and_then(|id| self.item_id_to_index.get(id).copied()))
                        .unwrap_or(0);
                    if let Some(d) = chunk.get("delta").and_then(Value::as_str) {
                        let entry = self.tool_map.entry(idx).or_insert_with(|| {
                            (format!("call_{}", idx + 1), String::new(), String::new(), None)
                        });
                        entry.2.push_str(d);
                    }
                }
                "response.function_call_arguments.done" => {
                    let idx = chunk
                        .get("output_index")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize)
                        .or_else(|| chunk.get("item_id").and_then(Value::as_str).and_then(|id| self.item_id_to_index.get(id).copied()))
                        .unwrap_or(0);
                    if let Some(args) = chunk.get("arguments").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                        let entry = self.tool_map.entry(idx).or_insert_with(|| {
                            (format!("call_{}", idx + 1), String::new(), String::new(), None)
                        });
                        if entry.2.is_empty() {
                            entry.2 = args.to_string();
                        }
                    }
                }
                "response.completed" | "response.done" => {
                    if let Some(resp_obj) = chunk.get("response") {
                        if let Some(u) = extract_responses_usage(resp_obj) {
                            self.latest_usage = Some(u);
                        }
                        if self.text_acc.is_empty() && self.tool_map.is_empty() {
                            self.completed_override = Some(parse_responses_json(resp_obj, &self.provider_label)?);
                            return Ok(());
                        }
                        if self.tool_map.is_empty() {
                            if let Some(output) = resp_obj.get("output").and_then(Value::as_array) {
                                for (idx, item) in output.iter().enumerate() {
                                    if item.get("type").and_then(Value::as_str) == Some("function_call") {
                                        let cid = item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str).unwrap_or("").to_string();
                                        let name = item.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                                        let args = item.get("arguments").and_then(Value::as_str).unwrap_or("{}").to_string();
                                        if !name.is_empty() {
                                            self.tool_map.insert(
                                                idx,
                                                (
                                                    if cid.is_empty() { format!("call_{}", idx + 1) } else { cid },
                                                    name,
                                                    args,
                                                    None,
                                                ),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        // 3. Google Gemini SSE (`candidates[0].content.parts`)
        if let Some(parts) = chunk
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(Value::as_array)
        {
            for (idx, part) in parts.iter().enumerate() {
                let is_thought = part.get("thought").and_then(Value::as_bool) == Some(true);
                if let Some(t) = part.get("text").and_then(Value::as_str) {
                    if is_thought {
                        self.thought_fallback.push_str(t);
                    } else {
                        self.text_acc.push_str(t);
                    }
                }
                if let Some(fc) = part.get("functionCall") {
                    let name = fc.get("name").and_then(Value::as_str).unwrap_or("").to_string();
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
                        let next_idx = self.tool_map.len() + idx;
                        self.tool_map.insert(next_idx, (format!("gemini_call_{}", next_idx + 1), name, args, sig));
                    }
                }
            }
            if let Some(um) = chunk.get("usageMetadata") {
                self.latest_usage = Some(ProviderTokenUsage {
                    prompt_tokens: um.get("promptTokenCount").and_then(Value::as_u64).unwrap_or(0) as usize,
                    completion_tokens: um.get("candidatesTokenCount").and_then(Value::as_u64).unwrap_or(0) as usize,
                    cached_prompt_tokens: um.get("cachedContentTokenCount").and_then(Value::as_u64).unwrap_or(0) as usize,
                    is_exact: true,
                });
            }
            return Ok(());
        }

        // 4. OpenAI Chat Completions SSE (`choices[0].delta`)
        if let Some(u) = (self.extract_openai_usage_fn)(&chunk) {
            self.latest_usage = Some(u);
        }
        if let Some(choice) = chunk.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first()) {
            let delta = choice.get("delta").or_else(|| choice.get("message")).unwrap_or(choice);
            if let Some(txt) = delta.get("content").and_then(|v| v.as_str()) {
                self.text_acc.push_str(txt);
            }
            if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                for (i, tc) in tcs.iter().enumerate() {
                    let idx = tc.get("index").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(i);
                    let entry = self
                        .tool_map
                        .entry(idx)
                        .or_insert_with(|| (format!("call_{}", idx + 1), String::new(), String::new(), None));
                    if let Some(id) = tc.get("id").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                        entry.0 = id.to_string();
                    }
                    if let Some(sig) = tc
                        .pointer("/extra_content/google/thought_signature")
                        .or_else(|| tc.pointer("/extra_content/google/thoughtSignature"))
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                    {
                        entry.3 = Some(sig.to_string());
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
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<ProviderResponse, ProviderError> {
        if !self.line_buf.is_empty() {
            let trailing = std::mem::take(&mut self.line_buf);
            self.process_raw_line(&trailing)?;
        }
        self.flush_event()?;

        if let Some(early) = self.completed_override {
            return Ok(early);
        }

        if self.saw_anthropic_usage {
            self.latest_usage = Some(ProviderTokenUsage {
                prompt_tokens: self.anthropic_in_tokens,
                completion_tokens: self.anthropic_out_tokens,
                cached_prompt_tokens: self.anthropic_cache_read,
                is_exact: true,
            });
        }

        let tool_calls: Vec<ToolCall> = self
            .tool_map
            .into_values()
            .filter(|(_, name, _, _)| !name.is_empty())
            .map(|(id, name, args, thought_signature)| ToolCall {
                id,
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name,
                    arguments: if args.is_empty() { "{}".to_string() } else { args },
                },
                thought_signature,
            })
            .collect();

        if self.text_acc.is_empty() && tool_calls.is_empty() && !self.thought_fallback.is_empty() {
            self.text_acc = self.thought_fallback;
        }

        let mut resp = finalize_provider_response(Some(self.text_acc), tool_calls, &self.provider_label);
        resp.usage = self.latest_usage;
        Ok(resp)
    }
}

/// Unified multi-dialect SSE stream parser supporting OpenAI Chat Completions,
/// OpenAI `/v1/responses`, Anthropic `/v1/messages`, and Google Gemini SSE streams.
pub(crate) fn parse_multidialect_sse_stream(
    raw_sse: &str,
    provider_label: &str,
    extract_openai_usage_fn: fn(&Value) -> Option<ProviderTokenUsage>,
) -> Result<ProviderResponse, ProviderError> {
    let mut agg = IncrementalSseAggregator::new(provider_label, extract_openai_usage_fn, usize::MAX);
    agg.push_bytes(raw_sse.as_bytes())?;
    agg.finish()
}
