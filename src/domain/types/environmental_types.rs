// [LAYER: DOMAIN]
use crate::domain::types::computer_use_types::UiElement;
use serde::{Deserialize, Serialize};

/// Contractual sovereignty interface for environmental observations.
pub trait IEnvironmentalObservation {
    fn window_identity(&self) -> &str;
    fn is_modal_blocking(&self) -> bool;
}

/// Contractual sovereignty interface for behavioral affordances.
pub trait IBehavioralAffordance {
    fn target_identity(&self) -> &str;
    fn affordance_name(&self) -> &str;
    fn confidence_value(&self) -> f32;
}

/// Functional region categorization within an application viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionKind {
    HeaderNav,
    Sidebar,
    ContentArea,
    ModalDialog,
    FooterStatusBar,
    Toolbar,
    UtilityPanel,
}

impl Default for RegionKind {
    fn default() -> Self {
        Self::ContentArea
    }
}

/// A cohesive structural region within a window (e.g. navigation bar, sidebar, content area, modal layer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentalRegion {
    pub id: String,
    pub kind: RegionKind,
    pub bounds: [i32; 4],
    pub element_ids: Vec<String>,
    pub z_order: u32,
}

impl EnvironmentalRegion {
    pub fn new(id: impl Into<String>, kind: RegionKind, bounds: [i32; 4], z_order: u32) -> Self {
        Self {
            id: id.into(),
            kind,
            bounds,
            element_ids: Vec::new(),
            z_order,
        }
    }
}

/// Interactive capability and expected operational outcome of a UI element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BehavioralAffordance {
    pub target_id: String,
    pub affordance_type: String, // "clickable", "input", "scrollable", "dismissable", "navigational", "modal_backdrop"
    pub expected_outcome: String,
    pub confidence: f32,
    pub contextual_clues: Vec<String>,
}

impl BehavioralAffordance {
    pub fn new(target_id: impl Into<String>, affordance_type: impl Into<String>, expected_outcome: impl Into<String>, confidence: f32) -> Self {
        Self {
            target_id: target_id.into(),
            affordance_type: affordance_type.into(),
            expected_outcome: expected_outcome.into(),
            confidence,
            contextual_clues: Vec::new(),
        }
    }
}

/// Comprehensive, multi-modal environmental representation capturing active application,
/// window hierarchy, visual regions, interactive elements, keyboard focus, and modal state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentalObservation {
    pub window_id: String,
    pub app_name: String,
    pub window_title: String,
    pub frame_seq: u64,
    pub layout_epoch: u64,
    pub active_focus_id: Option<String>,
    pub cursor: [i32; 2],
    pub modal_detected: bool,
    pub modal_container_id: Option<String>,
    pub regions: Vec<EnvironmentalRegion>,
    pub elements: Vec<UiElement>,
    pub frame_hash: String,
    pub timestamp_ms: u64,
}

impl EnvironmentalObservation {
    pub fn new(window_id: impl Into<String>, app_name: impl Into<String>, title: impl Into<String>, frame_seq: u64, layout_epoch: u64) -> Self {
        Self {
            window_id: window_id.into(),
            app_name: app_name.into(),
            window_title: title.into(),
            frame_seq,
            layout_epoch,
            active_focus_id: None,
            cursor: [0, 0],
            modal_detected: false,
            modal_container_id: None,
            regions: Vec::new(),
            elements: Vec::new(),
            frame_hash: String::new(),
            timestamp_ms: 0,
        }
    }

    pub fn find_element_by_id(&self, id: &str) -> Option<&UiElement> {
        self.elements.iter().find(|e| e.id == id)
    }

    pub fn is_element_in_modal(&self, element_id: &str) -> bool {
        if !self.modal_detected {
            return false;
        }
        self.regions
            .iter()
            .filter(|r| r.kind == RegionKind::ModalDialog)
            .any(|r| r.element_ids.iter().any(|eid| eid == element_id))
    }
}

impl IEnvironmentalObservation for EnvironmentalObservation {
    fn window_identity(&self) -> &str {
        &self.window_id
    }

    fn is_modal_blocking(&self) -> bool {
        self.modal_detected
    }
}

impl IBehavioralAffordance for BehavioralAffordance {
    fn target_identity(&self) -> &str {
        &self.target_id
    }

    fn affordance_name(&self) -> &str {
        &self.affordance_type
    }

    fn confidence_value(&self) -> f32 {
        self.confidence
    }
}

