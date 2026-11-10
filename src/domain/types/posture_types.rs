// [LAYER: DOMAIN]
pub use crate::domain::types::runtime::{ExecutionPosture, UserApprovalChoice};
use crate::plumbing::seatbelt_profile::{inspect_command_security, CommandSecurityVerdict};
use crate::plumbing::secret_redactor::{CredentialDenyScope, DeterministicSecretRedactor};
use crate::plumbing::text_tokens::canonical_tool_name;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Verdict returned by the monotonic 9-layer authorization reducer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationVerdict {
    Allow,
    AskUser,
    Deny,
}

/// Protected path invariant inside `.heav3ns/constitution.json` or `.codewhale/constitution.json` (`RepoLaw`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConstitutionRule {
    pub id: String,
    pub description: String,
    pub paths: Vec<String>,
    pub action: String,
}

/// Repository constitution policy (`RepoLaw`). Malformed JSON fails closed (`invalid_error.is_some()`),
/// holding every mutating tool call behind explicit human approval rather than failing open.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoConstitution {
    pub version: u32,
    pub rules: Vec<RepoConstitutionRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid_error: Option<String>,
}

impl RepoConstitution {
    pub fn parse_strict(raw_json: &str) -> Self {
        let parsed: Result<Value, _> = serde_json::from_str(raw_json);
        let Ok(val) = parsed else {
            return Self {
                version: 0,
                rules: Vec::new(),
                invalid_error: Some("Malformed constitution JSON (fail-closed: all writes require human approval)".to_string()),
            };
        };
        let version = val.get("version").and_then(Value::as_u64).unwrap_or(1) as u32;
        if version != 1 {
            return Self {
                version,
                rules: Vec::new(),
                invalid_error: Some(format!("Unsupported constitution schema version {version} (expected 1; fail-closed)")),
            };
        }
        let mut rules = Vec::new();
        if let Some(arr) = val.get("rules").or_else(|| val.get("invariants")).and_then(Value::as_array) {
            for item in arr {
                let id = item.get("id").and_then(Value::as_str).unwrap_or("rule").to_string();
                let description = item.get("description").and_then(Value::as_str).unwrap_or("Protected path").to_string();
                let action = item.get("action").and_then(Value::as_str).unwrap_or("ask").to_ascii_lowercase();
                let paths = item
                    .get("paths")
                    .and_then(Value::as_array)
                    .map(|p| p.iter().filter_map(Value::as_str).map(String::from).collect())
                    .unwrap_or_default();
                rules.push(RepoConstitutionRule { id, description, paths, action });
            }
        }
        Self { version, rules, invalid_error: None }
    }

    pub fn evaluate_path(&self, rel_path: &str) -> Option<(AuthorizationVerdict, String)> {
        if let Some(ref err) = self.invalid_error {
            return Some((AuthorizationVerdict::AskUser, err.clone()));
        }
        let norm = rel_path.trim_start_matches("./").replace('\\', "/");
        for rule in &self.rules {
            let matched = rule.paths.iter().any(|pat| {
                let p = pat.trim_start_matches("./").trim_end_matches('*');
                norm == pat.as_str() || (!p.is_empty() && norm.starts_with(p))
            });
            if matched {
                let verdict = if rule.action == "block" || rule.action == "deny" {
                    AuthorizationVerdict::Deny
                } else {
                    AuthorizationVerdict::AskUser
                };
                return Some((verdict, format!("RepoLaw [{}]: {}", rule.id, rule.description)));
            }
        }
        None
    }
}

/// Complete authorization decision for a single tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostureEvaluation {
    pub verdict: AuthorizationVerdict,
    pub layer: String,
    pub reason: String,
    pub session_rule_key: String,
}

