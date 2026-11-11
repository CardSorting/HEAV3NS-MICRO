// [LAYER: PLUMBING]
use serde::de::IgnoredAny;
use serde_json::{Map, Value};

/// Hard cap (8 MiB) on accumulated `data:` payload per SSE event frame to prevent OOM on malformed streams.
pub const MAX_SSE_EVENT_DATA_BYTES: usize = 8 * 1024 * 1024;

/// Parsed Server-Sent Events (`text/event-stream`) frame with optional `event:` tag and joined `data:` lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEventFrame {
    pub event: Option<String>,
    pub data: String,
}

/// Spec-compliant SSE frame parser supporting `\r\n`/`\n`, multi-line `data:`, `event:` headers,
/// `: comment` heartbeats (OpenRouter/Anthropic), `[DONE]` sentinels, and 8 MiB frame bounds.
pub fn parse_sse_events(raw_sse: &str) -> Vec<SseEventFrame> {
    let mut frames = Vec::new();
    let mut current_event: Option<String> = None;
    let mut data_buf = String::new();
    let mut has_data = false;
    let mut overflowed = false;

    let flush = |frames: &mut Vec<SseEventFrame>,
                 current_event: &mut Option<String>,
                 data_buf: &mut String,
                 has_data: &mut bool,
                 overflowed: &mut bool| {
        if *has_data && !*overflowed {
            let trimmed = data_buf.trim();
            if !trimmed.is_empty() && trimmed != "[DONE]" {
                frames.push(SseEventFrame {
                    event: current_event.take(),
                    data: data_buf.clone(),
                });
            }
        }
        *current_event = None;
        data_buf.clear();
        *has_data = false;
        *overflowed = false;
    };

    for raw_line in raw_sse.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() {
            flush(&mut frames, &mut current_event, &mut data_buf, &mut has_data, &mut overflowed);
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        let (field, raw_val) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => {
                if has_data || overflowed {
                    flush(&mut frames, &mut current_event, &mut data_buf, &mut has_data, &mut overflowed);
                }
                current_event = Some(raw_val.trim().to_string());
            }
            "data" => {
                if raw_val.trim() == "[DONE]" {
                    flush(&mut frames, &mut current_event, &mut data_buf, &mut has_data, &mut overflowed);
                    continue;
                }
                if has_data
                    && !overflowed
                    && serde_json::from_str::<IgnoredAny>(data_buf.trim()).is_ok()
                {
                    flush(&mut frames, &mut current_event, &mut data_buf, &mut has_data, &mut overflowed);
                }
                if overflowed {
                    continue;
                }
                let needed = raw_val.len() + usize::from(has_data);
                if data_buf.len() + needed > MAX_SSE_EVENT_DATA_BYTES {
                    overflowed = true;
                    data_buf.clear();
                    continue;
                }
                if has_data {
                    data_buf.push('\n');
                }
                data_buf.push_str(raw_val);
                has_data = true;
            }
            _ => {}
        }
    }
    flush(&mut frames, &mut current_event, &mut data_buf, &mut has_data, &mut overflowed);
    frames
}

/// Redacts an HTTP endpoint down to `scheme://host[:port]` so path-embedded tokens,
/// userinfo credentials, and query parameters (`?key=...`) never leak into manifests or logs.
pub fn redact_endpoint_for_display(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return "(unset)".to_string();
    }
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return "(invalid-endpoint)".to_string();
    };
    let authority_and_path = rest.split(['?', '#']).next().unwrap_or(rest);
    let authority = authority_and_path.split('/').next().unwrap_or(authority_and_path);
    let host_port = authority.rsplit_once('@').map(|(_, hp)| hp).unwrap_or(authority);
    if host_port.is_empty() {
        return format!("{scheme}://(invalid-host)");
    }
    format!("{}://{}", scheme.to_ascii_lowercase(), host_port)
}

