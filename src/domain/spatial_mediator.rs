// [LAYER: DOMAIN]
use crate::domain::spatial_action::{GroundedSpatialTarget, SpatialExecutionReceipt, SpatialGroundingResult};
use crate::domain::types::spatial_types::{AssuranceVerdict, SpatialActionContract, SpatialError, SpatialScene, SpatialTarget};
use crate::plumbing::spatial_geometry::{calculate_rectangles_union_area, normalize_coordinate_value, SpatialPoint, ViewportTransform};
use serde::{Deserialize, Serialize};

/// The primary spatial interaction class, managing the spatial scene, target arbitration,
/// Double-Wrapped Action Mediation (Wrapper A: Grounding, Wrapper B: Assurance), and recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialActionMediator {
    pub active_scene: SpatialScene,
    pub viewport_transform: ViewportTransform,
    pub audit_history: Vec<SpatialExecutionReceipt>,
    pub layout_epoch: u64,
}

impl Default for SpatialActionMediator {
    fn default() -> Self {
        Self::new(1440, 900, "win-main-1")
    }
}

impl SpatialActionMediator {
    pub fn new(width: u32, height: u32, window_id: impl Into<String>) -> Self {
        let win_id = window_id.into();
        Self {
            active_scene: SpatialScene::new(width, height, win_id),
            viewport_transform: ViewportTransform::new(width as f64, height as f64),
            audit_history: Vec::new(),
            layout_epoch: 1,
        }
    }

    pub fn update_scene(&mut self, scene: SpatialScene) {
        self.layout_epoch += 1;
        self.viewport_transform = ViewportTransform::new(scene.width as f64, scene.height as f64)
            .with_scroll(scene.scroll_offset[0] as f64, scene.scroll_offset[1] as f64)
            .with_dpr(scene.dpr);
        self.active_scene = scene;
        self.active_scene.layout_epoch = self.layout_epoch;
    }

    pub fn invalidate_layout(&mut self) {
        self.layout_epoch += 1;
        self.active_scene.layout_epoch = self.layout_epoch;
    }

    // ========================================================================
    // WRAPPER A — SPATIAL GROUNDING GATE
    // ========================================================================