/// Evaluates Codewhale's monotonic authorization gate across `ExecutionPosture`, `RepoConstitution`,
/// session allowlists, and `CommandSecurityVerdict` without mutating wire tool schemas.
pub fn evaluate_tool_posture(
    posture: ExecutionPosture,
    tool_name: &str,
    args: &Value,
    is_mutating_spec: bool,
    constitution: Option<&RepoConstitution>,
    session_allowed_keys: &[String],
) -> PostureEvaluation {
    let canonical = canonical_tool_name(tool_name);
    let target_path = extract_primary_path(args);

    // Layer 1: Dual-scope credential protection applies in EVERY posture (even FullAccess for Process scope)
    if let Some(ref p) = target_path {
        let scope = if posture == ExecutionPosture::FullAccess {
            CredentialDenyScope::Process
        } else {
            CredentialDenyScope::ModelContext
        };
        if let Some(label) = DeterministicSecretRedactor::classify_sensitive_path(p, scope) {
            return PostureEvaluation {
                verdict: AuthorizationVerdict::Deny,
                layer: "credential_guard".to_string(),
                reason: format!("Access to protected credential store refused: {label}"),
                session_rule_key: format!("deny:{canonical}"),
            };
        }
    }

    // Pure read-only tools pass in every posture (including Plan!)
    if !is_mutating_spec && canonical != "run_command" && canonical != "skill_forge" {
        return PostureEvaluation {
            verdict: AuthorizationVerdict::Allow,
            layer: "readonly_baseline".to_string(),
            reason: "Read-only inspection tool".to_string(),
            session_rule_key: format!("read:{canonical}"),
        };
    }

    // Layer 2: Shell command security & read-only disambiguation
    if canonical == "run_command" {
        let cmd = args.get("command").and_then(Value::as_str).unwrap_or("");
        match inspect_command_security(cmd) {
            CommandSecurityVerdict::Deny { reason } => {
                return PostureEvaluation {
                    verdict: AuthorizationVerdict::Deny,
                    layer: "command_safety".to_string(),
                    reason,
                    session_rule_key: "deny:run_command".to_string(),
                };
            }
            CommandSecurityVerdict::ReadOnlySafe => {
                return PostureEvaluation {
                    verdict: AuthorizationVerdict::Allow,
                    layer: "readonly_shell".to_string(),
                    reason: "Verified read-only shell command".to_string(),
                    session_rule_key: "shell:readonly".to_string(),
                };
            }
            CommandSecurityVerdict::RequireApproval { reason } => {
                if posture == ExecutionPosture::Plan {
                    return PostureEvaluation {
                        verdict: AuthorizationVerdict::Deny,
                        layer: "plan_mode".to_string(),
                        reason: "Plan mode is read-only; switch posture (`Shift+Tab` or `/mode accept-edits`) to run commands.".to_string(),
                        session_rule_key: "plan:block".to_string(),
                    };
                }
                let rule_key = format!("shell:high_risk:{}", cmd.split_whitespace().next().unwrap_or("cmd"));
                if posture == ExecutionPosture::FullAccess || session_allowed_keys.iter().any(|k| k == &rule_key) {
                    return PostureEvaluation {
                        verdict: AuthorizationVerdict::Allow,
                        layer: "authorized_high_risk".to_string(),
                        reason,
                        session_rule_key: rule_key,
                    };
                }
                return PostureEvaluation {
                    verdict: AuthorizationVerdict::AskUser,
                    layer: "high_risk_command".to_string(),
                    reason,
                    session_rule_key: rule_key,
                };
            }
            CommandSecurityVerdict::WorkspaceMutating { summary } => {
                if posture == ExecutionPosture::Plan {
                    return PostureEvaluation {
                        verdict: AuthorizationVerdict::Deny,
                        layer: "plan_mode".to_string(),
                        reason: format!("Plan mode blocks mutating command ({summary}); wire tool schema stays cached. Step mode with `Shift+Tab` or `/mode`."),
                        session_rule_key: "plan:block".to_string(),
                    };
                }
                let rule_key = format!("shell:{summary}");
                if matches!(posture, ExecutionPosture::AutoReview | ExecutionPosture::FullAccess)
                    || session_allowed_keys.iter().any(|k| k == &rule_key)
                {
                    return PostureEvaluation {
                        verdict: AuthorizationVerdict::Allow,
                        layer: "posture_shell".to_string(),
                        reason: format!("Authorized under {}", posture.display_label()),
                        session_rule_key: rule_key,
                    };
                }
                return PostureEvaluation {
                    verdict: AuthorizationVerdict::AskUser,
                    layer: "posture_ask_shell".to_string(),
                    reason: format!("Shell command ({summary}) requires approval in {} mode", posture.display_label()),
                    session_rule_key: rule_key,
                };
            }
        }
    }

    // Layer 3: Central Plan-mode mutation block (keeps wire tool list frozen for KV cache!)
    if posture == ExecutionPosture::Plan {
        return PostureEvaluation {
            verdict: AuthorizationVerdict::Deny,
            layer: "plan_mode".to_string(),
            reason: format!("Plan mode is read-only (`{canonical}` blocked without mutating KV-cached tool schemas). Switch mode via `Shift+Tab` or `/mode` to apply edits."),
            session_rule_key: "plan:block".to_string(),
        };
    }

    // Layer 4: RepoConstitution (`RepoLaw`) invariants
    if let (Some(law), Some(ref p)) = (constitution, &target_path) {
        if let Some((verdict, reason)) = law.evaluate_path(p) {
            let rule_key = format!("repolaw:{canonical}:{p}");
            if verdict == AuthorizationVerdict::Deny {
                return PostureEvaluation { verdict, layer: "repo_constitution".to_string(), reason, session_rule_key: rule_key };
            }
            if !session_allowed_keys.iter().any(|k| k == &rule_key) {
                return PostureEvaluation { verdict, layer: "repo_constitution".to_string(), reason, session_rule_key: rule_key };
            }
        }
    }

    // Protected `.git/hooks` & `.git/config` write guard outside FullAccess
    if let Some(ref p) = target_path {
        let norm = p.replace('\\', "/").to_ascii_lowercase();
        if posture != ExecutionPosture::FullAccess
            && (norm.contains(".git/hooks") || norm.ends_with(".git/config") || norm.contains("../"))
        {
            let rule_key = format!("protected_write:{canonical}");
            if !session_allowed_keys.iter().any(|k| k == &rule_key) {
                return PostureEvaluation {
                    verdict: AuthorizationVerdict::AskUser,
                    layer: "protected_subpath".to_string(),
                    reason: format!("Write to protected path '{p}' requires explicit approval"),
                    session_rule_key: rule_key,
                };
            }
        }
    }

    // Layer 5: Posture dial decision for workspace file mutations
    let rule_key = format!("edit:{canonical}");
    match posture {
        ExecutionPosture::Ask => {
            if session_allowed_keys.iter().any(|k| k == &rule_key) {
                PostureEvaluation {
                    verdict: AuthorizationVerdict::Allow,
                    layer: "session_allowlist".to_string(),
                    reason: "Approved for session".to_string(),
                    session_rule_key: rule_key,
                }
            } else {
                PostureEvaluation {
                    verdict: AuthorizationVerdict::AskUser,
                    layer: "ask_mode".to_string(),
                    reason: format!("Tool `{canonical}` requires confirmation in Ask mode"),
                    session_rule_key: rule_key,
                }
            }
        }
        ExecutionPosture::AcceptEdits | ExecutionPosture::AutoReview | ExecutionPosture::FullAccess => PostureEvaluation {
            verdict: AuthorizationVerdict::Allow,
            layer: "posture_allow".to_string(),
            reason: format!("Authorized under {}", posture.display_label()),
            session_rule_key: rule_key,
        },
        ExecutionPosture::Plan => unreachable!(),
    }
}

fn extract_primary_path(args: &Value) -> Option<String> {
    for key in ["path", "filePath", "file_path", "target", "source", "destination"] {
        if let Some(s) = args.get(key).and_then(Value::as_str) {
            if !s.trim().is_empty() {
                return Some(s.trim().to_string());
            }
        }
    }
    None
}
