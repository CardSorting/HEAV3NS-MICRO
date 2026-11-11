// [LAYER: DOMAIN]
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::runtime::ExecutionPosture;
use crate::plumbing::hash_and_math::fnv1a_32;
use crate::plumbing::schema_and_sse::redact_endpoint_for_display;
use crate::plumbing::secret_redactor::DeterministicSecretRedactor;
use crate::plumbing::text_tokens::{estimate_text_tokens, safe_utf8_prefix};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Order-preserving wire fingerprint of the session's KV-cache prefix (`system + tools`).
/// Tools are hashed in exact provider wire order (never sorted) because LLM KV-caches are byte-order sensitive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixFingerprint {
    #[serde(rename = "systemHash")]
    pub system_hash: String,
    #[serde(rename = "toolsHash")]
    pub tools_hash: String,
    #[serde(rename = "combinedHash")]
    pub combined_hash: String,
    #[serde(rename = "toolCount")]
    pub tool_count: usize,
}

impl PrefixFingerprint {
    pub fn from_wire_prefix(system_prompt: &str, tools_wire: &[Value]) -> Self {
        let sys_h = fnv1a_32(system_prompt.as_bytes());
        let tools_serialized = serde_json::to_string(tools_wire).unwrap_or_default();
        let tools_h = fnv1a_32(tools_serialized.as_bytes());
        let mut combined_buf = Vec::with_capacity(system_prompt.len() + tools_serialized.len() + 1);
        combined_buf.extend_from_slice(system_prompt.as_bytes());
        combined_buf.push(0x1f);
        combined_buf.extend_from_slice(tools_serialized.as_bytes());
        let comb_h = fnv1a_32(&combined_buf);
        Self {
            system_hash: format!("{sys_h:08x}"),
            tools_hash: format!("{tools_h:08x}"),
            combined_hash: format!("{comb_h:08x}"),
            tool_count: tools_wire.len(),
        }
    }
}

/// KV-cache prefix pin stability status across turns (`Codewhale CACHE.md` invariant).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixPinStatus {
    PinnedInitial,
    Stable,
    Repinned { reason: String },
    Drift { detail: String },
}

impl PrefixPinStatus {
    pub fn label(&self) -> String {
        match self {
            Self::PinnedInitial => "pinned_initial".to_string(),
            Self::Stable => "stable".to_string(),
            Self::Repinned { reason } => format!("repinned:{reason}"),
            Self::Drift { detail } => format!("drift:{detail}"),
        }
    }
}

/// Session-pinned KV-cache stability manager.
/// Declared changes (`repin`) advance the pin; undeclared byte mutations report `Drift` and keep the original pin
/// so cache misses remain visible instead of being silently normalized!
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrefixStabilityTracker {
    pub pinned: Option<PrefixFingerprint>,
    pub pending_repin_reason: Option<String>,
    pub stable_hits: u64,
    pub repin_count: u64,
    pub drift_count: u64,
    pub last_memory_digest_hash: Option<u32>,
}

impl PrefixStabilityTracker {
    pub fn declare_repin(&mut self, reason: impl Into<String>) {
        self.pending_repin_reason = Some(reason.into());
    }

    pub fn record_prefix(&mut self, current: PrefixFingerprint) -> PrefixPinStatus {
        match &self.pinned {
            None => {
                self.pinned = Some(current);
                self.pending_repin_reason = None;
                PrefixPinStatus::PinnedInitial
            }
            Some(pin) if *pin == current => {
                self.pending_repin_reason = None;
                self.stable_hits += 1;
                PrefixPinStatus::Stable
            }
            Some(pin) => {
                if let Some(reason) = self.pending_repin_reason.take() {
                    self.pinned = Some(current);
                    self.repin_count += 1;
                    PrefixPinStatus::Repinned { reason }
                } else {
                    self.drift_count += 1;
                    let mut parts = Vec::new();
                    if pin.system_hash != current.system_hash {
                        parts.push("system_bytes_changed");
                    }
                    if pin.tools_hash != current.tools_hash {
                        parts.push("tool_catalog_changed");
                    }
                    PrefixPinStatus::Drift {
                        detail: parts.join("+"),
                    }
                }
            }
        }
    }
}

/// Formats volatile memory/context updates as an append-only `<context_update>` block inside a `Role::User` turn
/// instead of mutating `Role::System` or injecting `Role::Assistant` at index 1 (preserving 100% KV-cache hits
/// AND maintaining strict Anthropic `user`-first alternation).
pub fn format_append_only_context_update(memory_envelope: Option<&str>, posture: ExecutionPosture) -> Option<String> {
    let mut blocks = Vec::new();
    blocks.push(format!("active_posture: {} ({})", posture.display_label(), posture.slug()));
    if let Some(mem) = memory_envelope {
        let trimmed = mem.trim();
        if !trimmed.is_empty() {
            blocks.push(trimmed.to_string());
        }
    }
    if blocks.len() <= 1 && posture == ExecutionPosture::AcceptEdits {
        return None;
    }
    Some(format!(
        "<context_update trust=\"untrusted-session-delta\">\n{}\n</context_update>",
        blocks.join("\n")
    ))
}