/// Sanitizes a JSON Schema in-place for strict LLM tool-calling validators
/// (OpenAI, DeepSeek, Anthropic, OpenRouter, Gemini, Mistral, xAI, Qwen):
/// 1. Collapses Draft 2020-12 nullable type arrays (`"type": ["string", "null"]` -> `"type": "string"`).
/// 2. Collapses nullable `anyOf`/`oneOf: [T, {"type": "null"}]` and single-element `[T]` into `T`.
/// 3. Injects `"items"` on bare `"type": "array"` schemas so strict validators never return HTTP 400.
/// 4. Injects `"properties": {}` on bare `"type": "object"` schemas lacking `additionalProperties` object maps.
/// 5. Prunes dangling keys in `"required"` that do not exist in `"properties"`.
pub fn sanitize_tool_schema(schema: &mut Value) {
    let Some(map) = schema.as_object_mut() else {
        return;
    };

    if let Some(collapsed_type) = map.get("type").and_then(Value::as_array).and_then(|arr| {
        let non_null: Vec<&str> = arr
            .iter()
            .filter_map(Value::as_str)
            .filter(|t| *t != "null")
            .collect();
        (non_null.len() == 1).then(|| non_null[0].to_string())
    }) {
        map.insert("type".to_string(), Value::String(collapsed_type));
    }

    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(Value::Array(items)) = map.get_mut(key) {
            for item in items.iter_mut() {
                sanitize_tool_schema(item);
            }
        }
    }

    for union_key in ["anyOf", "oneOf"] {
        let collapsed = map.get(union_key).and_then(Value::as_array).and_then(|variants| {
            if variants.len() == 1 {
                return variants.first().and_then(Value::as_object).cloned();
            }
            if variants.len() == 2 {
                let non_null: Vec<&Value> = variants.iter().filter(|v| !is_null_type_schema(v)).collect();
                if non_null.len() == 1 {
                    return non_null[0].as_object().cloned();
                }
            }
            None
        });
        if let Some(replacement) = collapsed {
            map.remove(union_key);
            for (k, v) in replacement {
                map.entry(k).or_insert(v);
            }
        }
    }

    if let Some(Value::Object(props)) = map.get_mut("properties") {
        for prop_schema in props.values_mut() {
            sanitize_tool_schema(prop_schema);
        }
    }
    if let Some(items) = map.get_mut("items") {
        match items {
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    sanitize_tool_schema(item);
                }
            }
            other => sanitize_tool_schema(other),
        }
    } else if map.get("type").and_then(Value::as_str) == Some("array") {
        let desc = map
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_lowercase();
        let inferred_items = if desc.contains('{') || desc.contains("object") {
            serde_json::json!({ "type": "object", "properties": {} })
        } else if desc.contains("[x, y]") || desc.contains("coordinate") || desc.contains("number") {
            serde_json::json!({ "type": "number" })
        } else {
            serde_json::json!({ "type": "string" })
        };
        map.insert("items".to_string(), inferred_items);
    }
    if let Some(ap) = map.get_mut("additionalProperties") {
        if ap.is_object() {
            sanitize_tool_schema(ap);
        }
    }

    let is_object_type = map.get("type").and_then(Value::as_str) == Some("object");
    let has_record_map = matches!(map.get("additionalProperties"), Some(Value::Object(_)));
    if is_object_type && !map.contains_key("properties") && !has_record_map {
        map.insert("properties".to_string(), Value::Object(Map::new()));
    }

    if let Some(prop_keys) = map
        .get("properties")
        .and_then(Value::as_object)
        .map(|p| p.keys().cloned().collect::<Vec<String>>())
    {
        if let Some(Value::Array(req)) = map.get_mut("required") {
            req.retain(|entry| entry.as_str().is_some_and(|k| prop_keys.iter().any(|pk| pk == k)));
        }
    }
}

/// Sanitizes a tool parameter JSON Schema for `/v1/responses` and xAI Grok endpoints,
/// which additionally forbid top-level `oneOf`/`anyOf`/`allOf` combinators and require root `"type": "object"`.
pub fn sanitize_tool_schema_for_responses(schema: &mut Value) {
    sanitize_tool_schema(schema);
    let Some(map) = schema.as_object_mut() else {
        *schema = serde_json::json!({ "type": "object", "properties": {} });
        return;
    };

    for root_union in ["oneOf", "anyOf", "allOf"] {
        let Some(Value::Array(branches)) = map.remove(root_union) else {
            continue;
        };
        let mut merged_props = map
            .remove("properties")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        let mut common_required: Option<Vec<String>> = None;

        for branch in &branches {
            let Some(bmap) = branch.as_object() else {
                continue;
            };
            if let Some(Value::Object(bprops)) = bmap.get("properties") {
                for (k, v) in bprops {
                    merged_props.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            let branch_req: Vec<String> = bmap
                .get("required")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).map(String::from).collect())
                .unwrap_or_default();
            common_required = Some(match common_required {
                None => branch_req,
                Some(prev) => prev.into_iter().filter(|k| branch_req.contains(k)).collect(),
            });
        }

        map.insert("type".to_string(), Value::String("object".to_string()));
        map.insert("properties".to_string(), Value::Object(merged_props));
        if let Some(req) = common_required {
            let valid_req: Vec<Value> = req
                .into_iter()
                .filter(|k| {
                    map.get("properties")
                        .and_then(Value::as_object)
                        .is_some_and(|p| p.contains_key(k))
                })
                .map(Value::String)
                .collect();
            if !valid_req.is_empty() {
                map.insert("required".to_string(), Value::Array(valid_req));
            } else {
                map.remove("required");
            }
        }
    }

    if map.get("type").and_then(Value::as_str) != Some("object") {
        map.insert("type".to_string(), Value::String("object".to_string()));
    }
    if !map.contains_key("properties") {
        map.insert("properties".to_string(), Value::Object(Map::new()));
    }
}

/// Sanitizes a tool parameter JSON Schema for Google Gemini `functionDeclarations.parameters`,
/// flattening root combinators and recursively stripping unsupported `$schema` / `additionalProperties` keys.
pub fn sanitize_tool_schema_for_gemini(schema: &mut Value) {
    sanitize_tool_schema_for_responses(schema);
    strip_gemini_unsupported_keys(schema);
}

fn strip_gemini_unsupported_keys(node: &mut Value) {
    match node {
        Value::Object(map) => {
            map.remove("$schema");
            map.remove("additionalProperties");
            map.remove("strict");
            for child in map.values_mut() {
                strip_gemini_unsupported_keys(child);
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                strip_gemini_unsupported_keys(item);
            }
        }
        _ => {}
    }
}

fn is_null_type_schema(v: &Value) -> bool {
    v.as_object()
        .and_then(|m| m.get("type"))
        .and_then(Value::as_str)
        == Some("null")
}
