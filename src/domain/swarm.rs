// [LAYER: DOMAIN]
use crate::domain::types::runtime::EngineTickOutcome;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub type SwarmWaveIndex = usize;

/// Abstract contract for topological multi-agent swarm participants.
pub trait ISwarmParticipant {
    fn assignment_id(&self) -> &str;
    fn specialist_role(&self) -> &str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmAssignment {
    pub id: String,
    pub role: String,
    pub objective: String,
    #[serde(default, rename = "dependsOn")]
    pub depends_on: Vec<String>,
    #[serde(default, rename = "assignedFiles")]
    pub assigned_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmItemReport {
    pub id: String,
    pub role: String,
    pub outcome: EngineTickOutcome,
    pub response: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmFormationResult {
    pub objective: String,
    pub outcome: EngineTickOutcome,
    pub waves: usize,
    pub reports: Vec<SwarmItemReport>,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
}

/// Deterministic `The Legion` multi-agent task classifier across 5 strategies and 7 specialist personas.
pub fn classify_task_for_legion(
    task_description: &str,
    target_files: &[String],
    strategy_override: Option<&str>,
    max_specialists: usize,
) -> Value {
    let lower = task_description.to_ascii_lowercase();
    let max_cap = max_specialists.clamp(1, 7);

    // Score each of the 7 Legion personas
    let has_arch = ["architect", "design", "zone", "joyzoning", "layer", "contract", "interface", "refactor", "structure"]
        .iter().any(|k| lower.contains(k));
    let has_research = ["audit", "investigate", "explore", "find", "analyze", "discover", "understand", "survey"]
        .iter().any(|k| lower.contains(k));
    let has_ux = ["ux", "ui", "ergonomics", "window", "display", "click", "browser", "navigation", "view", "swift", "appkit"]
        .iter().any(|k| lower.contains(k))
        || target_files.iter().any(|f| f.contains("ui/") || f.ends_with(".swift"));
    let has_harden = ["harden", "security", "resilient", "robust", "failure", "weakpoint", "ssrf", "redact", "safety", "guard"]
        .iter().any(|k| lower.contains(k));
    let has_data = ["db", "persistence", "session", "vfs", "journal", "memory", "compaction", "schema", "store"]
        .iter().any(|k| lower.contains(k));

    let strategy = match strategy_override.unwrap_or("adaptive").trim().to_ascii_lowercase().as_str() {
        "tactical_strike" | "tactical" => "tactical_strike",
        "balanced_squad" | "balanced" => "balanced_squad",
        "full_legion" | "full" => "full_legion",
        "verification_heavy" | "verify" => "verification_heavy",
        _ => {
            if lower.len() > 240 || (has_arch && has_harden && has_ux) {
                "full_legion"
            } else if has_harden {
                "verification_heavy"
            } else if target_files.len() <= 1 && lower.len() < 80 {
                "tactical_strike"
            } else {
                "balanced_squad"
            }
        }
    };

    let mut roles: Vec<(&str, &str, usize)> = Vec::new(); // (id, title, stage)
    match strategy {
        "tactical_strike" => {
            roles.push(("legion_implementer", "Core Implementation Specialist", 1));
            roles.push(("legion_verifier", "Deterministic Verification & QA Specialist", 2));
        }
        "verification_heavy" => {
            roles.push(("legion_researcher", "Deep Codebase & Failure-Mode Researcher", 1));
            roles.push(("legion_implementer", "Core Implementation Specialist", 2));
            roles.push(("legion_hardener", "Security & Adversarial Hardening Specialist", 2));
            roles.push(("legion_verifier", "Deterministic Verification & QA Specialist", 3));
        }
        "full_legion" => {
            roles.push(("legion_architect", "Sovereign Systems Architect", 1));
            roles.push(("legion_researcher", "Deep Codebase & Failure-Mode Researcher", 1));
            roles.push(("legion_implementer", "Core Implementation Specialist", 2));
            roles.push(("legion_ux_ergonomics", "World-Class UX & Ergonomics Specialist", 2));
            roles.push(("legion_hardener", "Security & Adversarial Hardening Specialist", 2));
            roles.push(("legion_data_steward", "State, VFS & Persistence Steward", 2));
            roles.push(("legion_verifier", "Deterministic Verification & QA Specialist", 3));
        }
        _ => {
            // balanced_squad
            roles.push(("legion_architect", "Sovereign Systems Architect", 1));
            if has_research {
                roles.push(("legion_researcher", "Deep Codebase & Failure-Mode Researcher", 1));
            }
            roles.push(("legion_implementer", "Core Implementation Specialist", 2));
            if has_ux {
                roles.push(("legion_ux_ergonomics", "World-Class UX & Ergonomics Specialist", 2));
            }
            if has_harden {
                roles.push(("legion_hardener", "Security & Adversarial Hardening Specialist", 2));
            }
            if has_data {
                roles.push(("legion_data_steward", "State, VFS & Persistence Steward", 2));
            }
            roles.push(("legion_verifier", "Deterministic Verification & QA Specialist", 3));
        }
    }
    roles.truncate(max_cap);

    let stage1_ids: Vec<String> = roles.iter().filter(|r| r.2 == 1).map(|r| r.0.to_string()).collect();
    let stage2_ids: Vec<String> = roles.iter().filter(|r| r.2 == 2).map(|r| r.0.to_string()).collect();

    let mut specialists = Vec::new();
    let mut assignments = Vec::new();
    for (idx, (id, title, stage)) in roles.iter().enumerate() {
        let deps: Vec<String> = if *stage == 2 {
            stage1_ids.clone()
        } else if *stage == 3 {
            if !stage2_ids.is_empty() { stage2_ids.clone() } else { stage1_ids.clone() }
        } else {
            Vec::new()
        };
        let assigned_files: Vec<String> = if target_files.is_empty() {
            Vec::new()
        } else {
            target_files
                .iter()
                .enumerate()
                .filter(|(f_idx, _)| f_idx % roles.len().max(1) == idx % roles.len().max(1))
                .map(|(_, f)| f.clone())
                .collect()
        };
        let obj = format!("[{title}] Execute stage-{stage} analysis and implementation for: {task_description}");
        specialists.push(json!({
            "id": id,
            "role": title,
            "stage": stage,
            "dependsOn": deps,
            "assignedFiles": assigned_files.clone(),
            "objective": obj
        }));
        assignments.push(SwarmAssignment {
            id: (*id).to_string(),
            role: (*title).to_string(),
            objective: obj,
            depends_on: deps,
            assigned_files,
        });
    }

    let complexity = if roles.len() >= 5 { "high" } else if roles.len() >= 3 { "medium" } else { "low" };
    json!({
        "classified": true,
        "codename": "THE_LEGION",
        "strategy": strategy,
        "complexity": complexity,
        "specialistCount": specialists.len(),
        "waves": if roles.iter().any(|r| r.2 == 3) { 3 } else { 2 },
        "specialists": specialists,
        "assignments": assignments
    })
}

