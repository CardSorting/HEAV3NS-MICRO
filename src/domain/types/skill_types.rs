// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};

pub type SkillManifestList = Vec<SkillManifest>;

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
}