/// Zero-side-effect outbound request manifest (`heav3ns.request_manifest/v1`) for `/preview-request`.
/// Guarantees exact canonical JSON byte identity:
/// `system_bytes + tool_schema_bytes + history_bytes + json_framing_bytes == canonical_body_bytes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestManifest {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "wireDialect")]
    pub wire_dialect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "endpointDisplay")]
    pub endpoint_display: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "reasoningEffort")]
    pub reasoning_effort: Option<String>,
    pub posture: String,
    #[serde(rename = "prefixFingerprint")]
    pub prefix_fingerprint: PrefixFingerprint,
    #[serde(rename = "prefixStatus")]
    pub prefix_status: String,
    #[serde(rename = "systemBytes")]
    pub system_bytes: usize,
    #[serde(rename = "toolSchemaBytes")]
    pub tool_schema_bytes: usize,
    #[serde(rename = "historyBytes")]
    pub history_bytes: usize,
    #[serde(rename = "jsonFramingBytes")]
    pub json_framing_bytes: usize,
    #[serde(rename = "canonicalBodyBytes")]
    pub canonical_body_bytes: usize,
    #[serde(rename = "contextWindowTokens")]
    pub context_window_tokens: usize,
    #[serde(rename = "estimatedInputTokens")]
    pub estimated_input_tokens: usize,
    #[serde(rename = "headroomTokens")]
    pub headroom_tokens: i64,
    #[serde(rename = "activeTools")]
    pub active_tools: Vec<String>,
    #[serde(rename = "messageCount")]
    pub message_count: usize,
    #[serde(rename = "redactedSystemPreview")]
    pub redacted_system_preview: String,
}

impl RequestManifest {
    pub fn build(
        model: &str,
        posture: ExecutionPosture,
        messages: &[SessionMessage],
        tools: &[Value],
        context_window_tokens: usize,
        prefix_status: &str,
    ) -> Self {
        let system_text = messages
            .iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.as_str())
            .unwrap_or("");
        let fp = PrefixFingerprint::from_wire_prefix(system_text, tools);

        let canonical_payload = json!({
            "model": model,
            "messages": messages,
            "tools": tools,
        });
        let canonical_str = serde_json::to_string(&canonical_payload).unwrap_or_default();
        let canonical_body_bytes = canonical_str.len();

        let system_bytes = messages
            .iter()
            .filter(|m| m.role == Role::System)
            .map(|m| serde_json::to_string(m).map(|s| s.len()).unwrap_or(0))
            .sum::<usize>();
        let history_bytes = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| serde_json::to_string(m).map(|s| s.len()).unwrap_or(0))
            .sum::<usize>();
        let tool_schema_bytes = serde_json::to_string(tools).map(|s| s.len()).unwrap_or(0);
        let parts_sum = system_bytes + history_bytes + tool_schema_bytes;
        let json_framing_bytes = canonical_body_bytes.saturating_sub(parts_sum);

        let estimated_input_tokens = estimate_text_tokens(&canonical_str);
        let headroom_tokens = (context_window_tokens as i64) - (estimated_input_tokens as i64);

        let active_tools = tools
            .iter()
            .filter_map(|t| t.get("function").and_then(|f| f.get("name")).and_then(Value::as_str).map(String::from))
            .collect();

        let redacted_sys = DeterministicSecretRedactor::redact(safe_utf8_prefix(system_text, 600));

        Self {
            schema: "heav3ns.request_manifest/v1".to_string(),
            provider: None,
            model: model.to_string(),
            wire_dialect: None,
            endpoint_display: None,
            reasoning_effort: None,
            posture: posture.slug().to_string(),
            prefix_fingerprint: fp,
            prefix_status: prefix_status.to_string(),
            system_bytes,
            tool_schema_bytes,
            history_bytes,
            json_framing_bytes,
            canonical_body_bytes,
            context_window_tokens,
            estimated_input_tokens,
            headroom_tokens,
            active_tools,
            message_count: messages.len(),
            redacted_system_preview: redacted_sys,
        }
    }

    pub fn with_route_metadata(
        mut self,
        provider: &str,
        wire_dialect: &str,
        endpoint: &str,
        reasoning_effort: Option<&str>,
    ) -> Self {
        self.provider = Some(provider.to_string());
        self.wire_dialect = Some(wire_dialect.to_string());
        self.endpoint_display = Some(redact_endpoint_for_display(endpoint));
        self.reasoning_effort = reasoning_effort.map(String::from);
        self
    }

    pub fn render_human_readable(&self) -> String {
        let route_line = match (&self.provider, &self.wire_dialect, &self.endpoint_display) {
            (Some(prov), Some(wire), Some(ep)) => format!(
                "{prov} · {wire} · {ep}{}",
                self.reasoning_effort
                    .as_deref()
                    .map(|r| format!(" · effort={r}"))
                    .unwrap_or_default()
            ),
            _ => "canonical".to_string(),
        };
        format!(
            "┌─ HEAV3NS-MICRO Request Manifest ({}) ───────────────────\n\
             │ Route / Wire    : {}\n\
             │ Model / Posture : {} · {}\n\
             │ KV-Cache Pin    : {} [sys={} · tools={} · {}]\n\
             │ Byte Accounting : sys={}B + tools={}B + hist={}B + frame={}B == {}B\n\
             │ Token Headroom  : ~{} / {} tokens (headroom: {:+}t)\n\
             │ Active Tools    : {} ({})\n\
             │ Messages        : {} total\n\
             └──────────────────────────────────────────────────────────────────────────",
            self.schema,
            route_line,
            self.model,
            self.posture,
            self.prefix_fingerprint.combined_hash,
            self.prefix_fingerprint.system_hash,
            self.prefix_fingerprint.tools_hash,
            self.prefix_status,
            self.system_bytes,
            self.tool_schema_bytes,
            self.history_bytes,
            self.json_framing_bytes,
            self.canonical_body_bytes,
            self.estimated_input_tokens,
            self.context_window_tokens,
            self.headroom_tokens,
            self.active_tools.len(),
            self.active_tools.join(", "),
            self.message_count
        )
    }
}
