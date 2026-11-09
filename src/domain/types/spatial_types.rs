// [LAYER: DOMAIN]
use crate::plumbing::spatial_geometry::SpatialRect;
use serde::{Deserialize, Serialize};

/// Contractual sovereignty interface for spatial target entities.
pub trait ISpatialTarget {
    fn target_id(&self) -> &str;
    fn bounding_rect(&self) -> &SpatialRect;
}

/// Contractual sovereignty interface for spatial scenes.
pub trait ISpatialScene {
    fn viewport_dimensions(&self) -> (f64, f64);
    fn target_count(&self) -> usize;
}

/// Contractual sovereignty interface for spatial action contracts.
pub trait ISpatialActionContract {
    fn contract_action(&self) -> &str;
    fn requires_auto_scroll(&self) -> bool;
}

/// Origin and grounding source of a spatial target bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetProvenance {
    /// Extracted directly from DOM node hierarchy and CSS layout bounding client rect.
    DomTree,
    /// Detected via Set-of-Marks visual badge overlay.
    SetOfMarks,
    /// Accessibility (AX) tree node with platform accessibility coordinates.
    AxTree,
    /// Machine-vision / visual OCR object detection model.
    VisionDetector,
    /// Window frame boundary from OS or virtual window manager.
    WindowFrame,
    /// Inferred or synthesized through heuristic spatial extrapolation.
    Synthetic,
    /// Cross-modally reconciled between DOM tree structure and visual/SoM perception.
    Reconciled,
}

impl Default for TargetProvenance {
    fn default() -> Self {
        Self::DomTree
    }
}

/// Represents an operational target primitive with identity, bounds, provenance, and state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialTarget {
    pub id: String,
    pub som_id: Option<u32>,
    pub role: String,
    pub label: String,
    pub value: String,
    pub bounds: [i32; 4], // [x, y, width, height]
    pub confidence: f32,  // 0.0 to 1.0
    pub provenance: TargetProvenance,
    pub z_order: u32,
    pub enabled: bool,
    pub focused: bool,
    pub layout_epoch: u64,
}

impl SpatialTarget {
    pub fn new(id: impl Into<String>, som_id: Option<u32>, role: impl Into<String>, label: impl Into<String>, bounds: [i32; 4]) -> Self {
        Self {
            id: id.into(),
            som_id,
            role: role.into(),
            label: label.into(),
            value: String::new(),
            bounds,
            confidence: 1.0,
            provenance: TargetProvenance::DomTree,
            z_order: 1,
            enabled: true,
            focused: false,
            layout_epoch: 1,
        }
    }

    pub fn to_spatial_rect(&self) -> SpatialRect {
        SpatialRect::from_i32_slice(self.bounds)
    }

    pub fn is_input_capable(&self) -> bool {
        let r = self.role.to_ascii_lowercase();
        matches!(r.as_str(), "textbox" | "input" | "textarea" | "combobox" | "searchbox" | "text")
    }

    pub fn is_clickable(&self) -> bool {
        let r = self.role.to_ascii_lowercase();
        matches!(r.as_str(), "button" | "link" | "checkbox" | "radio" | "tab" | "menuitem" | "switch" | "option" | "summary")
    }
}

/// An immutable, spatially indexed frame containing all visible and occluded targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialScene {
    pub width: u32,
    pub height: u32,
    pub dpr: f64,
    pub scroll_offset: [i32; 2],
    pub frame_seq: u64,
    pub layout_epoch: u64,
    pub window_id: String,
    pub targets: Vec<SpatialTarget>,
}

impl Default for SpatialScene {
    fn default() -> Self {
        Self::new(1440, 900, "win-main-1")
    }
}

impl SpatialScene {
    pub fn new(width: u32, height: u32, window_id: impl Into<String>) -> Self {
        Self {
            width,
            height,
            dpr: 1.0,
            scroll_offset: [0, 0],
            frame_seq: 1,
            layout_epoch: 1,
            window_id: window_id.into(),
            targets: Vec::new(),
        }
    }

    pub fn target_count(&self) -> usize {
        self.targets.len()
    }

    pub fn find_by_id(&self, id: &str) -> Option<&SpatialTarget> {
        self.targets.iter().find(|t| t.id == id)
    }

    pub fn find_by_som(&self, som_id: u32) -> Option<&SpatialTarget> {
        self.targets.iter().find(|t| t.som_id == Some(som_id))
    }

