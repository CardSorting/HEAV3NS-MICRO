// [LAYER: INFRASTRUCTURE]
use crate::domain::types::messages::{ToolCall, ToolCallFunction};
use crate::domain::types::tick_result::{
    finalize_provider_response, is_retryable_http_status, ProviderError, ProviderResponse,
    ProviderTokenUsage,
};
use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use serde_json::Value;
use std::borrow::Cow;
use std::fmt;

pub(crate) enum FastDelta<'a> {
    Str(Cow<'a, str>),
    Anthropic {
        kind: Option<Cow<'a, str>>,
        text: Option<Cow<'a, str>>,
        partial_json: Option<Cow<'a, str>>,
    },
}

impl<'de: 'a, 'a> Deserialize<'de> for FastDelta<'a> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DeltaVisitor;
        impl<'de> Visitor<'de> for DeltaVisitor {
            type Value = FastDelta<'de>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("string or anthropic delta object")
            }
            fn visit_borrowed_str<E: de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
                Ok(FastDelta::Str(Cow::Borrowed(v)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(FastDelta::Str(Cow::Owned(v.to_owned())))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let (mut kind, mut text, mut partial_json) = (None, None, None);
                while let Some(k) = map.next_key::<Cow<'de, str>>()? {
                    match k.as_ref() {
                        "type" => kind = map.next_value()?,
                        "text" => text = map.next_value()?,
                        "partial_json" => partial_json = map.next_value()?,
                        _ => {
                            let _: IgnoredAny = map.next_value()?;
                        }
                    }
                }
                Ok(FastDelta::Anthropic { kind, text, partial_json })
            }
        }
        deserializer.deserialize_any(DeltaVisitor)
    }
}

#[derive(Deserialize)]
pub(crate) struct FastChatDelta<'a> {
    #[serde(borrow)]
    pub(crate) content: Option<Cow<'a, str>>,
    pub(crate) tool_calls: Option<IgnoredAny>,
}

#[derive(Deserialize)]
pub(crate) struct FastChoice<'a> {
    #[serde(borrow)]
    pub(crate) delta: Option<FastChatDelta<'a>>,
    pub(crate) message: Option<IgnoredAny>,
}

pub(crate) struct FastFirstChoice<'a>(pub(crate) Option<FastChoice<'a>>);

impl<'de: 'a, 'a> Deserialize<'de> for FastFirstChoice<'a> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SeqVisitor;
        impl<'de> Visitor<'de> for SeqVisitor {
            type Value = FastFirstChoice<'de>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("choices array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let first = seq.next_element::<FastChoice<'de>>()?;
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(FastFirstChoice(first))
            }
        }
        deserializer.deserialize_seq(SeqVisitor)
    }
}

#[derive(Deserialize)]
pub(crate) struct SseFastFrame<'a> {
    #[serde(borrow, rename = "type")]
    pub(crate) ev_type: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub(crate) delta: Option<FastDelta<'a>>,
    pub(crate) output_index: Option<usize>,
    #[serde(borrow)]
    pub(crate) item_id: Option<Cow<'a, str>>,
    pub(crate) index: Option<usize>,
    #[serde(borrow)]
    pub(crate) choices: Option<FastFirstChoice<'a>>,
    pub(crate) candidates: Option<IgnoredAny>,
    pub(crate) error: Option<IgnoredAny>,
    pub(crate) usage: Option<IgnoredAny>,
}

pub(crate) fn classify_stream_or_response_error(
    chunk: &Value,
    ev_type: &str,
    raw_body: &str,
) -> Option<ProviderError> {
    let err_obj = chunk.get("error").filter(|v| !v.is_null()).or_else(|| {
        chunk
            .get("response")
            .and_then(|r| r.get("error"))
            .filter(|v| !v.is_null())
    });

    let is_explicit_error_event = ev_type == "error"
        || ev_type == "response.failed"
        || chunk.get("status").and_then(Value::as_str) == Some("failed");
    let is_top_level_error_chunk =
        err_obj.is_some() && chunk.get("choices").is_none() && chunk.get("candidates").is_none();

    if !is_explicit_error_event && !is_top_level_error_chunk {
        return None;
    }

    let explicit_status = err_obj
        .and_then(|e| e.get("status").or_else(|| e.get("code")))
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
        })
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
        } else if code_or_type.contains("rate_limit") || code_or_type.contains("too_many_requests")
        {
            429
        } else if code_or_type.contains("insufficient_quota")
            || code_or_type.contains("billing")
            || code_or_type.contains("credit_balance")
        {
            402
        } else if code_or_type.contains("authentication")
            || code_or_type.contains("invalid_api_key")
        {
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
pub(crate) fn parse_responses_json(
    body: &Value,
    provider_label: &str,
) -> Result<ProviderResponse, ProviderError> {
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
                            if let Some(t) = part
                                .get("text")
                                .and_then(Value::as_str)
                                .filter(|s| !s.is_empty())
                            {
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
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    let arguments = match item.get("arguments") {
                        Some(Value::String(s)) => s.clone(),
                        Some(other) if !other.is_null() => other.to_string(),
                        _ => "{}".to_string(),
                    };
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
        if let Some(ot) = root
            .get("output_text")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            text_parts.push(ot.to_string());
        }
    }

    let mut resp =
        finalize_provider_response(Some(text_parts.join("")), tool_calls, provider_label);
    resp.usage = extract_responses_usage(root);
    Ok(resp)
}

pub(crate) fn extract_responses_usage(root: &Value) -> Option<ProviderTokenUsage> {
    let u = root.get("usage")?;
    let prompt_tokens = u
        .get("input_tokens")
        .or_else(|| u.get("prompt_tokens"))
        .and_then(Value::as_u64)? as usize;
    let completion_tokens = u
        .get("output_tokens")
        .or_else(|| u.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let cached_prompt_tokens = u
        .get("input_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .or_else(|| {
            u.get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
        })
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    Some(ProviderTokenUsage {
        prompt_tokens,
        completion_tokens,
        cached_prompt_tokens,
        is_exact: true,
    })
}

pub(crate) type ToolSlot = (String, String, String, Option<String>);

