// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};

/// Contractual sovereignty interface for learned interaction patterns.
pub trait ILearnedPattern {
    fn pattern_identity(&self) -> &str;
    fn pattern_confidence(&self) -> f32;
}

/// Contractual sovereignty interface for environmental reconciliation reports.
pub trait IReconciliationReport {
    fn has_diverged(&self) -> bool;
    fn external_drift_observed(&self) -> bool;
}

/// A durable or session-accumulated interaction convention learned through observation and feedback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearnedInteractionPattern {
    pub pattern_id: String,
    pub app_pattern: String,
    pub ui_trigger: String,
    pub affordance: String,
    pub recommended_modality: String,
    pub success_count: u32,
    pub failure_count: u32,
    pub confidence: f32,
    pub provenance_epoch: u64,
    pub last_verified_ms: u64,
}

impl LearnedInteractionPattern {
    pub fn new(id: impl Into<String>, app: impl Into<String>, trigger: impl Into<String>, affordance: impl Into<String>) -> Self {
        let aff = affordance.into();
        let default_modality = if aff.contains("input") || aff.contains("search") {
            "type"
        } else if aff.contains("dismiss") {
            "key"
        } else {
            "click"
        };
        Self {
            pattern_id: id.into(),
            app_pattern: app.into(),
            ui_trigger: trigger.into(),
            affordance: aff,
            recommended_modality: default_modality.into(),
            success_count: 1,
            failure_count: 0,
            confidence: 0.80,
            provenance_epoch: 1,
            last_verified_ms: 0,
        }
    }

    pub fn record_success(&mut self, epoch: u64, now_ms: u64) {
        self.success_count += 1;
        self.provenance_epoch = epoch;
        self.last_verified_ms = now_ms;
        self.confidence = (self.confidence + 0.05).min(1.0);
    }

    pub fn record_failure(&mut self, epoch: u64, now_ms: u64) {
        self.failure_count += 1;
        self.provenance_epoch = epoch;
        self.last_verified_ms = now_ms;
        self.confidence = (self.confidence - 0.15).max(0.10);
    }
}

impl ILearnedPattern for LearnedInteractionPattern {
    fn pattern_identity(&self) -> &str {
        &self.pattern_id
    }

    fn pattern_confidence(&self) -> f32 {
        self.confidence
    }
}

/// A verified multi-step operational procedure stored for durable reuse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearnedWorkflowProcedure {
    pub procedure_id: String,
    pub app: String,
    pub goal: String,
    pub steps: Vec<String>,
    pub success_rate: f64,
    pub created_epoch: u64,
}

impl LearnedWorkflowProcedure {
    pub fn new(id: impl Into<String>, app: impl Into<String>, goal: impl Into<String>, steps: Vec<String>, epoch: u64) -> Self {
        Self {
            procedure_id: id.into(),
            app: app.into(),
            goal: goal.into(),
            steps,
            success_rate: 1.0,
            created_epoch: epoch,
        }
    }
}

/// Report produced when reconciling internal checkpointed state with actual external environment state.
/// Explicitly recognizes that internal Rewind does NOT reverse external GUI side effects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentalReconciliationReport {
    pub checkpoint_epoch: u64,
    pub current_epoch: u64,
    pub state_diverged: bool,
    pub external_drift_detected: bool,
    pub divergence_reasons: Vec<String>,
    pub suggested_recovery: String,
}

impl EnvironmentalReconciliationReport {
    pub fn synchronized(epoch: u64) -> Self {
        Self {
            checkpoint_epoch: epoch,
            current_epoch: epoch,
            state_diverged: false,
            external_drift_detected: false,
            divergence_reasons: Vec::new(),
            suggested_recovery: "none_needed".into(),
        }
    }

    pub fn diverged(ckpt: u64, cur: u64, drift: bool, reasons: Vec<String>, recovery: impl Into<String>) -> Self {
        Self {
            checkpoint_epoch: ckpt,
            current_epoch: cur,
            state_diverged: true,
            external_drift_detected: drift,
            divergence_reasons: reasons,
            suggested_recovery: recovery.into(),
        }
    }
}

impl IReconciliationReport for EnvironmentalReconciliationReport {
    fn has_diverged(&self) -> bool {
        self.state_diverged
    }

    fn external_drift_observed(&self) -> bool {
        self.external_drift_detected
    }
}

