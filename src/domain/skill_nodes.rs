// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillNode {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub tier: u8,
    pub prerequisites: Vec<String>,
    pub synergies: Vec<String>,
    pub tags: Vec<String>,
    pub mastery: u8,
    pub unlocked: bool,
    pub instructions: String,
    pub source_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulProfile {
    pub id: String,
    pub name: String,
    pub archetype: String,
    pub alignment: String,
    pub axioms: Vec<String>,
    pub traits: BTreeMap<String, f64>,
    pub directives: Vec<String>,
    pub integrity_hash: String,
}

