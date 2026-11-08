use crate::{ToolCall, ToolCallFunction};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    String,
    Number,
    Integer,
    Boolean,
    Array,
    Object,
}

impl ParamType {
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::Array => "array",
            Self::Object => "object",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSchema {
    pub r#type: ParamType,
    pub required: bool,
    pub description: &'static str,
}

impl ParamSchema {
    pub const fn req(r#type: ParamType, description: &'static str) -> Self {
        Self { r#type, required: true, description }
    }
    pub const fn opt(r#type: ParamType, description: &'static str) -> Self {
        Self { r#type, required: false, description }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SchemaValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub suggestions: Vec<String>,
}

/// Maps model tool name variations across OpenAI, Anthropic, Gemini, Codex, and Cline to canonical names.
pub fn canonical_tool_name(name: &str) -> &str {
    match name.trim() {
        "read_file" | "readFile" | "read" | "cat" | "viewFile" | "view_file" => "view_file",
        "writeFile" | "write_to_file" | "create_file" | "createFile" | "write" | "write_file" => "write_file",
        "replaceFileContent" | "replace_in_file" | "replace_content" | "edit_file" | "editFile"
        | "heal_and_apply_patch" | "replace_file_content" => "replace_file_content",
        "multi_replace" | "multiReplaceFileContent" | "multi_replace_file_content" => "multi_replace_file_content",
        "edit_file_anchored" | "anchored_edit" | "hashline_edit" => "edit_file_anchored",
        "apply_patch" | "v4a_apply_patch" | "patch_file" | "apply_unified_diff" => "apply_unified_diff",
        "runCommand" | "execute_command" | "executeCommand" | "bash" | "sh" | "terminal" | "exec" | "run_command" => "run_command",
        "list_files" | "listFiles" | "list_directory" | "ls" | "dir" | "list_dir" => "list_dir",
        "grep" | "search" | "search_files" | "ripgrep" | "rg" | "grep_search" => "grep_search",
        "find_file" | "findFiles" | "find_by_name" | "find_files_by_pattern" | "glob" | "glob_files" | "find_files" => "find_files",
        "stat" | "stat_file" | "fileInfo" | "path_exists" | "file_hash" | "file_info" => "file_info",
        "tree" | "directoryTree" | "get_workspace_file_tree" | "directory_tree" => "directory_tree",
        "remove_file" | "removeFile" | "deleteFile" | "rm" | "unlink" | "delete_file" => "delete_file",
        "rename_file" | "renameFile" | "moveFile" | "copy_file" | "mv" | "rename" | "move_file" => "move_file",
        "undo" | "rollback" | "rollback_turn_mutations" | "rollback_last_mutation" => "rollback_last_mutation",
        "search_codebase_symbols" | "get_file_outline" | "get_symbol_definition" | "get_symbol_references" | "search_symbols" => "search_symbols",
        "rename_symbol_across_codebase" | "search_and_replace" | "batch_replace_regex" => "batch_replace_regex",
        "workspace_diff" | "v4a_collect_working_diff" | "get_workspace_diff" => "get_workspace_diff",
        "memory_remember" | "remember" | "save_memory" => "save_memory",
        "memory_recall" | "recall" | "search_memory" => "search_memory",
        "spawn_subagent" | "execute_parallel_batch" | "engage_swarm" => "engage_swarm",
        other => other,
    }
}

const PARAM_ALIASES: &[(&str, &[&str])] = &[
    ("path", &["filePath", "file_path", "file", "targetFile", "target_file", "targetPath", "target_path", "filename", "directory", "dir", "subpath"]),
    ("paths", &["filePaths", "file_paths", "files", "targetFiles"]),
    ("source", &["sourcePath", "source_path", "from", "src", "inputPath"]),
    ("target", &["targetContent", "target_content", "targetSnippet", "find", "oldName", "old_name", "search_text", "to", "dest", "destination", "outputPath"]),
    ("replacement", &["replacementContent", "replacement_content", "replace", "newName", "new_name", "new_text", "newText", "newContent", "new_content"]),
    ("command", &["cmd", "script", "shellCommand", "shell_command", "run"]),
    ("content", &["text", "body", "data", "code", "fileContent", "file_content", "value", "fact"]),
    ("diff", &["patch", "patchText", "unifiedDiff", "v4aPatch"]),
    ("query", &["pattern", "symbol", "search_term", "searchTerm", "search_query", "term", "q"]),
    ("name", &["skillName", "skill_name", "skill", "toolName", "tool_name"]),
    ("startLine", &["start_line", "fromLine", "start"]),
    ("endLine", &["end_line", "toLine", "end"]),
];

#[inline]
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut prev: Vec<usize> = (0..=b_bytes.len()).collect();
    let mut curr = vec![0; b_bytes.len() + 1];
    for (i, &ca) in a_bytes.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b_bytes.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_bytes.len()]
}

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
                if esc { esc = false; } else if ch == b'\\' { esc = true; } else if ch == b'"' { in_str = false; }
            } else {
                match ch {
                    b'"' => in_str = true,
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 { end_idx = Some(i + 1); i += 1; break; }
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

/// Extracts `<tool_call>{...}</tool_call>` blocks emitted in plain text by open-weights/local models.
pub fn extract_text_tool_calls(text: &str) -> Vec<ToolCall> {
    if !text.contains("<tool_call>") {
        return Vec::new();
    }
    let mut calls = Vec::new();
    let mut rest = text;
    let mut idx = 0usize;
    while let Some(start) = rest.find("<tool_call>") {
        let after = &rest[start + 11..];
        let Some(end) = after.find("</tool_call>") else { break };
        let body = after[..end].trim();
        rest = &after[end + 12..];
        if let Ok(val) = serde_json::from_str::<Value>(body) {
            let name = val.get("name").or_else(|| val.get("tool")).and_then(|v| v.as_str()).unwrap_or("");
            if !name.is_empty() {
                idx += 1;
                let args = val.get("arguments").or_else(|| val.get("args")).cloned().unwrap_or_else(|| Value::Object(Map::new()));
                let args_str = match args { Value::String(s) => s, other => other.to_string() };
                calls.push(ToolCall {
                    id: format!("txt_call_{idx}"),
                    r#type: "function".to_string(),
                    function: ToolCallFunction { name: name.to_string(), arguments: args_str },
                });
            }
        }
    }
    calls
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
        return Err(format!("{provider_label} returned {} conflicting concatenated JSON payloads", roots.len()));
    }
    Err(format!("{provider_label} returned invalid tool arguments"))
}

// ============================================================================
// DETERMINISTIC SECRET REDACTOR & SENSITIVE PATH SAFETY ENGINE (ADR-047)
// ============================================================================

const DENIED_PATH_SUBSTRINGS: &[&str] = &[
    ".ssh/id_rsa", ".ssh/id_ed25519", ".ssh/id_ecdsa", ".ssh/authorized_keys",
    ".aws/credentials", ".docker/config.json", ".kube/config", ".gnupg",
    ".netrc", ".git-credentials", ".anthropic_oauth.json", "bws_cache.enc.json",
];

pub struct DeterministicSecretRedactor;

impl DeterministicSecretRedactor {
    #[inline]
    pub fn is_sensitive_path(path: &str) -> bool {
        let norm = path.replace('\\', "/");
        DENIED_PATH_SUBSTRINGS.iter().any(|p| norm.contains(p))
    }

    pub fn redact(text: &str) -> String {
        if text.len() < 12 {
            return text.to_string();
        }
        static PATTERNS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
        let rules = PATTERNS.get_or_init(|| {
            vec![
                ("pem_key", Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----").unwrap()),
                ("anthropic_key", Regex::new(r"\b(sk-ant-[A-Za-z0-9_-]{20,})\b").unwrap()),
                ("openai_key", Regex::new(r"\b(sk-(?:proj-)?[A-Za-z0-9_-]{20,})\b").unwrap()),
                ("github_token", Regex::new(r"\b(gh[pousr]_[A-Za-z0-9_]{36,})\b").unwrap()),
                ("aws_key", Regex::new(r"\b(AKIA[0-9A-Z]{16})\b").unwrap()),
                ("google_key", Regex::new(r"\b(AIza[0-9A-Za-z_-]{30,})\b").unwrap()),
                ("hf_token", Regex::new(r"\b(hf_[A-Za-z0-9]{24,})\b").unwrap()),
                ("slack_token", Regex::new(r"\b(xox[baprs]-[0-9A-Za-z-]{10,64})\b").unwrap()),
                ("jwt", Regex::new(r"\b(eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,})\b").unwrap()),
            ]
        });
        let mut out = text.to_string();
        for &(label, ref re) in rules {
            if re.is_match(&out) {
                out = re.replace_all(&out, format!("[REDACTED:{label}]")).into_owned();
            }
        }
        out
    }
}

// ============================================================================
// 4-PASS SELF-HEALING TOOL CALL ARGUMENT PARSER
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct ToolCallArgParser;

impl ToolCallArgParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn parse_raw_arguments(&self, raw: &str) -> (Map<String, Value>, bool) {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == "{}" || trimmed == "null" || trimmed == "undefined" {
            return (Map::new(), false);
        }
        // Pass 1: Direct JSON parse
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(trimmed) {
            return (map, false);
        }
        // Pass 1.5: Concatenated JSON roots check (ADR-0014)
        let (roots, rem) = extract_json_roots(trimmed);
        if roots.len() > 1 && rem.trim().is_empty() {
            if let Some(Value::Object(first_map)) = roots.first() {
                if roots.iter().all(|r| r.as_object() == Some(first_map)) {
                    return (first_map.clone(), true);
                }
            }
            let mut err_map = Map::new();
            err_map.insert(
                "__toolArgumentConcatenationError".to_string(),
                Value::String(format!("Tool arguments contain {} distinct concatenated JSON payloads.", roots.len())),
            );
            return (err_map, false);
        }
        // Pass 2: Strip Markdown code fences (closed or unclosed)
        let mut sanitized = trimmed.to_string();
        if let Some(fence_start) = sanitized.find("```") {
            let after_fence = &sanitized[fence_start + 3..];
            let body_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
            let after_header = &after_fence[body_start..];
            sanitized = match after_header.rfind("```") {
                Some(end) => after_header[..end].trim().to_string(),
                None => after_header.trim().to_string(),
            };
        }
        // Pass 3: Extract first outer JSON object
        if let Some(first_brace) = sanitized.find('{') {
            sanitized = match sanitized.rfind('}') {
                Some(last_brace) if last_brace > first_brace => sanitized[first_brace..=last_brace].to_string(),
                _ => sanitized[first_brace..].to_string(),
            };
        }
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&sanitized) {
            return (map, true);
        }
        // Pass 4: Heuristic syntax repair
        let repaired = self.repair_json_string(&sanitized);
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&repaired) {
            return (map, true);
        }
        (Map::new(), true)
    }

