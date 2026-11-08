// [LAYER: DOMAIN]
use crate::domain::types::runtime::EngineTickOutcome;
use serde::{Deserialize, Serialize};

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
