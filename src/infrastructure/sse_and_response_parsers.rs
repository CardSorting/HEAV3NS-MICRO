// [LAYER: INFRASTRUCTURE]
use crate::domain::types::messages::{ToolCall, ToolCallFunction};
use crate::domain::types::tick_result::{
    finalize_provider_response, ProviderError, ProviderResponse,
    ProviderTokenUsage,
};
use crate::plumbing::json_repair::{
    accumulate_streaming_tool_name, compute_streaming_arguments_delta,
};
use crate::plumbing::schema_and_sse::MAX_SSE_EVENT_DATA_BYTES;
use serde::de::IgnoredAny;
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) use crate::infrastructure::response_json_parsers::{classify_stream_or_response_error, extract_responses_usage, parse_responses_json, ToolSlot};
use crate::infrastructure::response_json_parsers::{FastDelta, SseFastFrame};

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
    use_fast_delta: bool,
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
            line_buf: Vec::with_capacity(512),
            current_event: None,
            data_buf: String::with_capacity(1024),
            has_data: false,
            overflowed: false,
            text_acc: String::with_capacity(2048),
            thought_fallback: String::new(),
            tool_map: BTreeMap::new(),
            item_id_to_index: BTreeMap::new(),
            latest_usage: None,
            anthropic_in_tokens: 0,
            anthropic_cache_read: 0,
            anthropic_out_tokens: 0,
            saw_anthropic_usage: false,
            completed_override: None,
            use_fast_delta: true,
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
        while let Some(rel_idx) = bytes[start..].iter().position(|&b| b == b'\n') {
            let idx = start + rel_idx;
            let slice = &bytes[start..idx];
            if self.line_buf.is_empty() {
                if !self.has_data
                    && !self.overflowed
                    && self.current_event.is_none()
                    && slice.starts_with(b"data:")
                {
                    let next = &bytes[idx + 1..];
                    if next.starts_with(b"\n") || next.starts_with(b"\r\n") {
                        let skip = if next.starts_with(b"\r\n") { 2 } else { 1 };
                        let payload = slice[5..].strip_prefix(b" ").unwrap_or(&slice[5..]);
                        let payload = payload.strip_suffix(b"\r").unwrap_or(payload);
                        if payload.len() <= MAX_SSE_EVENT_DATA_BYTES {
                            if let Ok(s) = std::str::from_utf8(payload) {
                                let trimmed = s.trim();
                                if !trimmed.is_empty() && trimmed != "[DONE]" {
                                    self.ingest_frame(None, trimmed)?;
                                }
                                start = idx + 1 + skip;
                                continue;
                            }
                        }
                    }
                }
                self.process_raw_line(slice)?;
            } else {
                self.line_buf.extend_from_slice(slice);
                let mut taken = std::mem::take(&mut self.line_buf);
                let res = self.process_raw_line(&taken);
                taken.clear();
                self.line_buf = taken;
                res?;
            }
            start = idx + 1;
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
                if self.has_data && !self.overflowed {
                    let trimmed_buf = self.data_buf.trim();
                    if (trimmed_buf.ends_with('}') || trimmed_buf.ends_with(']'))
                        && serde_json::from_str::<IgnoredAny>(trimmed_buf).is_ok()
                    {
                        self.flush_event()?;
                    }
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

        if overflowed {
            self.data_buf.clear();
            return Err(ProviderError::Transport(format!(
                "SSE event exceeded {} byte frame cap",
                MAX_SSE_EVENT_DATA_BYTES
            )));
        }
        if !had_data {
            self.data_buf.clear();
            return Ok(());
        }
        let mut raw_data = std::mem::take(&mut self.data_buf);
        let trimmed = raw_data.trim();
        let res = if trimmed.is_empty() || trimmed == "[DONE]" {
            Ok(())
        } else {
            self.ingest_frame(event.as_deref(), trimmed)
        };
        raw_data.clear();
        self.data_buf = raw_data;
        res
    }

    fn try_ingest_fast_delta(&mut self, frame_event: Option<&str>, raw_data: &str) -> bool {
        let Ok(fast) = serde_json::from_str::<SseFastFrame<'_>>(raw_data) else {
            return false;
        };
        if fast.error.is_some() || fast.usage.is_some() || fast.candidates.is_some() {
            return false;
        }
        let ev_type = fast
            .ev_type
            .as_deref()
            .or(frame_event)
            .unwrap_or("");
        match ev_type {
            "content_block_delta" => {
                let idx = fast.index.unwrap_or(0);
                if let Some(FastDelta::Anthropic { kind: Some(k), text, partial_json }) = fast.delta {
                    match k.as_ref() {
                        "text_delta" => {
                            if let Some(t) = text {
                                self.text_acc.push_str(&t);
                            }
                            return true;
                        }
                        "input_json_delta" => {
                            if let Some(pj) = partial_json {
                                let entry = self.tool_map.entry(idx).or_insert_with(|| {
                                    (format!("toolu_{}", idx + 1), String::new(), String::new(), None)
                                });
                                entry.2.push_str(&pj);
                            }
                            return true;
                        }
                        _ => return false,
                    }
                }
                false
            }
            "response.output_text.delta" => {
                if let Some(FastDelta::Str(d)) = fast.delta {
                    self.text_acc.push_str(&d);
                    return true;
                }
                false
            }
            "response.function_call_arguments.delta" => {
                if let Some(FastDelta::Str(d)) = fast.delta {
                    let idx = fast
                        .output_index
                        .or_else(|| fast.item_id.as_deref().and_then(|id| self.item_id_to_index.get(id).copied()))
                        .unwrap_or(0);
                    let entry = self.tool_map.entry(idx).or_insert_with(|| {
                        (format!("call_{}", idx + 1), String::new(), String::new(), None)
                    });
                    entry.2.push_str(&d);
                    return true;
                }
                false
            }
            "" => {
                if let Some(Some(choice)) = fast.choices.map(|c| c.0) {
                    if choice.message.is_some() {
                        return false;
                    }
                    if let Some(delta) = choice.delta {
                        if delta.tool_calls.is_some() {
                            return false;
                        }
                        if let Some(txt) = delta.content {
                            self.text_acc.push_str(&txt);
                        }
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn ingest_frame(
        &mut self,
        frame_event: Option<&str>,
        raw_data: &str,
    ) -> Result<(), ProviderError> {
        if self.completed_override.is_some() {
            return Ok(());
        }
        if self.use_fast_delta && self.try_ingest_fast_delta(frame_event, raw_data) {
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
        if matches!(
            ev_type,
            "message_start" | "content_block_start" | "content_block_delta" | "message_delta"
        ) {
            match ev_type {
                "message_start" => {
                    if let Some(u) = chunk.get("message").and_then(|m| m.get("usage")) {
                        let uncached =
                            u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
                        let cr = u
                            .get("cache_read_input_tokens")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as usize;
                        let cc = u
                            .get("cache_creation_input_tokens")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as usize;
                        self.anthropic_in_tokens = uncached + cr + cc;
                        self.anthropic_cache_read = cr;
                        self.anthropic_out_tokens =
                            u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
                        self.saw_anthropic_usage = true;
                    }
                }
                "content_block_start" => {
                    let idx = chunk.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                    if let Some(cb) = chunk.get("content_block") {
                        match cb.get("type").and_then(Value::as_str) {
                            Some("tool_use") => {
                                let id = cb
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string();
                                let name = cb
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string();
                                let entry = self.tool_map.entry(idx).or_insert_with(|| {
                                    (
                                        format!("toolu_{}", idx + 1),
                                        String::new(),
                                        String::new(),
                                        None,
                                    )
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
                                if let Some(pj) = delta.get("partial_json").and_then(Value::as_str)
                                {
                                    let entry = self.tool_map.entry(idx).or_insert_with(|| {
                                        (
                                            format!("toolu_{}", idx + 1),
                                            String::new(),
                                            String::new(),
                                            None,
                                        )
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
                    let idx = chunk
                        .get("output_index")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as usize;
                    if let Some(item) = chunk.get("item") {
                        if item.get("type").and_then(Value::as_str) == Some("function_call") {
                            let call_id = item
                                .get("call_id")
                                .or_else(|| item.get("id"))
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            if let Some(item_id) = item.get("id").and_then(Value::as_str) {
                                self.item_id_to_index.insert(item_id.to_string(), idx);
                            }
                            let name = item
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            let entry = self.tool_map.entry(idx).or_insert_with(|| {
                                (
                                    format!("call_{}", idx + 1),
                                    String::new(),
                                    String::new(),
                                    None,
                                )
                            });
                            if !call_id.is_empty() {
                                entry.0 = call_id;
                            }
                            if !name.is_empty() {
                                entry.1 = name;
                            }
                            match item.get("arguments") {
                                Some(Value::String(s)) if !s.is_empty() => entry.2 = s.clone(),
                                Some(other) if !other.is_string() && !other.is_null() => {
                                    entry.2 = other.to_string()
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "response.function_call_arguments.delta" => {
                    let idx = chunk
                        .get("output_index")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize)
                        .or_else(|| {
                            chunk
                                .get("item_id")
                                .and_then(Value::as_str)
                                .and_then(|id| self.item_id_to_index.get(id).copied())
                        })
                        .unwrap_or(0);
                    if let Some(d) = chunk.get("delta").and_then(Value::as_str) {
                        let entry = self.tool_map.entry(idx).or_insert_with(|| {
                            (
                                format!("call_{}", idx + 1),
                                String::new(),
                                String::new(),
                                None,
                            )
                        });
                        entry.2.push_str(d);
                    }
                }
                "response.function_call_arguments.done" => {
                    let idx = chunk
                        .get("output_index")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize)
                        .or_else(|| {
                            chunk
                                .get("item_id")
                                .and_then(Value::as_str)
                                .and_then(|id| self.item_id_to_index.get(id).copied())
                        })
                        .unwrap_or(0);
                    if let Some(args) = chunk
                        .get("arguments")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                    {
                        let entry = self.tool_map.entry(idx).or_insert_with(|| {
                            (
                                format!("call_{}", idx + 1),
                                String::new(),
                                String::new(),
                                None,
                            )
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
                            self.completed_override =
                                Some(parse_responses_json(resp_obj, &self.provider_label)?);
                            return Ok(());
                        }
                        if self.tool_map.is_empty() {
                            if let Some(output) = resp_obj.get("output").and_then(Value::as_array) {
                                for (idx, item) in output.iter().enumerate() {
                                    if item.get("type").and_then(Value::as_str)
                                        == Some("function_call")
                                    {
                                        let cid = item
                                            .get("call_id")
                                            .or_else(|| item.get("id"))
                                            .and_then(Value::as_str)
                                            .unwrap_or("")
                                            .to_string();
                                        let name = item
                                            .get("name")
                                            .and_then(Value::as_str)
                                            .unwrap_or("")
                                            .to_string();
                                        let args = match item.get("arguments") {
                                            Some(Value::String(s)) => s.clone(),
                                            Some(other) if !other.is_null() => other.to_string(),
                                            _ => "{}".to_string(),
                                        };
                                        if !name.is_empty() {
                                            self.tool_map.insert(
                                                idx,
                                                (
                                                    if cid.is_empty() {
                                                        format!("call_{}", idx + 1)
                                                    } else {
                                                        cid
                                                    },
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

        // 3. Google Gemini SSE (`candidates[0]`)
        if let Some(cand) = chunk
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
        {
            if let Some(parts) = cand.pointer("/content/parts").and_then(Value::as_array) {
                for part in parts {
                    let is_thought = part.get("thought").and_then(Value::as_bool) == Some(true);
                    if let Some(t) = part.get("text").and_then(Value::as_str) {
                        if is_thought {
                            self.thought_fallback.push_str(t);
                        } else {
                            self.text_acc.push_str(t);
                        }
                    }
                    if let Some(fc) = part.get("functionCall") {
                        let name = fc
                            .get("name")
                            .and_then(Value::as_str)
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
                            let next_idx = self.tool_map.len();
                            self.tool_map.insert(
                                next_idx,
                                (format!("gemini_call_{}", next_idx + 1), name, args, sig),
                            );
                        }
                    }
                }
            }
            if let Some(um) = chunk.get("usageMetadata") {
                self.latest_usage = Some(ProviderTokenUsage {
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
            if let Some(reason) = cand.get("finishReason").and_then(Value::as_str) {
                if !matches!(reason, "STOP" | "MAX_TOKENS") {
                    return Err(ProviderError::HttpStatus {
                        status: 200,
                        body: format!("Gemini candidate blocked: finishReason = {reason}"),
                        retryable: false,
                        retry_after_ms: None,
                    });
                }
            }
            return Ok(());
        }

        // 4. OpenAI Chat Completions SSE (`choices[0].delta`)
        if let Some(u) = (self.extract_openai_usage_fn)(&chunk) {
            self.latest_usage = Some(u);
        }
        if let Some(choice) = chunk
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
        {
            let delta = choice
                .get("delta")
                .or_else(|| choice.get("message"))
                .unwrap_or(choice);
            if let Some(txt) = delta.get("content").and_then(|v| v.as_str()) {
                self.text_acc.push_str(txt);
            }
            if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                for (i, tc) in tcs.iter().enumerate() {
                    let idx = tc
                        .get("index")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as usize)
                        .unwrap_or(i);
                    let entry = self.tool_map.entry(idx).or_insert_with(|| {
                        (
                            format!("call_{}", idx + 1),
                            String::new(),
                            String::new(),
                            None,
                        )
                    });
                    if let Some(id) = tc
                        .get("id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                    {
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
                        match func.get("arguments") {
                            Some(Value::String(arg_chunk)) => {
                                let (_, next_acc) =
                                    compute_streaming_arguments_delta(&entry.2, arg_chunk);
                                entry.2 = next_acc;
                            }
                            Some(other) if !other.is_string() && !other.is_null() => {
                                let (_, next_acc) =
                                    compute_streaming_arguments_delta(&entry.2, &other.to_string());
                                entry.2 = next_acc;
                            }
                            _ => {}
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
                    arguments: if args.is_empty() {
                        "{}".to_string()
                    } else {
                        args
                    },
                },
                thought_signature,
            })
            .collect();

        if self.text_acc.is_empty() && tool_calls.is_empty() && !self.thought_fallback.is_empty() {
            self.text_acc = self.thought_fallback;
        }

        let mut resp =
            finalize_provider_response(Some(self.text_acc), tool_calls, &self.provider_label);
        resp.usage = self.latest_usage;
        Ok(resp)
    }
}

/// Unified multi-dialect SSE stream parser supporting OpenAI Chat Completions,
/// OpenAI `/v1/responses`, Anthropic `/v1/messages`, and Google Gemini SSE streams.
pub(crate) fn parse_multidialect_sse_stream(raw_sse: &str, provider_label: &str, extract_openai_usage_fn: fn(&Value) -> Option<ProviderTokenUsage>) -> Result<ProviderResponse, ProviderError> {
    let mut agg = IncrementalSseAggregator::new(provider_label, extract_openai_usage_fn, usize::MAX);
    agg.push_bytes(raw_sse.as_bytes())?;
    agg.finish()
}

pub(crate) fn parse_multidialect_sse_stream_value_only(raw_sse: &str, provider_label: &str, extract_openai_usage_fn: fn(&Value) -> Option<ProviderTokenUsage>) -> Result<ProviderResponse, ProviderError> {
    let mut agg = IncrementalSseAggregator::new(provider_label, extract_openai_usage_fn, usize::MAX);
    agg.use_fast_delta = false;
    agg.push_bytes(raw_sse.as_bytes())?;
    agg.finish()
}