    /// Resolves, normalizes, disambiguates, and validates a spatial target prior to action authorization.
    pub fn ground_action(&self, contract: &SpatialActionContract) -> SpatialGroundingResult {
        // 1. Stale-Epoch Fence
        if let Some(expected) = contract.expected_epoch {
            if expected != self.layout_epoch {
                return SpatialGroundingResult::failed(
                    SpatialError::StaleTargetEpoch { expected, current: self.layout_epoch },
                    Some(format!("Re-query spatial scene for fresh epoch {}", self.layout_epoch)),
                );
            }
        }

        // 2. Target Resolution (Token, Mark, Selector, or Coordinates)
        let fallback_window_target: SpatialTarget;
        let candidates = {
            let mut c = self.resolve_target_candidates(contract);
            let act_lower = contract.action_name.to_ascii_lowercase();
            let is_keyboard_act = matches!(act_lower.as_str(), "key" | "hotkey" | "press" | "tab" | "escape");
            let is_scroll_act = act_lower == "scroll";
            if c.is_empty() && (is_keyboard_act || is_scroll_act) {
                // If keyboard action has no explicit target, ground to active focus or active window plane;
                // if scroll action has no explicit target, ground to the active window plane.
                if is_keyboard_act {
                    if let Some(focused) = self.active_scene.targets.iter().find(|t| t.focused) {
                        c.push(focused);
                    }
                }
                if c.is_empty() {
                    fallback_window_target = SpatialTarget::new(
                        format!("window-{}", self.active_scene.window_id),
                        None,
                        "window",
                        "Active Window",
                        [0, 0, self.active_scene.width as i32, self.active_scene.height as i32],
                    );
                    c.push(&fallback_window_target);
                }
            }
            c
        };

        if candidates.is_empty() {
            let id = contract.target_selector.clone().or_else(|| contract.som_id.map(|s| s.to_string())).unwrap_or_else(|| "unspecified".into());
            return SpatialGroundingResult::failed(
                SpatialError::TargetNotFound(id),
                Some("Perform capture or som element scan to locate target".into()),
            );
        }

        if candidates.len() > 1 && contract.som_id.is_none() && !contract.target_selector.as_deref().unwrap_or("").starts_with('s') {
            let candidate_ids: Vec<String> = candidates.iter().map(|c| c.id.clone()).collect();
            // If candidates share identical bounds or overlap, flag ambiguity unless disambiguated
            if candidate_ids.len() > 1 && candidates[0].to_spatial_rect().area() == candidates[1].to_spatial_rect().area() {
                return SpatialGroundingResult::ambiguous(candidate_ids);
            }
        }

        let target = &candidates[0];

        // 3. Confidence Verification
        if target.confidence < contract.required_confidence {
            return SpatialGroundingResult::failed(
                SpatialError::ConfidenceTooLow {
                    target_id: target.id.clone(),
                    confidence: target.confidence,
                    required: contract.required_confidence,
                },
                Some("Elevate perception quality or use explicit Set-of-Marks badge".into()),
            );
        }

        // 4. Bounding Box Geometry Validity
        let target_rect = target.to_spatial_rect();
        if !target_rect.is_valid() {
            return SpatialGroundingResult::failed(
                SpatialError::TargetOutOfBounds {
                    target_id: target.id.clone(),
                    bounds: target.bounds,
                },
                Some("Bounding box coordinates must be finite with width/height > 0".into()),
            );
        }

        // 5. Viewport Transform & Visibility
        let vp_rect = self.viewport_transform.element_to_viewport_rect(&target_rect);
        let visibility = self.viewport_transform.visibility_ratio(&target_rect);
        let (requires_scroll, scroll_delta) = if visibility <= 0.05 {
            let (dx, dy) = self.viewport_transform.compute_centering_scroll_delta(&target_rect);
            (true, (dx, dy))
        } else {
            (false, (0.0, 0.0))
        };

        // 6. Occlusion Detection against Higher Z-Order Targets (excluding non-blocking transparent layers)
        let mut occluding_intersections = Vec::new();
        let mut primary_occluder = None;
        for other in &self.active_scene.targets {
            if other.z_order > target.z_order && other.id != target.id {
                let is_non_blocking = !other.enabled
                    || other.role.eq_ignore_ascii_case("none")
                    || other.role.eq_ignore_ascii_case("presentation")
                    || other.role.eq_ignore_ascii_case("transparent_overlay")
                    || other.role.eq_ignore_ascii_case("non_blocking")
                    || other.role.eq_ignore_ascii_case("tooltip_mask");
                if !is_non_blocking {
                    let other_rect = other.to_spatial_rect();
                    if let Some(inter) = target_rect.intersection(&other_rect) {
                        if inter.area() > 0.0 {
                            occluding_intersections.push(inter);
                            if primary_occluder.is_none() {
                                primary_occluder = Some(other.id.clone());
                            }
                        }
                    }
                }
            }
        }
        let total_occluded_area = if occluding_intersections.is_empty() {
            0.0
        } else if occluding_intersections.len() == 1 {
            occluding_intersections[0].area()
        } else if occluding_intersections.len() == 2 {
            let inter_area = occluding_intersections[0].intersection(&occluding_intersections[1]).map(|i| i.area()).unwrap_or(0.0);
            (occluding_intersections[0].area() + occluding_intersections[1].area() - inter_area).max(0.0)
        } else {
            calculate_rectangles_union_area(&occluding_intersections)
        };
        let occlusion_ratio = if target.role == "window" {
            0.0
        } else if target_rect.area() > 0.0 {
            (total_occluded_area / target_rect.area()).clamp(0.0, 1.0)
        } else {
            1.0
        };

        if occlusion_ratio > contract.max_occlusion {
            return SpatialGroundingResult::failed(
                SpatialError::TargetOccluded {
                    target_id: target.id.clone(),
                    occlusion_ratio,
                    occluded_by: primary_occluder.unwrap_or_else(|| "unknown".into()),
                },
                Some("Dismiss overlapping modal/dialog or bring target window to front".into()),
            );
        }

        // 7. Safe Center Calculation with 15% Margins
        let safe_pt = target_rect.safe_center(0.15).to_i32();

        SpatialGroundingResult::success(GroundedSpatialTarget {
            target: (*target).clone(),
            safe_point: safe_pt,
            screen_bounds: vp_rect.to_i32_slice(),
            visibility_ratio: visibility,
            occlusion_ratio,
            occluding_target_id: primary_occluder,
            confidence: target.confidence,
            requires_scroll,
            scroll_delta,
        })
    }

