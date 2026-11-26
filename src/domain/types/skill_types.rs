// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};

pub type SkillManifestList = Vec<SkillManifest>;

/// Embedded canonical GALXAI strict documentation sovereignty skill shipped with every HEAV3NS-MICRO runtime.
pub const GALXAI_DOCS_SKILL_NAME: &str = "galxai-documentation-sovereignty";
pub const GALXAI_DOCS_SKILL_DESC: &str = "Enforces the GALXAI strict documentation strategy, Diátaxis workspace topology, deterministic README blueprint, autorolling ROADMAP/CHANGELOG governance, and zero-dependency docs-check CI gates for every project created with HEAV3NS-MICRO.";
pub const GALXAI_DOCS_SKILL_MD: &str =
    include_str!("../../../skills/galxai-documentation-sovereignty/SKILL.md");
pub const GALXAI_DOCS_CHECK_MJS: &str =
    include_str!("../../../skills/galxai-documentation-sovereignty/scripts/docs-check.mjs");
pub const GALXAI_DOCS_ROLL_MJS: &str =
    include_str!("../../../skills/galxai-documentation-sovereignty/scripts/docs-roll.mjs");
pub const GALXAI_SCAFFOLD_DOCS_MJS: &str =
    include_str!("../../../skills/galxai-documentation-sovereignty/scripts/scaffold-docs.mjs");

/// Returns embedded `(relative_script_path, content)` bundles for a built-in sovereign skill.
pub fn builtin_skill_scripts(skill_name: &str) -> &'static [(&'static str, &'static str)] {
    if skill_name.eq_ignore_ascii_case(GALXAI_DOCS_SKILL_NAME) {
        &[
            ("scripts/scaffold-docs.mjs", GALXAI_SCAFFOLD_DOCS_MJS),
            ("scripts/docs-check.mjs", GALXAI_DOCS_CHECK_MJS),
            ("scripts/docs-roll.mjs", GALXAI_DOCS_ROLL_MJS),
        ]
    } else {
        &[]
    }
}

/// Strips an optional leading HTML layer comment and YAML frontmatter block so built-in and disk-loaded skill instructions match identically.
pub fn strip_skill_frontmatter(raw: &str) -> String {
    let normalized = if raw.trim_start().starts_with("<!--") {
        raw.split_once("-->")
            .map(|(_, rest)| rest.trim_start())
            .unwrap_or(raw)
    } else {
        raw
    };
    if let Some(after_open) = normalized
        .strip_prefix("---\n")
        .or_else(|| normalized.strip_prefix("---\r\n"))
    {
        if let Some(end_idx) = after_open.find("\n---") {
            let after_close = &after_open[end_idx + 4..];
            let body = if after_close.trim_start().starts_with("<!--") {
                after_close
                    .split_once("-->")
                    .map(|(_, rest)| rest.trim_start())
                    .unwrap_or(after_close)
            } else {
                after_close
            };
            return body.trim().to_string();
        }
    }
    normalized.trim().to_string()
}

/// Returns built-in sovereign skill manifests always available across every workspace created or opened with HEAV3NS-MICRO.
pub fn builtin_skill_manifests() -> Vec<SkillManifest> {
    vec![SkillManifest {
        name: GALXAI_DOCS_SKILL_NAME.to_string(),
        description: GALXAI_DOCS_SKILL_DESC.to_string(),
        path: "builtin://skills/galxai-documentation-sovereignty/SKILL.md".to_string(),
        instructions: strip_skill_frontmatter(GALXAI_DOCS_SKILL_MD),
    }]
}

/// Merges workspace-discovered skills with built-in sovereign skills (workspace skills win on name collision).
pub fn merge_builtin_skills(mut manifests: Vec<SkillManifest>) -> Vec<SkillManifest> {
    for builtin in builtin_skill_manifests() {
        if !manifests
            .iter()
            .any(|m| m.name.eq_ignore_ascii_case(&builtin.name))
        {
            manifests.push(builtin);
        }
    }
    manifests.sort_by(|a, b| a.name.cmp(&b.name));
    manifests
}

/// Abstract contract for discovered workspace skill manifests.
pub trait ISkillManifest {
    fn skill_name(&self) -> &str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub path: String,
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedOutput {
    pub output_text: String,
    pub was_truncated: bool,
    pub spill_id: Option<String>,
}

/// Cached, zero-I/O snapshot of workspace manifests, Joyzoning topology, and sanitized governance digest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspacePromptFacts {
    pub manifests: Vec<String>,
    pub joyzoning_detected: bool,
    pub os: String,
    pub shell: String,
    pub governance_digest: Option<String>,
    #[serde(default)]
    pub governance_sources: Vec<String>,
    #[serde(default)]
    pub governance_warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joy_methodology_path: Option<String>,
}
