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
    let mut delim_stack: Vec<char> = Vec::new();
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
            '{' => delim_stack.push('}'),
            '[' => delim_stack.push(']'),
            '}' | ']' => {
                if delim_stack.last() == Some(&c) {
                    delim_stack.pop();
                }
            }
            _ => {}
        }
        cleaned.push(c);
        i += 1;
    }
    if in_str {
        if esc {
            cleaned.push('\\');
        }
        cleaned.push('"');
    }
    while let Some(closing) = delim_stack.pop() {
        cleaned.push(closing);
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
    if accumulated.trim() == "{}" && incoming.trim_start().starts_with('{') {
        return (incoming.to_string(), incoming.to_string());
    }
    if incoming.starts_with(accumulated) && incoming.len() > accumulated.len() {
        return (
            incoming[accumulated.len()..].to_string(),
            incoming.to_string(),
        );
    }
    if accumulated.len() >= 2 && accumulated.trim_end().ends_with('}') {
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
pub fn normalize_completed_tool_call_arguments(
    raw: &str,
    provider_label: &str,
) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok("{}".to_string());
    }
    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        if parsed.is_object() {
            return Ok(trimmed.to_string());
        }
        return Err(format!(
            "{provider_label} returned non-object tool arguments"
        ));
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

/// Extracts a partial or complete string value for `key` from streaming JSON tool arguments.
fn extract_partial_json_string_field(raw: &str, keys: &[&str]) -> Option<String> {
    for key in keys {
        let needle = format!("\"{key}\"");
        let Some(pos) = raw.find(&needle) else { continue };
        let after_key = &raw[pos + needle.len()..];
        let trimmed = after_key.trim_start();
        let Some(after_colon) = trimmed.strip_prefix(':') else { continue };
        let after_ws = after_colon.trim_start();
        let Some(rest) = after_ws.strip_prefix('"') else { continue };
        let mut out = String::new();
        let mut chars = rest.chars();
        while let Some(ch) = chars.next() {
            if ch == '"' {
                return Some(out);
            }
            if ch == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some(other) => out.push(other),
                    None => break,
                }
            } else {
                out.push(ch);
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

/// Synthesizes a live unified-diff preview from streaming partial tool-call arguments
/// for file-editing tools (`edit_file`, `replace_file_content`, `write_file`, `apply_patch`).
pub fn synthesize_streaming_tool_diff(
    tool_name: &str,
    partial_args: &str,
) -> Option<(Option<String>, String)> {
    let lower = tool_name.to_ascii_lowercase();
    let is_edit_tool = lower.contains("edit")
        || lower.contains("replace")
        || lower.contains("write")
        || lower.contains("patch");
    if !is_edit_tool {
        return None;
    }
    let path = extract_partial_json_string_field(
        partial_args,
        &["Path", "path", "file_path", "TargetFile", "target_file"],
    );
    if let Some(raw_diff) =
        extract_partial_json_string_field(partial_args, &["diff", "patch", "patchText"])
    {
        if raw_diff.contains('\n') || raw_diff.starts_with('-') || raw_diff.starts_with('+') {
            return Some((path, raw_diff));
        }
    }
    let target = extract_partial_json_string_field(
        partial_args,
        &["Target", "target", "TargetContent", "old_string"],
    );
    let replacement = extract_partial_json_string_field(
        partial_args,
        &[
            "Replacement",
            "replacement",
            "ReplacementContent",
            "new_string",
            "Content",
            "content",
        ],
    );
    if target.is_none() && replacement.is_none() {
        return None;
    }
    let label = path.as_deref().unwrap_or("workspace_edit");
    let mut lines = vec![
        format!("--- a/{label}"),
        format!("+++ b/{label}"),
        "@@ live streaming edit @@".to_string(),
    ];
    if let Some(t) = target {
        for line in t.lines().take(24) {
            lines.push(format!("-{line}"));
        }
    }
    if let Some(r) = replacement {
        for line in r.lines().take(24) {
            lines.push(format!("+{line}"));
        }
    }
    if lines.len() <= 3 {
        return None;
    }
    Some((path, lines.join("\n")))
}
