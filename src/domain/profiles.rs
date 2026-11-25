// [LAYER: DOMAIN]
use crate::domain::types::runtime::ExecutionPosture;
use serde::{Deserialize, Serialize};

pub const DEFAULT_JOY_METHODOLOGY_PATH: &str = "/Users/bozoegg/Desktop/HEAV3NS-JOY";
pub const DEFAULT_JOY_CLI_PATH: &str = "/Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs";
pub const DEFAULT_AGENT_PROFILE_ID: &str = "joyzoning-architect";

pub const DEFAULT_CI_GATE_TIMEOUT_MS: u64 = 30_000;

fn default_ci_gate_timeout_ms() -> u64 {
    DEFAULT_CI_GATE_TIMEOUT_MS
}

/// Policy governing how `HEAV3NS-JOY` acts as the CI gate for development tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiGatePolicy {
    pub enabled: bool,
    #[serde(rename = "triggerOnDevTasks")]
    pub trigger_on_dev_tasks: bool,
    #[serde(rename = "joyCliPath")]
    pub joy_cli_path: String,
    pub theme: String,
    pub phase: String,
    #[serde(rename = "failClosed")]
    pub fail_closed: bool,
    #[serde(default = "default_ci_gate_timeout_ms", rename = "timeoutMs")]
    pub timeout_ms: u64,
}

impl Default for CiGatePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger_on_dev_tasks: true,
            joy_cli_path: DEFAULT_JOY_CLI_PATH.to_string(),
            theme: "strict".to_string(),
            phase: "ci".to_string(),
            fail_closed: true,
            timeout_ms: DEFAULT_CI_GATE_TIMEOUT_MS,
        }
    }
}

/// Deterministic CI gate execution receipt emitted by `ICiGateRunner`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiGateReceipt {
    pub passed: bool,
    pub gate: String,
    pub theme: String,
    pub phase: String,
    #[serde(rename = "exitCode")]
    pub exit_code: i32,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    #[serde(rename = "blockingCount")]
    pub blocking_count: usize,
    #[serde(rename = "advisoryCount")]
    pub advisory_count: usize,
    pub summary: String,
}

/// Selectable agent operational profile binding methodology, style directives, posture, and CI gate policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub methodology: String,
    #[serde(rename = "methodologyPath")]
    pub methodology_path: String,
    #[serde(rename = "styleDirectives")]
    pub style_directives: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "defaultPosture")]
    pub default_posture: Option<ExecutionPosture>,
    #[serde(rename = "ciGate")]
    pub ci_gate: CiGatePolicy,
}

impl Default for AgentProfile {
    fn default() -> Self {
        Self::default_profile()
    }
}

impl AgentProfile {
    pub fn default_profile() -> Self {
        Self {
            id: DEFAULT_AGENT_PROFILE_ID.to_string(),
            name: "Joyzoning Architect".to_string(),
            description: "Primary sovereign development profile using HEAV3NS-JOY as methodology, style, and fail-closed CI gate.".to_string(),
            methodology: "heav3ns-joy".to_string(),
            methodology_path: DEFAULT_JOY_METHODOLOGY_PATH.to_string(),
            style_directives: vec![
                "Triad Audit (Architect/Critic/SRE) -> Producer-Before-Consumer (`PLUMBING->DOMAIN->INFRA->CORE->UI`).".to_string(),
                "Rules: `[LAYER: <ZONE>]` L1, Geo Alignment, <=3 `../` & 0 cycles, `I<Name>`, <800L; `joy-check.mjs` pre/post-edit.".to_string(),
            ],
            default_posture: None,
            ci_gate: CiGatePolicy::default(),
        }
    }

    pub fn builtin_profiles() -> Vec<Self> {
        vec![
            Self::default_profile(),
            Self {
                id: "strict-ci".to_string(),
                name: "Strict CI Guardian".to_string(),
                description: "Interactive approval posture with mandatory HEAV3NS-JOY strict CI gate on every development task.".to_string(),
                methodology: "heav3ns-joy".to_string(),
                methodology_path: DEFAULT_JOY_METHODOLOGY_PATH.to_string(),
                style_directives: vec![
                    "Follow HEAV3NS-JOY Triad Audit and Producer-Before-Consumer layering strictly.".to_string(),
                    "Verify every touched file with joy-check.mjs --phase pre-edit and --phase post-edit; fail closed on any SPI blocking violation.".to_string(),
                ],
                default_posture: Some(ExecutionPosture::Ask),
                ci_gate: CiGatePolicy::default(),
            },
            Self {
                id: "fast-patch".to_string(),
                name: "Fast Patch".to_string(),
                description: "Rapid iteration with HEAV3NS-JOY style and advisory CI gate reporting.".to_string(),
                methodology: "heav3ns-joy".to_string(),
                methodology_path: DEFAULT_JOY_METHODOLOGY_PATH.to_string(),
                style_directives: vec![
                    "Follow HEAV3NS-JOY 5-zone boundaries and [LAYER: <ZONE>] tagging; keep edits minimal and targeted.".to_string(),
                ],
                default_posture: Some(ExecutionPosture::AcceptEdits),
                ci_gate: CiGatePolicy {
                    enabled: true,
                    trigger_on_dev_tasks: true,
                    joy_cli_path: DEFAULT_JOY_CLI_PATH.to_string(),
                    theme: "strict".to_string(),
                    phase: "ci".to_string(),
                    fail_closed: false,
                    timeout_ms: DEFAULT_CI_GATE_TIMEOUT_MS,
                },
            },
            Self {
                id: "research-readonly".to_string(),
                name: "Research Read-Only".to_string(),
                description: "Read-only exploration and architectural analysis without workspace mutations or CI gate.".to_string(),
                methodology: "heav3ns-joy".to_string(),
                methodology_path: DEFAULT_JOY_METHODOLOGY_PATH.to_string(),
                style_directives: vec![
                    "Analyze architecture through HEAV3NS-JOY 5-zone lens without mutating files.".to_string(),
                ],
                default_posture: Some(ExecutionPosture::Plan),
                ci_gate: CiGatePolicy {
                    enabled: false,
                    trigger_on_dev_tasks: false,
                    joy_cli_path: DEFAULT_JOY_CLI_PATH.to_string(),
                    theme: "strict".to_string(),
                    phase: "ci".to_string(),
                    fail_closed: false,
                    timeout_ms: DEFAULT_CI_GATE_TIMEOUT_MS,
                },
            },
        ]
    }

