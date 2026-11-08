// [LAYER: DOMAIN]
use crate::domain::types::tool_types::{ParamSchema, ParamType, SchemaValidationResult};
use crate::plumbing::hash_and_math::levenshtein;
use crate::plumbing::json_repair::{extract_json_roots, repair_json_string};
use serde_json::{Map, Value};

const PARAM_ALIASES: &[(&str, &[&str])] = &[
    (
        "path",
        &[
            "filePath",
            "file_path",
            "file",
            "targetFile",
            "target_file",
            "targetPath",
            "target_path",
            "filename",
            "directory",
            "dir",
            "subpath",
        ],
    ),
    ("paths", &["filePaths", "file_paths", "files", "targetFiles"]),
    ("source", &["sourcePath", "source_path", "from", "src", "inputPath"]),
    (
        "target",
        &[
            "targetContent",
            "target_content",
            "targetSnippet",
            "find",
            "oldName",
            "old_name",
            "search_text",
            "to",
            "dest",
            "destination",
            "outputPath",
        ],
    ),
    (
        "replacement",
        &[
            "replacementContent",
            "replacement_content",
            "replace",
            "newName",
            "new_name",
            "new_text",
            "newText",
            "newContent",
            "new_content",
        ],
    ),
    ("command", &["cmd", "script", "shellCommand", "shell_command", "run"]),
    (
        "content",
        &["text", "body", "data", "code", "fileContent", "file_content", "value", "fact"],
    ),
    ("diff", &["patch", "patchText", "unifiedDiff", "v4aPatch"]),
    (
        "query",
        &["pattern", "symbol", "search_term", "searchTerm", "search_query", "term", "q"],
    ),
    ("name", &["skillName", "skill_name", "skill", "toolName", "tool_name"]),
    ("startLine", &["start_line", "fromLine", "start"]),
    ("endLine", &["end_line", "toLine", "end"]),
];

/// 4-pass self-healing tool call argument parser with alias normalization and schema coercion.
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
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(trimmed) {
            return (map, false);
        }
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
                Value::String(format!(
                    "Tool arguments contain {} distinct concatenated JSON payloads.",
                    roots.len()
                )),
            );
            return (err_map, false);
        }
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
        if let Some(first_brace) = sanitized.find('{') {
            sanitized = match sanitized.rfind('}') {
                Some(last_brace) if last_brace > first_brace => sanitized[first_brace..=last_brace].to_string(),
                _ => sanitized[first_brace..].to_string(),
            };
        }
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&sanitized) {
            return (map, true);
        }
        let repaired = self.repair_json_string(&sanitized);
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&repaired) {
            return (map, true);
        }
        (Map::new(), true)
    }

    pub extern "Rust" fn repair_json_string(&self, input: &str) -> String {
        repair_json_string(input)
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
                        None => {
                            args.insert(name.to_string(), Value::String(val.to_string()));
                        }
                    },
                    ParamType::Number | ParamType::Integer => {
                        if !val.is_number() {
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
                        }
                    }
                    ParamType::Boolean => {
                        if !val.is_boolean() {
                            if let Some(s) = val.as_str() {
                                match s.trim().to_ascii_lowercase().as_str() {
                                    "true" | "1" | "yes" => {
                                        args.insert(name.to_string(), Value::Bool(true));
                                    }
                                    "false" | "0" | "no" => {
                                        args.insert(name.to_string(), Value::Bool(false));
                                    }
                                    _ => errors.push(format!("Parameter '{name}' expected boolean, got '{s}'")),
                                }
                            } else {
                                errors.push(format!("Parameter '{name}' expected boolean"));
                            }
                        }
                    }
                    ParamType::Array => {
                        if !val.is_array() {
                            if let Some(s) = val.as_str() {
                                let arr = serde_json::from_str::<Value>(s)
                                    .ok()
                                    .and_then(|v| v.as_array().cloned())
                                    .unwrap_or_else(|| vec![Value::String(s.to_string())]);
                                args.insert(name.to_string(), Value::Array(arr));
                            } else {
                                errors.push(format!("Parameter '{name}' expected array"));
                            }
                        }
                    }
                    ParamType::Object => {
                        if !val.is_object() {
                            if let Some(s) = val.as_str() {
                                if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(s) {
                                    args.insert(name.to_string(), Value::Object(obj));
                                } else {
                                    errors.push(format!("Parameter '{name}' expected JSON object"));
                                }
                            } else {
                                errors.push(format!("Parameter '{name}' expected object"));
                            }
                        }
                    }
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
