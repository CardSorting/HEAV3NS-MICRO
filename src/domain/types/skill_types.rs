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
