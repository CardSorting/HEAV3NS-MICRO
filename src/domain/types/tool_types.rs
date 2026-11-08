// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub type ParameterSchemaEntry = (&'static str, ParamSchema);
pub type OpenAiToolSchema = Value;

/// Abstract contract for registered tool specifications.
pub trait IToolSpecification {
    fn spec_name(&self) -> &'static str;
    fn is_mutating_tool(&self) -> bool;
}

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
        Self {
            r#type,
            required: true,
            description,
        }
    }

    pub const fn opt(r#type: ParamType, description: &'static str) -> Self {
        Self {
            r#type,
            required: false,
            description,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SchemaValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub is_mutating: bool,
    pub is_core: bool,
    pub parameters: Vec<(&'static str, ParamSchema)>,
}

impl ToolSpec {
    pub fn to_openai_tool(&self) -> Value {
        let mut props = Map::new();
        let mut required = Vec::new();
        for &(pname, ref pschema) in &self.parameters {
            props.insert(
                pname.to_string(),
                json!({ "type": pschema.r#type.as_str(), "description": pschema.description }),
            );
            if pschema.required {
                required.push(Value::String(pname.to_string()));
            }
        }
        json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": { "type": "object", "properties": props, "required": required }
            }
        })
    }
}
