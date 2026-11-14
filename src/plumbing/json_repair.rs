// [LAYER: PLUMBING]
use serde_json::Value;

pub type JsonRootsExtraction = (Vec<Value>, String);
pub type StreamingDeltaPair = (String, String);

/// Extracts top-level balanced JSON roots from a raw string (`ADR-0014`).
pub fn extract_json_roots(input: &str) -> (Vec<Value>, String) {
    let (mut roots, mut remainder) = (Vec::new(), String::new());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] != b'{' && bytes[i] != b'[' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'{' && bytes[i] != b'[' {
                i += 1;
            }
            remainder.push_str(&input[start..i]);
            continue;
        }
        let (start, mut depth, mut in_str, mut esc, mut end_idx) = (i, 0i32, false, false, None);
        while i < bytes.len() {
            let ch = bytes[i];
            if in_str {
                if esc {
                    esc = false;
                } else if ch == b'\\' {
                    esc = true;
                } else if ch == b'"' {
                    in_str = false;
                }
            } else {
                match ch {
                    b'"' => in_str = true,
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            end_idx = Some(i + 1);
                            i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        if let Some(end) = end_idx {
            if let Ok(val) = serde_json::from_str::<Value>(&input[start..end]) {
                roots.push(val);
            } else {
                remainder.push_str(&input[start..end]);
            }
        } else {
            remainder.push_str(&input[start..]);
            break;
        }
    }
    (roots, remainder)
}

/// Repairs common LLM JSON syntax errors (Python literals, single quotes, trailing commas, unclosed delimiters).
pub fn repair_json_string(input: &str) -> String {
    let trimmed = input.trim();
    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return trimmed.to_string();
    }
    let mut text = trimmed
        .replace(": True", ": true")
        .replace(":True", ": true")
        .replace(": False", ": false")
        .replace(":False", ": false")
        .replace(": None", ": null")
        .replace(":None", ": null");
    if !text.contains('"') && text.contains('\'') {
        text = text.replace('\'', "\"");
    }
    let mut cleaned = String::with_capacity(text.len() + 4);
    let chars: Vec<char> = text.chars().collect();
    let (mut i, mut in_str, mut esc) = (0, false, false);
    let (mut open_braces, mut close_braces, mut open_brackets, mut close_brackets) = (0usize, 0usize, 0usize, 0usize);
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            cleaned.push(c);
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            cleaned.push(c);
            i += 1;
            continue;
        }
        if c == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j == chars.len() || chars[j] == '}' || chars[j] == ']' {
                i += 1;
                continue;
            }
        }
        match c {
            '{' => open_braces += 1,
            '}' => close_braces += 1,
            '[' => open_brackets += 1,
            ']' => close_brackets += 1,
            _ => {}
        }
        cleaned.push(c);
        i += 1;
    }
    for _ in 0..open_braces.saturating_sub(close_braces) {
        cleaned.push('}');
    }
    for _ in 0..open_brackets.saturating_sub(close_brackets) {
        cleaned.push(']');
    }
    cleaned
}

/// Deduplicates streaming tool names when providers emit both incremental deltas and terminal snapshots.
pub fn accumulate_streaming_tool_name(accumulated: &str, incoming: &str) -> String {
    if incoming.is_empty() {
        return accumulated.to_string();
    }
    if accumulated.is_empty() || incoming == accumulated || incoming.starts_with(accumulated) {
        return incoming.to_string();
    }
    format!("{accumulated}{incoming}")
}

/// Computes incremental argument delta from streaming tool call chunks (`ADR-0014`).
pub fn compute_streaming_arguments_delta(accumulated: &str, incoming: &str) -> (String, String) {
    if incoming.is_empty() || incoming == accumulated {
        return (String::new(), accumulated.to_string());
    }
    if accumulated.is_empty() {
        return (incoming.to_string(), incoming.to_string());
    }
    if incoming.starts_with(accumulated) {
        let can_slice = accumulated.len() >= 2 || {
            let (roots, rem) = extract_json_roots(incoming);
            accumulated == "{" && roots.len() == 1 && rem.trim().is_empty() && roots[0].is_object()
        };
        if can_slice {
            return (incoming[accumulated.len()..].to_string(), incoming.to_string());
        }
    }
    if accumulated.len() >= 2 {
        let (ex_roots, ex_rem) = extract_json_roots(accumulated);
        if ex_roots.len() == 1 && ex_rem.trim().is_empty() && ex_roots[0].is_object() {
            let (in_roots, in_rem) = extract_json_roots(incoming);
            if in_roots.len() == 1 && in_rem.trim().is_empty() && in_roots[0] == ex_roots[0] {
                return (String::new(), accumulated.to_string());
            }
        }
    }
    (incoming.to_string(), format!("{accumulated}{incoming}"))
}

/// Validates and self-heals completed streaming tool call arguments (`ADR-0014`).
pub fn normalize_completed_tool_call_arguments(raw: &str, provider_label: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok("{}".to_string());
    }
    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        if parsed.is_object() {
            return Ok(trimmed.to_string());
        }
        return Err(format!("{provider_label} returned non-object tool arguments"));
    }
    let (roots, rem) = extract_json_roots(trimmed);
    if roots.len() > 1 && rem.trim().is_empty() {
        let first = &roots[0];
        if first.is_object() && roots.iter().all(|r| r == first) {
            return serde_json::to_string(first).map_err(|e| e.to_string());
        }
        return Err(format!(
            "{provider_label} returned {} conflicting concatenated JSON payloads",
            roots.len()
        ));
    }
    Err(format!("{provider_label} returned invalid tool arguments"))
}
