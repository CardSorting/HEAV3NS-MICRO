// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};

/// Contractual sovereignty interface for specialist insights.
pub trait ISpecialistInsight {
    fn specialist_role(&self) -> &str;
    fn insight_finding(&self) -> &str;
}

/// Contractual sovereignty interface for swarm consultation responses.
pub trait ISwarmConsultationResponse {
    fn action_recommendation(&self) -> &str;
    fn overall_confidence(&self) -> f32;
}

/// Risk stratification for collective intelligence recommendations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsultationRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl Default for ConsultationRiskLevel {
    fn default() -> Self {
        Self::Low
    }
}

/// A structured piece of intelligence contributed by a dedicated swarm specialist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecialistInsight {
    pub specialist_role: String,
    pub finding: String,
    pub confidence: f32,
    pub provenance: String,
}

impl SpecialistInsight {
    pub fn new(role: impl Into<String>, finding: impl Into<String>, confidence: f32, provenance: impl Into<String>) -> Self {
        Self {
            specialist_role: role.into(),
            finding: finding.into(),
            confidence,
            provenance: provenance.into(),
        }
    }
}

impl ISpecialistInsight for SpecialistInsight {
    fn specialist_role(&self) -> &str {
        &self.specialist_role
    }

    fn insight_finding(&self) -> &str {
        &self.finding
    }
}

/// Structured, token-compressed consultation request dispatched from Computer Use to the Synthetic Swarm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwarmConsultationRequest {
    pub consultation_id: String,
    pub task_objective: String,
    pub target_app: String,
    pub current_window_title: String,
    pub active_focus: Option<String>,
    pub modal_blocking: bool,
    pub candidate_target_ids: Vec<String>,
    pub last_failed_action: Option<String>,
    pub last_failure_reason: Option<String>,
    pub requested_specialists: Vec<String>,
    pub timestamp_ms: u64,
}

impl SwarmConsultationRequest {
    pub fn new(id: impl Into<String>, objective: impl Into<String>, app: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            consultation_id: id.into(),
            task_objective: objective.into(),
            target_app: app.into(),
            current_window_title: title.into(),
            active_focus: None,
            modal_blocking: false,
            candidate_target_ids: Vec::new(),
            last_failed_action: None,
            last_failure_reason: None,
            requested_specialists: vec![
                "VisualPerceiver".into(),
                "StructuralAnalyst".into(),
                "WorkflowNavigator".into(),
                "RiskAssessor".into(),
                "RecoverySpecialist".into(),
            ],
            timestamp_ms: 0,
        }
    }
}

/// Aggregated multi-specialist intelligence response informing the Computer Use operator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwarmConsultationResponse {
    pub consultation_id: String,
    pub recommended_action: String,
    pub recommended_target_id: Option<String>,
    pub recommended_payload: Option<String>,
    pub confidence: f32,
    pub risk_level: ConsultationRiskLevel,
    pub specialist_insights: Vec<SpecialistInsight>,
    pub recovery_recommendation: Option<String>,
    pub requires_human_confirmation: bool,
}

impl SwarmConsultationResponse {
    pub fn autonomous_recommendation(
        id: impl Into<String>,
        action: impl Into<String>,
        target: Option<String>,
        payload: Option<String>,
        confidence: f32,
    ) -> Self {
        Self {
            consultation_id: id.into(),
            recommended_action: action.into(),
            recommended_target_id: target,
            recommended_payload: payload,
            confidence,
            risk_level: ConsultationRiskLevel::Low,
            specialist_insights: Vec::new(),
            recovery_recommendation: None,
            requires_human_confirmation: false,
        }
    }

    pub fn with_risk(mut self, risk: ConsultationRiskLevel, human_confirm: bool) -> Self {
        self.risk_level = risk;
        self.requires_human_confirmation = human_confirm;
        self
    }
}

impl ISwarmConsultationResponse for SwarmConsultationResponse {
    fn action_recommendation(&self) -> &str {
        &self.recommended_action
    }

    fn overall_confidence(&self) -> f32 {
        self.confidence
    }
}

