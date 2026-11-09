// [LAYER: CORE]
use crate::domain::computer_use::VirtualDisplayState;
use crate::domain::spatial_action::SpatialGroundingResult;
use crate::domain::spatial_mediator::SpatialActionMediator;
use crate::domain::types::spatial_types::{SpatialActionContract, SpatialScene, SpatialTarget, TargetProvenance};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// Core orchestration engine for spatially grounded interactions, cross-modal target reconciliation,
/// and Double-Wrapped Action Mediation across GUI and DOM runtimes.
pub struct SpatialInteractionEngine {
    pub mediator: Mutex<SpatialActionMediator>,
    pub display: Arc<Mutex<VirtualDisplayState>>,
}

impl SpatialInteractionEngine {
    pub extern "Rust" fn new(display: Arc<Mutex<VirtualDisplayState>>) -> Self {
        let (w, h, active_id) = {
            let disp = display.lock().unwrap();
            (disp.width, disp.height, disp.active_window_id.clone())
        };
        let mut mediator = SpatialActionMediator::new(w, h, active_id);
        // Synchronize initial scene from display
        {
            let disp = display.lock().unwrap();
            let initial_scene = Self::extract_scene_from_display(&disp);
            mediator.update_scene(initial_scene);
        }
        Self {
            mediator: Mutex::new(mediator),
            display,
        }
    }

    /// Extracts a unified `SpatialScene` from the active window and elements in `VirtualDisplayState`.
    pub fn extract_scene_from_display(display: &VirtualDisplayState) -> SpatialScene {
        let mut scene = SpatialScene::new(display.width, display.height, &display.active_window_id);
        scene.frame_seq = display.frame_seq;
        scene.scroll_offset = display.scroll_offset;

        if let Some(win) = display.windows.iter().find(|w| w.id == display.active_window_id) {
            for el in &win.elements {
                let mut target = SpatialTarget::new(
                    el.id.clone(),
                    Some(el.som_id),
                    el.role.clone(),
                    el.label.clone(),
                    el.bounds,
                );
                target.value = el.value.clone();
                target.enabled = el.enabled;
                target.focused = el.focused;
                target.z_order = win.z_order;
                target.provenance = TargetProvenance::SetOfMarks;
                target.confidence = 1.0;
                scene.targets.push(target);
            }
        }
        for win in &display.windows {
            let tl = win.title.to_ascii_lowercase();
            if win.id != display.active_window_id
                && (tl.contains("dialog") || tl.contains("alert") || tl.contains("confirm") || tl.contains("modal"))
            {
                for el in &win.elements {
                    if !scene.targets.iter().any(|t| t.id == el.id) {
                        let mut target = SpatialTarget::new(
                            el.id.clone(),
                            Some(el.som_id),
                            el.role.clone(),
                            el.label.clone(),
                            el.bounds,
                        );
                        target.value = el.value.clone();
                        target.enabled = el.enabled;
                        target.focused = el.focused;
                        target.z_order = win.z_order.max(10);
                        target.provenance = TargetProvenance::SetOfMarks;
                        target.confidence = 1.0;
                        scene.targets.push(target);
                    }
                }
            }
        }
        scene
    }

    /// Reconciles Set-of-Marks visual elements with DOM HTML snapshot interactive elements.
    /// Elements that appear in both modalities receive boosted confidence and cross-modal provenance.
    pub fn reconcile_dom_and_visual(&self, dom_elements: &[Value]) -> SpatialScene {
        let disp = self.display.lock().unwrap();
        let mut scene = Self::extract_scene_from_display(&disp);
        let cur_epoch = scene.layout_epoch;

        for dom_el in dom_elements {
            let som_id = dom_el.get("somId").and_then(|v| v.as_u64()).map(|n| n as u32);
            let elem_id = dom_el.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let role = dom_el.get("role").and_then(|v| v.as_str()).unwrap_or("generic");
            let label = dom_el.get("label").and_then(|v| v.as_str()).unwrap_or("");

            // Try to match existing visual target by som_id or element_id
            if let Some(existing) = scene.targets.iter_mut().find(|t| (som_id.is_some() && t.som_id == som_id) || (!elem_id.is_empty() && t.id == elem_id)) {
                existing.provenance = TargetProvenance::DomTree;
                existing.confidence = 1.0;
                if !label.is_empty() && existing.label.is_empty() {
                    existing.label = label.to_string();
                }
            } else if !elem_id.is_empty() || som_id.is_some() {
                // Synthesize new spatial target with heuristic bounds if only in DOM
                let synth_id = if !elem_id.is_empty() { elem_id.to_string() } else { format!("dom-{}", som_id.unwrap_or(0)) };
                let mut target = SpatialTarget::new(
                    synth_id,
                    som_id,
                    role,
                    label,
                    [100, 100, 200, 40], // Default normalized layout slot
                );
                target.provenance = TargetProvenance::DomTree;
                target.confidence = 0.85;
                target.layout_epoch = cur_epoch;
                scene.targets.push(target);
            }
        }

        let mut med = self.mediator.lock().unwrap();
        med.update_scene(scene.clone());
        scene
    }