    fn resolve_target_candidates<'a>(&'a self, contract: &SpatialActionContract) -> Vec<&'a SpatialTarget> {
        // A. Match by exact som_id
        if let Some(sid) = contract.som_id {
            if let Some(t) = self.active_scene.find_by_som(sid) {
                return vec![t];
            }
        }

        // B. Match by element_token (e.g. "s1:2", "#2", "som:2") or selector / ID / label
        if let Some(ref sel) = contract.target_selector {
            let trimmed = sel.trim();
            if let Some(tok) = trimmed.strip_prefix('s') {
                if let Some((_, sid_str)) = tok.split_once(':') {
                    if let Ok(sid) = sid_str.parse::<u32>() {
                        if let Some(t) = self.active_scene.find_by_som(sid) {
                            return vec![t];
                        }
                    }
                }
            }
            if let Some(stripped) = trimmed.strip_prefix('#').or_else(|| trimmed.strip_prefix("som:")) {
                if let Ok(sid) = stripped.parse::<u32>() {
                    if let Some(t) = self.active_scene.find_by_som(sid) {
                        return vec![t];
                    }
                }
            }
            let by_id = self.active_scene.targets.iter().filter(|t| t.id == trimmed).collect::<Vec<_>>();
            if !by_id.is_empty() {
                return by_id;
            }
            let by_label = self.active_scene.targets.iter().filter(|t| t.label.eq_ignore_ascii_case(trimmed)).collect::<Vec<_>>();
            if !by_label.is_empty() {
                return by_label;
            }
        }

        // C. Match by explicit coordinates (excluding scroll where coordinates encode [delta_x, delta_y])
        if !contract.action_name.eq_ignore_ascii_case("scroll") {
            if let Some([cx, cy]) = contract.coordinates {
                let dim_w = self.active_scene.width as f64;
                let dim_h = self.active_scene.height as f64;
                let is_norm_1000 = contract.coordinate_space == "normalized_1000";
                let nx = normalize_coordinate_value(Some(&serde_json::json!(cx)), dim_w, cx, is_norm_1000);
                let ny = normalize_coordinate_value(Some(&serde_json::json!(cy)), dim_h, cy, is_norm_1000);
                return self.active_scene.find_at_point(nx.round() as i32, ny.round() as i32);
            }
        }

        Vec::new()
    }

    // ========================================================================
    // WRAPPER B — EXECUTION ASSURANCE GATE
    // ========================================================================

    /// Validates execution preconditions, role permissions, and coordinate containment before dispatch.
    pub fn assure_pre_execution(&self, grounded: &GroundedSpatialTarget, contract: &SpatialActionContract) -> Result<(), SpatialError> {
        let act = contract.action_name.to_ascii_lowercase();
        // Keyboard and viewport scroll inputs target the active window or focus plane and bypass pointer coordinate containment
        if matches!(act.as_str(), "key" | "hotkey" | "press" | "tab" | "escape" | "scroll") {
            return Ok(());
        }

        if !grounded.target.enabled {
            return Err(SpatialError::TargetDisabled(grounded.target.id.clone()));
        }

        // Permitted action by role check
        if (act == "type" || act == "set_value" || act == "fill") && !grounded.target.is_input_capable() && !grounded.target.focused {
            return Err(SpatialError::ActionNotPermittedForRole {
                role: grounded.target.role.clone(),
                action: act,
            });
        }

        // Coordinate containment check
        let safe_pt = SpatialPoint::from_i32(grounded.safe_point);
        let target_rect = grounded.target.to_spatial_rect();
        if !target_rect.contains_point(&safe_pt) {
            return Err(SpatialError::ContainmentViolation {
                point: grounded.safe_point,
                bounds: grounded.target.bounds,
            });
        }

        Ok(())
    }

    /// Independently evaluates observable postconditions following execution.
    pub fn verify_post_execution(
        &mut self,
        action: &str,
        grounded: Option<&GroundedSpatialTarget>,
        before: &SpatialScene,
        after: &SpatialScene,
        duration_ms: u64,
    ) -> SpatialExecutionReceipt {
        let (verdict, postconditions_met, recovery_suggestion) = match action {
            "click" | "double_click" => {
                if let Some(g) = grounded {
                    let target_focused_after = after.targets.iter().find(|t| t.id == g.target.id).map(|t| t.focused).unwrap_or(false);
                    let url_changed = before.window_id != after.window_id || before.frame_seq != after.frame_seq;
                    if target_focused_after || url_changed {
                        (AssuranceVerdict::Confirmed, true, None)
                    } else {
                        (
                            AssuranceVerdict::SuspectedNoop,
                            false,
                            Some("Click produced no focus change or frame update; escalate delivery_mode to foreground".into()),
                        )
                    }
                } else {
                    (AssuranceVerdict::Confirmed, true, None)
                }
            }
            "type" | "set_value" | "fill" => {
                if let Some(g) = grounded {
                    let new_val = after.targets.iter().find(|t| t.id == g.target.id).map(|t| t.value.as_str()).unwrap_or("");
                    let old_val = before.targets.iter().find(|t| t.id == g.target.id).map(|t| t.value.as_str()).unwrap_or("");
                    if new_val != old_val || !new_val.is_empty() {
                        (AssuranceVerdict::Confirmed, true, None)
                    } else {
                        (
                            AssuranceVerdict::SuspectedNoop,
                            false,
                            Some("Value did not update in target element; verify focus before typing".into()),
                        )
                    }
                } else {
                    (AssuranceVerdict::Confirmed, true, None)
                }
            }
            "scroll" => {
                if before.scroll_offset != after.scroll_offset {
                    (AssuranceVerdict::Confirmed, true, None)
                } else {
                    (AssuranceVerdict::SuspectedNoop, false, Some("Scroll offset did not change; document may be at boundary".into()))
                }
            }
            _ => (AssuranceVerdict::Confirmed, true, None),
        };

        let receipt = SpatialExecutionReceipt {
            action: action.to_string(),
            target_id: grounded.map(|g| g.target.id.clone()),
            executed_point: grounded.map(|g| g.safe_point).unwrap_or([0, 0]),
            verdict,
            postconditions_met,
            duration_ms,
            layout_epoch: self.layout_epoch,
            frame_seq: after.frame_seq,
            recovery_suggestion,
        };

        self.audit_history.push(receipt.clone());
        receipt
    }

    /// Plans a multi-step spatially grounded sequence (e.g. scroll into view -> click -> type).
    pub fn plan_action(&self, contract: &SpatialActionContract) -> SpatialActionPlan {
        let grounding = self.ground_action(contract);
        let mut steps = Vec::new();
        let target_id = contract.target_selector.clone().unwrap_or_else(|| "target".into());

        if let Some(ref g) = grounding.grounded {
            if g.requires_scroll {
                steps.push(SpatialActionContract {
                    action_name: "scroll".into(),
                    coordinates: Some([g.scroll_delta.0, g.scroll_delta.1]),
                    ..SpatialActionContract::default()
                });
            }
            if contract.action_name == "type" && !g.target.focused {
                steps.push(SpatialActionContract {
                    action_name: "click".into(),
                    target_selector: Some(g.target.id.clone()),
                    ..SpatialActionContract::default()
                });
            }
        }
        steps.push(contract.clone());

        SpatialActionPlan {
            steps,
            target_id,
            estimated_cost_tokens: 120,
            recovery_policy: "automatic_scroll_and_retry".into(),
        }
    }
}