    pub fn find_at_point(&self, x: i32, y: i32) -> Vec<&SpatialTarget> {
        let pt = crate::plumbing::spatial_geometry::SpatialPoint::new(x as f64, y as f64);
        let mut hits: Vec<&SpatialTarget> = self.targets
            .iter()
            .filter(|t| t.to_spatial_rect().contains_point(&pt))
            .collect();
        // Sort highest z_order first, then smallest area (most specific leaf element)
        hits.sort_by(|a, b| {
            b.z_order.cmp(&a.z_order).then_with(|| {
                let area_a = a.to_spatial_rect().area();
                let area_b = b.to_spatial_rect().area();
                area_a.partial_cmp(&area_b).unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        hits
    }
}

/// Specifications for an intended spatial action, containing desired target, coordinates, and constraints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialActionContract {
    pub action_name: String,
    pub target_selector: Option<String>,
    pub som_id: Option<u32>,
    pub coordinates: Option<[f64; 2]>,
    pub coordinate_space: String, // "screen", "window", "viewport", "normalized_1000", "unit"
    pub delivery_mode: String,    // "background", "foreground"
    pub required_confidence: f32, // e.g. 0.70
    pub max_occlusion: f64,       // e.g. 0.50 (max 50% occluded)
    pub expected_epoch: Option<u64>,
    pub payload_text: Option<String>,
    pub keys: Option<String>,
    pub to_coordinates: Option<[f64; 2]>,
    pub exclusion_zones: Vec<SpatialRect>,
    pub auto_scroll_if_needed: bool,
}

impl Default for SpatialActionContract {
    fn default() -> Self {
        Self {
            action_name: "click".into(),
            target_selector: None,
            som_id: None,
            coordinates: None,
            coordinate_space: "screen".into(),
            delivery_mode: "background".into(),
            required_confidence: 0.50,
            max_occlusion: 0.60,
            expected_epoch: None,
            payload_text: None,
            keys: None,
            to_coordinates: None,
            exclusion_zones: Vec::new(),
            auto_scroll_if_needed: true,
        }
    }
}

/// Domain errors encountered during spatial mediation, grounding, or assurance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpatialError {
    TargetNotFound(String),
    AmbiguousTarget(Vec<String>),
    StaleTargetEpoch { expected: u64, current: u64 },
    TargetOccluded { target_id: String, occlusion_ratio: f64, occluded_by: String },
    TargetOutOfBounds { target_id: String, bounds: [i32; 4] },
    ContainmentViolation { point: [i32; 2], bounds: [i32; 4] },
    TargetDisabled(String),
    ActionNotPermittedForRole { role: String, action: String },
    ConfidenceTooLow { target_id: String, confidence: f32, required: f32 },
    TrajectoryBlocked(String),
    ExecutionPreconditionFailed(String),
    PostconditionFailed(String),
}

impl std::fmt::Display for SpatialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TargetNotFound(id) => write!(f, "spatial_target_not_found: Target '{}' not resolved in spatial scene", id),
            Self::AmbiguousTarget(ids) => write!(f, "spatial_ambiguity: Multiple targets match without distinct precedence: {:?}", ids),
            Self::StaleTargetEpoch { expected, current } => write!(f, "stale_target_epoch: Expected epoch {} but current scene epoch is {}", expected, current),
            Self::TargetOccluded { target_id, occlusion_ratio, occluded_by } => write!(f, "target_occluded: Target '{}' is {:.1}% occluded by '{}'", target_id, occlusion_ratio * 100.0, occluded_by),
            Self::TargetOutOfBounds { target_id, bounds } => write!(f, "target_out_of_bounds: Target '{}' with bounds {:?} is outside active viewport", target_id, bounds),
            Self::ContainmentViolation { point, bounds } => write!(f, "containment_violation: Point {:?} is not contained in target bounds {:?}", point, bounds),
            Self::TargetDisabled(id) => write!(f, "target_disabled: Target '{}' is marked disabled", id),
            Self::ActionNotPermittedForRole { role, action } => write!(f, "action_not_permitted: Action '{}' is invalid for element role '{}'", action, role),
            Self::ConfidenceTooLow { target_id, confidence, required } => write!(f, "confidence_too_low: Target '{}' confidence {:.2} below required {:.2}", target_id, confidence, required),
            Self::TrajectoryBlocked(reason) => write!(f, "trajectory_blocked: {}", reason),
            Self::ExecutionPreconditionFailed(reason) => write!(f, "precondition_failed: {}", reason),
            Self::PostconditionFailed(reason) => write!(f, "postcondition_failed: {}", reason),
        }
    }
}

impl std::error::Error for SpatialError {}

/// Verified execution outcome from Wrapper B execution assurance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssuranceVerdict {
    Confirmed,
    SuspectedNoop,
    StateDiverged,
    RequiresEscalation(String),
    RecoveryAvailable(String),
}