    /// Dispatches an action through the complete Double-Wrapped Spatial Action Architecture:
    /// Wrapper A (Spatial Grounding) -> Wrapper B Pre (Preconditions & Containment) ->
    /// Native Dispatch -> Wrapper B Post (Observable Postcondition Verification).
    pub fn execute_spatially_grounded_action(&self, contract: &SpatialActionContract) -> Result<Value, String> {
        // Step 1: Refresh scene state from display
        let before_scene = {
            let disp = self.display.lock().unwrap();
            let fresh = Self::extract_scene_from_display(&disp);
            let mut med = self.mediator.lock().unwrap();
            med.update_scene(fresh.clone());
            fresh
        };

        // Step 2: WRAPPER A — Spatial Grounding
        let grounding = {
            let med = self.mediator.lock().unwrap();
            med.ground_action(contract)
        };

        let mut grounded = match grounding {
            SpatialGroundingResult { success: true, grounded: Some(g), .. } => g,
            SpatialGroundingResult { error: Some(err), recovery_hint, ambiguity_candidates, .. } => {
                return Ok(json!({
                    "success": false,
                    "groundingPhase": "rejected",
                    "error": err.to_string(),
                    "ambiguityCandidates": ambiguity_candidates,
                    "recoveryHint": recovery_hint,
                    "verdict": "grounding_failed"
                }));
            }
            _ => {
                return Err("Internal spatial grounding error".to_string());
            }
        };

        // Step 2b: Automatic Off-Screen Pre-Scroll Remediation
        let mut auto_scrolled = false;
        if grounded.requires_scroll
            && contract.auto_scroll_if_needed
            && !contract.action_name.eq_ignore_ascii_case("scroll")
        {
            let pre_scroll_args = json!({
                "action": "scroll",
                "delta_x": grounded.scroll_delta.0.round() as i32,
                "delta_y": grounded.scroll_delta.1.round() as i32,
                "delivery_mode": contract.delivery_mode,
            });
            let scrolled_scene = {
                let mut disp = self.display.lock().unwrap();
                let _ = disp.execute_action(&pre_scroll_args)?;
                Self::extract_scene_from_display(&disp)
            };
            let mut med = self.mediator.lock().unwrap();
            med.update_scene(scrolled_scene);
            let mut reground_contract = contract.clone();
            reground_contract.expected_epoch = None;
            if let Some(regrounded) = med.ground_action(&reground_contract).grounded {
                grounded = regrounded;
                auto_scrolled = true;
            }
        }

        // Step 3: WRAPPER B (Pre) — Execution Assurance Preconditions
        {
            let med = self.mediator.lock().unwrap();
            if let Err(pre_err) = med.assure_pre_execution(&grounded, contract) {
                return Ok(json!({
                    "success": false,
                    "assurancePhase": "pre_execution_failed",
                    "error": pre_err.to_string(),
                    "groundedTarget": grounded,
                    "verdict": "assurance_precondition_failed"
                }));
            }
        }

        // Step 4: Dispatched Execution on Underlying Display Runtime
        let start_time = crate::plumbing::hash_and_math::now_ms();
        let (scroll_dx, scroll_dy) = contract
            .coordinates
            .map(|[dx, dy]| (dx.round() as i32, dy.round() as i32))
            .unwrap_or((0, 240));
        let dispatch_args = json!({
            "action": contract.action_name,
            "x": grounded.safe_point[0],
            "y": grounded.safe_point[1],
            "delta_x": scroll_dx,
            "delta_y": scroll_dy,
            "element_id": grounded.target.id,
            "som_id": grounded.target.som_id,
            "text": contract.payload_text.clone().unwrap_or_default(),
            "keys": contract.keys.clone().unwrap_or_default(),
            "delivery_mode": contract.delivery_mode,
        });

        let raw_res = {
            let mut disp = self.display.lock().unwrap();
            disp.execute_action(&dispatch_args)
        }?;

        let duration_ms = crate::plumbing::hash_and_math::now_ms().saturating_sub(start_time).max(1);

        // Step 5: WRAPPER B (Post) — Observable Postcondition Verification
        let (after_scene, receipt) = {
            let disp = self.display.lock().unwrap();
            let after = Self::extract_scene_from_display(&disp);
            let mut med = self.mediator.lock().unwrap();
            let rcpt = med.verify_post_execution(&contract.action_name, Some(&grounded), &before_scene, &after, duration_ms);
            (after, rcpt)
        };

        Ok(json!({
            "success": receipt.postconditions_met,
            "action": contract.action_name,
            "autoScrolled": auto_scrolled,
            "groundedTarget": {
                "id": grounded.target.id,
                "somId": grounded.target.som_id,
                "role": grounded.target.role,
                "label": grounded.target.label,
                "safePoint": grounded.safe_point,
                "screenBounds": grounded.screen_bounds,
                "occlusionRatio": grounded.occlusion_ratio,
                "visibilityRatio": grounded.visibility_ratio,
                "confidence": grounded.confidence,
                "provenance": format!("{:?}", grounded.target.provenance)
            },
            "assuranceReceipt": receipt,
            "rawResult": raw_res,
            "frameSeq": after_scene.frame_seq,
            "layoutEpoch": after_scene.layout_epoch
        }))
    }

