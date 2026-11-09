// [LAYER: DOMAIN]
use crate::domain::types::spatial_types::{AssuranceVerdict, SpatialError, SpatialTarget};
use serde::{Deserialize, Serialize};

/// Contractual sovereignty interface for grounded spatial targets.
pub trait IGroundedTarget {
    fn resolved_coordinates(&self) -> (f64, f64);
    fn confidence_score(&self) -> f64;
}

/// Contractual sovereignty interface for execution receipts.
pub trait IExecutionReceipt {
    fn is_confirmed(&self) -> bool;
    fn receipt_action(&self) -> &str;
}

/// An operational target fully resolved, coordinate-transformed, visibility-checked, and safe-padded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundedSpatialTarget {
    pub target: SpatialTarget,
    pub safe_point: [i32; 2],
    pub screen_bounds: [i32; 4],
    pub visibility_ratio: f64,
    pub occlusion_ratio: f64,
    pub occluding_target_id: Option<String>,
    pub confidence: f32,
    pub requires_scroll: bool,
    pub scroll_delta: (f64, f64),
}

impl GroundedSpatialTarget {
    pub fn is_actionable(&self, max_occlusion: f64, min_confidence: f32) -> bool {
        self.target.enabled
            && self.confidence >= min_confidence
            && self.occlusion_ratio <= max_occlusion
            && self.visibility_ratio > 0.05
    }
}

/// Output of Wrapper A (Spatial Grounding), indicating whether the target was resolved unambiguously.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialGroundingResult {
    pub success: bool,
    pub grounded: Option<GroundedSpatialTarget>,
    pub ambiguity_candidates: Vec<String>,
    pub error: Option<SpatialError>,
    pub recovery_hint: Option<String>,
}

impl SpatialGroundingResult {
    pub fn success(grounded: GroundedSpatialTarget) -> Self {
        Self {
            success: true,
            grounded: Some(grounded),
            ambiguity_candidates: Vec::new(),
            error: None,
            recovery_hint: None,
        }
    }

    pub fn failed(error: SpatialError, recovery_hint: Option<String>) -> Self {
        Self {
            success: false,
            grounded: None,
            ambiguity_candidates: Vec::new(),
            error: Some(error),
            recovery_hint,
        }
    }

    pub fn ambiguous(candidates: Vec<String>) -> Self {
        let err = SpatialError::AmbiguousTarget(candidates.clone());
        Self {
            success: false,
            grounded: None,
            ambiguity_candidates: candidates,
            error: Some(err),
            recovery_hint: Some("Disambiguate with Set-of-Marks token (e.g. 's1:2') or unique ID".into()),
        }
    }
}

/// Verifiable audit receipt emitted after Wrapper B Execution Assurance concludes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialExecutionReceipt {
    pub action: String,
    pub target_id: Option<String>,
    pub executed_point: [i32; 2],
    pub verdict: AssuranceVerdict,
    pub postconditions_met: bool,
    pub duration_ms: u64,
    pub layout_epoch: u64,
    pub frame_seq: u64,
    pub recovery_suggestion: Option<String>,
}

impl SpatialExecutionReceipt {
    pub fn confirmed(action: impl Into<String>, target_id: Option<String>, pt: [i32; 2], epoch: u64, frame: u64, duration_ms: u64) -> Self {
        Self {
            action: action.into(),
            target_id,
            executed_point: pt,
            verdict: AssuranceVerdict::Confirmed,
            postconditions_met: true,
            duration_ms,
            layout_epoch: epoch,
            frame_seq: frame,
            recovery_suggestion: None,
        }
    }

    pub fn suspected_noop(action: impl Into<String>, target_id: Option<String>, pt: [i32; 2], epoch: u64, frame: u64, duration_ms: u64, escalation: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            target_id,
            executed_point: pt,
            verdict: AssuranceVerdict::SuspectedNoop,
            postconditions_met: false,
            duration_ms,
            layout_epoch: epoch,
            frame_seq: frame,
            recovery_suggestion: Some(escalation.into()),
        }
    }
}

