// [LAYER: DOMAIN]
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::text_tokens::estimate_text_tokens;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub type TranscriptSlice<'a> = &'a [SessionMessage];
pub type ToolCallBatch = Vec<ToolCall>;

/// Abstract contract for conversation transcript items.
pub trait ITranscriptMessage {
    fn message_role(&self) -> Role;
    fn message_content(&self) -> &str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(default = "default_tool_type")]
    pub r#type: String,
    pub function: ToolCallFunction,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "thoughtSignature")]
    pub thought_signature: Option<String>,
}

fn default_tool_type() -> String {
    "function".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "toolCallId")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    pub timestamp: u64,
}

impl SessionMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: None,
            timestamp: now_ms(),
        }
    }

    pub fn tool_result(call_id: impl Into<String>, name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_call_id: Some(call_id.into()),
            name: Some(name.into()),
            tool_calls: None,
            timestamp: now_ms(),
        }
    }
}

#[inline]
pub fn estimate_messages_tokens(messages: &[SessionMessage]) -> usize {
    messages.iter().map(|m| 4 + estimate_text_tokens(&m.content)).sum()
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
            let name = val
                .get("name")
                .or_else(|| val.get("tool"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !name.is_empty() {
                idx += 1;
                let args = val
                    .get("arguments")
                    .or_else(|| val.get("args"))
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Map::new()));
                let args_str = match args {
                    Value::String(s) => s,
                    other => other.to_string(),
                };
                calls.push(ToolCall {
                    id: format!("txt_call_{idx}"),
                    r#type: "function".to_string(),
                    function: ToolCallFunction {
                        name: name.to_string(),
                        arguments: args_str,
                    },
                    thought_signature: None,
                });
            }
        }
    }
    calls
}
