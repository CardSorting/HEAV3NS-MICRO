// [LAYER: DOMAIN]
pub use crate::domain::types::runtime::{ExecutionPosture, UserApprovalChoice};
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
                invalid_error: Some(
                    "Malformed constitution JSON (fail-closed: all writes require human approval)"
                        .to_string(),
                ),
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
        if let Some(arr) = val
            .get("rules")
            .or_else(|| val.get("invariants"))
            .and_then(Value::as_array)
        {
            for item in arr {
                let id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("rule")
                    .to_string();
                let description = item
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("Protected path")
                    .to_string();
                let action = item
                    .get("action")
                    .and_then(Value::as_str)
                    .unwrap_or("ask")
                    .to_ascii_lowercase();
                let paths = item
                    .get("paths")
                    .and_then(Value::as_array)
                    .map(|p| {
                        p.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                rules.push(RepoConstitutionRule {
                    id,
                    description,
                    paths,
                    action,
                });
            }
        }
        Self {
            version,
            rules,
            invalid_error: None,
        }
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


pub(crate) fn normalize_rel_path(raw: &str) -> String {
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
