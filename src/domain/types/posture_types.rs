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
        let raw_version = val.get("version").and_then(Value::as_u64).unwrap_or(1);
        if raw_version != 1 {
            return Self {
                version: u32::try_from(raw_version).unwrap_or(u32::MAX),
                rules: Vec::new(),
                invalid_error: Some(format!("Unsupported constitution schema version {raw_version} (expected 1; fail-closed)")),
            };
        }
        let version = 1u32;
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
        let norm = normalize_rel_path(rel_path);
        let mut strictest: Option<(AuthorizationVerdict, String)> = None;
        for rule in &self.rules {
            let matched = rule.paths.iter().any(|pat| {
                if pat == "*" || pat == "/*" {
                    return true;
                }
                let norm_pat = normalize_rel_path(pat);
                if norm == norm_pat {
                    return true;
                }
                let p = normalize_rel_path(pat.trim_end_matches('*').trim_end_matches('/'));
                !p.is_empty() && (norm == p || norm.starts_with(&format!("{p}/")))
            });
            if matched {
                let verdict = if rule.action == "block" || rule.action == "deny" {
                    AuthorizationVerdict::Deny
                } else {
                    AuthorizationVerdict::AskUser
                };
                let msg = format!("RepoLaw [{}]: {}", rule.id, rule.description);
                if verdict == AuthorizationVerdict::Deny {
                    return Some((verdict, msg));
                }
                if strictest.is_none() {
                    strictest = Some((verdict, msg));
                }
            }
        }
        strictest
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
    let target_paths = extract_all_paths(&canonical, args);

    // Layer 1: Dual-scope credential protection applies in EVERY posture (even FullAccess for Process scope)
    let scope = if posture == ExecutionPosture::FullAccess {
        CredentialDenyScope::Process
    } else {
        CredentialDenyScope::ModelContext
    };
    for p in &target_paths {
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
    let is_readonly_skill = canonical == "skill_forge"
        && matches!(
            args.get("action").and_then(Value::as_str).unwrap_or("list_dag"),
            "list_dag" | "soul_status" | "export" | "lint"
        );
    let is_readonly_gui = match canonical {
        "computer_use" => {
            let action = args.get("action").and_then(Value::as_str).unwrap_or("capture").trim().to_ascii_lowercase();
            let has_mutating_button = args.get("button").and_then(Value::as_str).is_some_and(|b| !b.trim().is_empty());
            let has_multi_click = args.get("click_count").and_then(Value::as_u64).is_some_and(|c| c > 1);
            !has_mutating_button && !has_multi_click && matches!(
                action.as_str(),
                "capture" | "capture_frame" | "screenshot" | "snapshot" | "list_windows" | "list_apps" | "list_elements" | "read_window_below" | "ax_query" | "clipboard_read" | "lease_status" | "search_dsl" | "query" | "audit_health" | "health" | "metrics"
            )
        }
        "browser_use" => matches!(
            args.get("action").and_then(Value::as_str).unwrap_or("snapshot"),
            "snapshot" | "dom" | "console" | "network" | "cookies_get" | "storage_get" | "validate_url" | "query"
        ),
        "spatial_interact" => matches!(
            args.get("action").and_then(Value::as_str).unwrap_or("inspect"),
            "observe" | "reconcile" | "ground" | "plan" | "inspect" | "query"
        ),
        _ => false,
    };
    if (!is_mutating_spec && !matches!(canonical, "run_command" | "skill_forge" | "computer_use" | "browser_use" | "spatial_interact"))
        || is_readonly_skill
        || is_readonly_gui
    {
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

    // Layer 4: RepoConstitution (`RepoLaw`) invariants across all path arguments
    if let Some(law) = constitution {
        for p in &target_paths {
            if let Some((verdict, reason)) = law.evaluate_path(p) {
                let rule_key = format!("repolaw:{canonical}:{}", normalize_rel_path(p));
                if verdict == AuthorizationVerdict::Deny {
                    return PostureEvaluation { verdict, layer: "repo_constitution".to_string(), reason, session_rule_key: rule_key };
                }
                if !session_allowed_keys.iter().any(|k| k == &rule_key) {
                    return PostureEvaluation { verdict, layer: "repo_constitution".to_string(), reason, session_rule_key: rule_key };
                }
            }
        }
    }

    // Protected `.git/hooks`, `.git/config`, and `../` write guard outside FullAccess
    for p in &target_paths {
        let norm = normalize_rel_path(p).to_ascii_lowercase();
        if posture != ExecutionPosture::FullAccess
            && (norm.contains(".git/hooks") || norm.ends_with(".git/config") || norm.contains("../") || norm.ends_with("/..") || norm == "..")
        {
            let rule_key = format!("protected_write:{canonical}:{norm}");
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

fn normalize_rel_path(raw: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let replaced = raw.replace('\\', "/");
    for seg in replaced.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.last().copied() == Some("..") || parts.is_empty() {
                    parts.push("..");
                } else {
                    parts.pop();
                }
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn extract_all_paths(canonical: &str, args: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    let keys: &[&str] = match canonical {
        "move_file" => &["source", "target", "destination", "path"],
        _ => &["path", "filePath", "file_path", "destination"],
    };
    for &key in keys {
        if let Some(s) = args.get(key).and_then(Value::as_str) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                paths.push(trimmed.to_string());
            }
        }
    }
    if canonical == "run_command" {
        if let Some(cmd) = args.get("command").and_then(Value::as_str) {
            for tok in cmd.split(|c: char| c.is_whitespace() || matches!(c, '=' | '<' | '>' | ';' | '|' | '(' | ')')) {
                let cleaned = tok.trim_matches(|c| c == '"' || c == '\'');
                if !cleaned.is_empty() {
                    paths.push(cleaned.to_string());
                }
            }
        }
    }
    if canonical == "apply_unified_diff" {
        if let Some(diff) = args.get("diff").or_else(|| args.get("patch")).and_then(Value::as_str) {
            for line in diff.lines() {
                let candidate = if let Some(rest) = line.strip_prefix("+++ ") {
                    let p = rest.split('\t').next().unwrap_or(rest).trim();
                    Some(p.strip_prefix("b/").or_else(|| p.strip_prefix("a/")).unwrap_or(p))
                } else if let Some(rest) = line.strip_prefix("--- ") {
                    let p = rest.split('\t').next().unwrap_or(rest).trim();
                    Some(p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(p))
                } else if let Some(rest) = line
                    .trim()
                    .strip_prefix("*** Update File:")
                    .or_else(|| line.trim().strip_prefix("*** File:"))
                    .or_else(|| line.trim().strip_prefix("*** Add File:"))
                    .or_else(|| line.trim().strip_prefix("*** Delete File:"))
                {
                    Some(rest)
                } else {
                    None
                };
                if let Some(c) = candidate {
                    let c = c.trim();
                    if !c.is_empty() && c != "/dev/null" && !paths.iter().any(|p| p == c) {
                        paths.push(c.to_string());
                    }
                }
            }
        }
    }
    paths
}