/// Multi-step spatially grounded execution plan with pre-computed dependencies and recovery policies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialActionPlan {
    pub steps: Vec<SpatialActionContract>,
    pub target_id: String,
    pub estimated_cost_tokens: u32,
    pub recovery_policy: String,
}

/// Comprehensive audit summary of all mediated spatial actions across the interaction session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialInteractionAudit {
    pub total_actions: usize,
    pub confirmed_count: usize,
    pub suspected_noop_count: usize,
    pub success_rate: f64,
    pub receipts: Vec<SpatialExecutionReceipt>,
}

impl SpatialActionMediator {
    pub fn generate_audit(&self) -> SpatialInteractionAudit {
        let total = self.audit_history.len();
        let confirmed = self.audit_history.iter().filter(|r| r.verdict == AssuranceVerdict::Confirmed).count();
        let suspected = self.audit_history.iter().filter(|r| r.verdict == AssuranceVerdict::SuspectedNoop).count();
        let rate = if total == 0 { 1.0 } else { confirmed as f64 / total as f64 };
        SpatialInteractionAudit {
            total_actions: total,
            confirmed_count: confirmed,
            suspected_noop_count: suspected,
            success_rate: rate,
            receipts: self.audit_history.clone(),
        }
    }
}

impl crate::domain::interfaces::ISpatialMediator for SpatialActionMediator {
    fn ground_contract(&self, contract: &SpatialActionContract) -> SpatialGroundingResult {
        self.ground_action(contract)
    }
}

impl crate::domain::interfaces::ISpatialGroundingGate for SpatialActionMediator {
    fn ground(&self, contract: &SpatialActionContract) -> SpatialGroundingResult {
        self.ground_action(contract)
    }
}

impl crate::domain::interfaces::IExecutionAssuranceGate for SpatialActionMediator {
    fn assure_pre(&self, grounded: &GroundedSpatialTarget, contract: &SpatialActionContract) -> Result<(), SpatialError> {
        self.assure_pre_execution(grounded, contract)
    }

    fn verify_post(&mut self, action: &str, grounded: Option<&GroundedSpatialTarget>, before: &SpatialScene, after: &SpatialScene, duration_ms: u64) -> SpatialExecutionReceipt {
        self.verify_post_execution(action, grounded, before, after, duration_ms)
    }
}