    pub fn repair_json_string(&self, input: &str) -> String {
        let mut text = input
            .trim()
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
        while i < chars.len() {
            let c = chars[i];
            if in_str {
                if esc { esc = false; } else if c == '\\' { esc = true; } else if c == '"' { in_str = false; }
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
            cleaned.push(c);
            i += 1;
        }
        let open_braces = cleaned.chars().filter(|&c| c == '{').count();
        let close_braces = cleaned.chars().filter(|&c| c == '}').count();
        for _ in 0..open_braces.saturating_sub(close_braces) {
            cleaned.push('}');
        }
        let open_brackets = cleaned.chars().filter(|&c| c == '[').count();
        let close_brackets = cleaned.chars().filter(|&c| c == ']').count();
        for _ in 0..open_brackets.saturating_sub(close_brackets) {
            cleaned.push(']');
        }
        cleaned
    }

    pub fn normalize_aliases(
        &self,
        mut args: Map<String, Value>,
        schema: &[(&'static str, ParamSchema)],
    ) -> Map<String, Value> {
        for &(canonical_key, aliases) in PARAM_ALIASES {
            if !args.contains_key(canonical_key) {
                for &alias in aliases {
                    if let Some(val) = args.get(alias).cloned() {
                        args.insert(canonical_key.to_string(), val);
                        break;
                    }
                }
            }
        }
        for &(param_name, _) in schema {
            if !args.contains_key(param_name) {
                let lower = param_name.to_lowercase();
                let found = args
                    .keys()
                    .find(|k| {
                        let kl = k.to_lowercase();
                        kl == lower || (kl.len() >= 3 && levenshtein(&kl, &lower) <= 2)
                    })
                    .cloned();
                if let Some(k) = found {
                    if let Some(val) = args.get(&k).cloned() {
                        args.insert(param_name.to_string(), val);
                    }
                }
            }
        }
        args
    }

    pub fn coerce_and_validate(
        &self,
        mut args: Map<String, Value>,
        schema: &[(&'static str, ParamSchema)],
    ) -> (Map<String, Value>, SchemaValidationResult) {
        if let Some(Value::String(err_msg)) = args.get("__toolArgumentConcatenationError") {
            return (
                args.clone(),
                SchemaValidationResult {
                    valid: false,
                    errors: vec![err_msg.clone()],
                    suggestions: vec!["Emit a single valid JSON object per tool call.".to_string()],
                },
            );
        }
        let (mut errors, mut suggestions) = (Vec::new(), Vec::new());
        for &(name, ref param) in schema {
            match args.get(name).cloned() {
                None | Some(Value::Null) => {
                    if param.required {
                        errors.push(format!("Missing required parameter '{}' ({})", name, param.r#type.as_str()));
                        suggestions.push(format!("Provide '{}': {}", name, param.description));
                    }
                }
                Some(val) => match param.r#type {
                    ParamType::String => match val.as_str() {
                        Some(s) if param.required && s.trim().is_empty() => {
                            errors.push(format!("Required string parameter '{name}' cannot be empty"));
                        }
                        Some(_) => {}
                        None => { args.insert(name.to_string(), Value::String(val.to_string())); }
                    },
                    ParamType::Number | ParamType::Integer => if !val.is_number() {
                        if let Some(s) = val.as_str() {
                            if let Ok(n) = s.trim().parse::<i64>() {
                                args.insert(name.to_string(), Value::Number(n.into()));
                            } else if let Ok(f) = s.trim().parse::<f64>() {
                                if let Some(num) = serde_json::Number::from_f64(f) {
                                    args.insert(name.to_string(), Value::Number(num));
                                }
                            } else {
                                errors.push(format!("Parameter '{name}' expected number, got '{s}'"));
                            }
                        } else {
                            errors.push(format!("Parameter '{name}' expected number"));
                        }
                    },
                    ParamType::Boolean => if !val.is_boolean() {
                        if let Some(s) = val.as_str() {
                            match s.trim().to_ascii_lowercase().as_str() {
                                "true" | "1" | "yes" => { args.insert(name.to_string(), Value::Bool(true)); }
                                "false" | "0" | "no" => { args.insert(name.to_string(), Value::Bool(false)); }
                                _ => errors.push(format!("Parameter '{name}' expected boolean, got '{s}'")),
                            }
                        } else {
                            errors.push(format!("Parameter '{name}' expected boolean"));
                        }
                    },
                    ParamType::Array => if !val.is_array() {
                        if let Some(s) = val.as_str() {
                            let arr = serde_json::from_str::<Value>(s)
                                .ok()
                                .and_then(|v| v.as_array().cloned())
                                .unwrap_or_else(|| vec![Value::String(s.to_string())]);
                            args.insert(name.to_string(), Value::Array(arr));
                        } else {
                            errors.push(format!("Parameter '{name}' expected array"));
                        }
                    },
                    ParamType::Object => if !val.is_object() {
                        if let Some(s) = val.as_str() {
                            if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(s) {
                                args.insert(name.to_string(), Value::Object(obj));
                            } else {
                                errors.push(format!("Parameter '{name}' expected JSON object"));
                            }
                        } else {
                            errors.push(format!("Parameter '{name}' expected object"));
                        }
                    },
                },
            }
        }
        let valid = errors.is_empty();
        (args, SchemaValidationResult { valid, errors, suggestions })
    }

    pub fn prepare_arguments(
        &self,
        raw: &Value,
        schema: &[(&'static str, ParamSchema)],
    ) -> (Map<String, Value>, SchemaValidationResult) {
        let initial_map = match raw {
            Value::Object(m) => m.clone(),
            Value::String(s) => self.parse_raw_arguments(s).0,
            _ => Map::new(),
        };
        let aliased = self.normalize_aliases(initial_map, schema);
        self.coerce_and_validate(aliased, schema)
    }
}