    /// Generates and executes a multi-step `SpatialActionPlan` (e.g., scroll into view -> focus click -> type)
    /// through the Double-Wrapped Spatial Action Architecture.
    pub extern "Rust" fn execute_plan(&self, contract: &SpatialActionContract) -> Result<Value, String> {
        let plan = {
            let disp = self.display.lock().unwrap();
            let fresh = Self::extract_scene_from_display(&disp);
            let mut med = self.mediator.lock().unwrap();
            med.update_scene(fresh);
            med.plan_action(contract)
        };

        let mut step_results = Vec::with_capacity(plan.steps.len());
        let mut all_succeeded = true;
        for mut step in plan.steps.clone() {
            // Since the plan itself explicitly schedules any required scroll step first,
            // disable redundant auto-scroll on subsequent steps of the plan.
            step.auto_scroll_if_needed = false;
            step.expected_epoch = None;
            let step_res = self.execute_spatially_grounded_action(&step)?;
            let step_ok = step_res.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            step_results.push(step_res);
            if !step_ok {
                all_succeeded = false;
                break;
            }
        }

        Ok(json!({
            "success": all_succeeded,
            "targetId": plan.target_id,
            "stepCount": step_results.len(),
            "plannedSteps": plan.steps.len(),
            "recoveryPolicy": plan.recovery_policy,
            "steps": step_results
        }))
    }

    /// Spatial directional query resolving relationships like `right_of`, `left_of`, `below`, `above`.
    pub fn query_directional(&self, origin_id: &str, direction: &str) -> Option<SpatialTarget> {
        let med = self.mediator.lock().unwrap();
        let origin = med.active_scene.find_by_id(origin_id)?;
        let origin_rect = origin.to_spatial_rect();
        let origin_center = origin_rect.center();

        let mut candidates: Vec<&SpatialTarget> = med.active_scene.targets
            .iter()
            .filter(|t| t.id != origin_id)
            .filter(|t| {
                let r = t.to_spatial_rect();
                match direction {
                    "right" | "right_of" => r.is_to_right_of(&origin_rect),
                    "left" | "left_of" => r.is_to_left_of(&origin_rect),
                    "below" | "under" => r.is_below(&origin_rect),
                    "above" | "over" => r.is_above(&origin_rect),
                    _ => false,
                }
            })
            .collect();

        // Sort by Euclidean distance from origin center
        candidates.sort_by(|a, b| {
            let dist_a = a.to_spatial_rect().center().distance_sq(&origin_center);
            let dist_b = b.to_spatial_rect().center().distance_sq(&origin_center);
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates.first().map(|t| (*t).clone())
    }

    /// Audits health and statistics of the spatial action architecture.
    pub extern "Rust" fn audit_health(&self) -> Value {
        let med = self.mediator.lock().unwrap();
        let audit = med.generate_audit();
        json!({
            "healthy": audit.success_rate >= 0.90,
            "totalMediatedActions": audit.total_actions,
            "confirmedActions": audit.confirmed_count,
            "suspectedNoopActions": audit.suspected_noop_count,
            "successRate": audit.success_rate,
            "activeTargetCount": med.active_scene.target_count(),
            "currentLayoutEpoch": med.layout_epoch,
            "windowId": med.active_scene.window_id
        })
    }
}