    pub fn resolve(id: &str) -> Option<Self> {
        let normalized = id.trim().to_ascii_lowercase().replace('_', "-");
        let target = match normalized.as_str() {
            "" | "default" | "joy" | "heav3ns-joy" | "architect" => DEFAULT_AGENT_PROFILE_ID,
            "strict" | "ci" | "guardian" => "strict-ci",
            "fast" | "patch" => "fast-patch",
            "research" | "readonly" | "read-only" | "plan" => "research-readonly",
            other => other,
        };
        Self::builtin_profiles()
            .into_iter()
            .find(|p| p.id == target)
    }

    /// Returns true if a tool name represents a workspace-mutating development action that should trigger the CI gate.
    pub fn is_dev_mutation_tool(tool_name: &str) -> bool {
        matches!(
            tool_name,
            "write_file"
                | "replace_file_content"
                | "multi_replace_file_content"
                | "apply_unified_diff"
                | "delete_file"
                | "move_file"
                | "batch_replace"
                | "execute_tools"
        )
    }

    /// Parses `.heav3ns/profiles.json` with strict schema, size, and CI-gate trust-boundary validation.
    pub fn parse_workspace_profiles_strict(raw: &str) -> Result<Vec<Self>, String> {
        if raw.len() > 64 * 1024 {
            return Err("Workspace profiles.json exceeds 64 KiB limit".to_string());
        }
        let parsed: Vec<Self> =
            serde_json::from_str(raw).map_err(|e| format!("Invalid profiles.json: {e}"))?;
        if parsed.len() > 16 {
            return Err("Workspace profiles.json exceeds maximum of 16 profiles".to_string());
        }
        let mut out = Vec::with_capacity(parsed.len());
        for mut prof in parsed {
            let id = prof.id.trim().to_ascii_lowercase();
            if id.is_empty()
                || id.len() > 48
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(format!("Invalid workspace profile id '{}'", prof.id));
            }
            prof.id = id;
            if prof.style_directives.len() > 16 {
                return Err(format!("Profile '{}' exceeds 16 styleDirectives", prof.id));
            }
            for d in &prof.style_directives {
                if d.len() > 240 {
                    return Err(format!("Profile '{}' styleDirective exceeds 240 bytes", prof.id));
                }
            }
            let cli = prof.ci_gate.joy_cli_path.trim();
            if !cli.is_empty() && cli != DEFAULT_JOY_CLI_PATH {
                return Err(format!(
                    "Profile '{}' attempts to set untrusted joyCliPath '{}'",
                    prof.id, cli
                ));
            }
            if cli.is_empty() {
                prof.ci_gate.joy_cli_path = DEFAULT_JOY_CLI_PATH.to_string();
            }
            if !matches!(prof.ci_gate.theme.as_str(), "strict" | "relaxed" | "safety") {
                return Err(format!(
                    "Profile '{}' has invalid CI gate theme '{}'",
                    prof.id, prof.ci_gate.theme
                ));
            }
            if !matches!(
                prof.ci_gate.phase.as_str(),
                "pre-edit" | "post-edit" | "ci" | "delta"
            ) {
                return Err(format!(
                    "Profile '{}' has invalid CI gate phase '{}'",
                    prof.id, prof.ci_gate.phase
                ));
            }
            prof.ci_gate.timeout_ms = prof.ci_gate.timeout_ms.clamp(500, 120_000);
            if prof.methodology.trim().is_empty() {
                prof.methodology = "heav3ns-joy".to_string();
            }
            if prof.methodology_path.trim().is_empty() {
                prof.methodology_path = DEFAULT_JOY_METHODOLOGY_PATH.to_string();
            }
            out.push(prof);
        }
        Ok(out)
    }

    /// Resolves a profile by id, checking workspace-defined profiles first and falling back to built-ins.
    pub fn resolve_with_custom(id: &str, custom: &[Self]) -> Option<Self> {
        let normalized = id.trim().to_ascii_lowercase().replace('_', "-");
        if let Some(found) = custom.iter().find(|p| p.id == normalized) {
            return Some(found.clone());
        }
        Self::resolve(id)
    }
}
