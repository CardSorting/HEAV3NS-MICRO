// [LAYER: CORE]
use crate::core::spatial_engine::SpatialInteractionEngine;
use crate::domain::computer_use::VirtualDisplayState;
use crate::domain::interfaces::{IBehavioralKnowledgeStore, IComputerUseOrchestrator, IEnvironmentalDiffer, IEnvironmentalObserver, ISpatialGeometry, ISwarmIntelligenceConsultant};
use crate::domain::types::environmental_types::{BehavioralAffordance, EnvironmentalObservation, EnvironmentalRegion, RegionKind};
use crate::domain::types::learned_knowledge_types::LearnedWorkflowProcedure;
use crate::domain::types::spatial_types::SpatialActionContract;
use crate::domain::types::swarm_intelligence_types::{ConsultationRiskLevel, SpecialistInsight, SwarmConsultationRequest, SwarmConsultationResponse};
use crate::infrastructure::behavioral_store::BehavioralKnowledgeStore;
use crate::plumbing::environmental_diff::{analyze_spider_structural_relationships, classify_element_affordance, compute_element_deltas, detect_modal_or_dialog, diagnose_failure_cause};
use crate::plumbing::hash_and_math::{fnv1a_32, now_ms};
use crate::plumbing::secret_redactor::SafetyPolicy;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Sovereign closed-loop Computer Use orchestrator coordinating environmental intelligence,
/// double-wrapped spatial action mediation, Spider structural hierarchy, Rewind state reconciliation,
/// and Synthetic Swarm collective intelligence.
pub struct ComputerUseOrchestrator {
    pub display: Arc<Mutex<VirtualDisplayState>>,
    pub spatial_engine: Arc<SpatialInteractionEngine>,
    pub knowledge_store: Arc<Mutex<BehavioralKnowledgeStore>>,
    pub failure_counts: Mutex<HashMap<String, u32>>,
    pub last_checkpoint_epoch: Mutex<u64>,
    pub consultation_cache: Mutex<HashMap<String, SwarmConsultationResponse>>,
    pub session_steps: Mutex<Vec<String>>,
}

impl ComputerUseOrchestrator {
    pub extern "Rust" fn new(
        display: Arc<Mutex<VirtualDisplayState>>,
        spatial_engine: Arc<SpatialInteractionEngine>,
        knowledge_store: Arc<Mutex<BehavioralKnowledgeStore>>,
    ) -> Self {
        Self {
            display,
            spatial_engine,
            knowledge_store,
            failure_counts: Mutex::new(HashMap::new()),
            last_checkpoint_epoch: Mutex::new(1),
            consultation_cache: Mutex::new(HashMap::new()),
            session_steps: Mutex::new(Vec::new()),
        }
    }

    // ========================================================================
    // 1. OBSERVE & ORIENT
    // ========================================================================

    /// Captures the complete multi-modal environmental state from the active virtual display.
    pub fn observe(&self) -> EnvironmentalObservation {
        let disp = self.display.lock().unwrap();
        let active_win = disp.windows.iter().find(|w| w.id == disp.active_window_id).or_else(|| disp.windows.first());
        let (win_id, app, title, mut elements) = match active_win {
            Some(w) => (w.id.clone(), w.app.clone(), w.title.clone(), w.elements.clone()),
            None => ("none".into(), "none".into(), "none".into(), Vec::new()),
        };

        // Also incorporate elements from any active modal/dialog windows in the display
        let mut modal_window_element_ids = Vec::new();
        for w in &disp.windows {
            let t = w.title.to_ascii_lowercase();
            if w.id != win_id && (t.contains("dialog") || t.contains("alert") || t.contains("confirm") || t.contains("modal")) {
                for el in &w.elements {
                    modal_window_element_ids.push(el.id.clone());
                    if !elements.iter().any(|existing| existing.id == el.id) {
                        elements.push(el.clone());
                    }
                }
            }
        }

        let layout_epoch = self.spatial_engine.mediator.lock().unwrap().layout_epoch;
        let mut obs = EnvironmentalObservation::new(win_id, app, title, disp.frame_seq, layout_epoch);
        obs.cursor = disp.cursor;
        obs.elements = elements.clone();
        obs.timestamp_ms = now_ms();

        // Identify active focus
        obs.active_focus_id = elements.iter().find(|e| e.focused).map(|e| e.id.clone());

        // Derive functional viewport regions (HeaderNav, Sidebar, Toolbar, FooterStatusBar, ContentArea)
        let mut header_ids = Vec::new();
        let mut sidebar_ids = Vec::new();
        let mut toolbar_ids = Vec::new();
        let mut footer_ids = Vec::new();
        for el in &elements {
            if modal_window_element_ids.contains(&el.id) {
                continue;
            }
            let r = el.role.to_ascii_lowercase();
            let id_l = el.id.to_ascii_lowercase();
            let lbl_l = el.label.to_ascii_lowercase();
            if matches!(r.as_str(), "navigation" | "banner" | "menubar") || id_l.contains("header") || id_l.contains("navbar") {
                header_ids.push(el.id.clone());
            } else if matches!(r.as_str(), "complementary" | "tree" | "sidebar") || id_l.contains("sidebar") || lbl_l.contains("sidebar") {
                sidebar_ids.push(el.id.clone());
            } else if r == "toolbar" || id_l.contains("toolbar") {
                toolbar_ids.push(el.id.clone());
            } else if matches!(r.as_str(), "status" | "contentinfo") || id_l.contains("status") || id_l.contains("footer") {
                footer_ids.push(el.id.clone());
            }
        }

        if !header_ids.is_empty() {
            let mut reg = EnvironmentalRegion::new("reg-header", RegionKind::HeaderNav, [0, 0, disp.width as i32, 64], 2);
            reg.element_ids = header_ids;
            obs.regions.push(reg);
        }
        if !sidebar_ids.is_empty() {
            let mut reg = EnvironmentalRegion::new("reg-sidebar", RegionKind::Sidebar, [0, 64, 260, disp.height as i32 - 64], 2);
            reg.element_ids = sidebar_ids;
            obs.regions.push(reg);
        }
        if !toolbar_ids.is_empty() {
            let mut reg = EnvironmentalRegion::new("reg-toolbar", RegionKind::Toolbar, [0, 64, disp.width as i32, 48], 3);
            reg.element_ids = toolbar_ids;
            obs.regions.push(reg);
        }
        if !footer_ids.is_empty() {
            let mut reg = EnvironmentalRegion::new("reg-footer", RegionKind::FooterStatusBar, [0, disp.height as i32 - 40, disp.width as i32, 40], 2);
            reg.element_ids = footer_ids;
            obs.regions.push(reg);
        }

        let mut content_region = EnvironmentalRegion::new("reg-main", RegionKind::ContentArea, [0, 0, disp.width as i32, disp.height as i32], 1);
        content_region.element_ids = elements.iter().map(|e| e.id.clone()).collect();
        obs.regions.push(content_region);

        // Check for modal dialogs blocking interaction
        let titles: Vec<&str> = disp.windows.iter().map(|w| w.title.as_str()).collect();
        let roles: Vec<&str> = elements.iter().map(|e| e.role.as_str()).collect();
        let labels: Vec<&str> = elements.iter().map(|e| e.label.as_str()).collect();
        if let Some(modal_title) = detect_modal_or_dialog(&titles, &roles, &labels) {
            obs.modal_detected = true;
            obs.modal_container_id = Some(modal_title.clone());

            // Determine dynamic modal bounds from inline modal container or modal window elements
            let inline_modal_bounds = elements
                .iter()
                .find(|e| matches!(e.role.to_ascii_lowercase().as_str(), "dialog" | "alertdialog" | "modal") && e.bounds[2] > 0 && e.bounds[3] > 0)
                .map(|e| e.bounds);
            let modal_bounds = inline_modal_bounds.unwrap_or([200, 150, 1040, 600]);
            let mut modal_region = EnvironmentalRegion::new("reg-modal", RegionKind::ModalDialog, modal_bounds, 99);

            if !modal_window_element_ids.is_empty() {
                // Separate modal window: only elements from the modal window(s) or explicit modal roles belong to the modal
                for eid in &modal_window_element_ids {
                    if !modal_region.element_ids.contains(eid) {
                        modal_region.element_ids.push(eid.clone());
                    }
                }
                for el in &elements {
                    let r = el.role.to_ascii_lowercase();
                    if (r == "dialog" || r == "alertdialog" || r == "modal") && !modal_region.element_ids.contains(&el.id) {
                        modal_region.element_ids.push(el.id.clone());
                    }
                }
            } else {
                // Inline DOM modal within the active window: use actual inline modal bounds
                let (mx, my, mw, mh) = (modal_bounds[0], modal_bounds[1], modal_bounds[2], modal_bounds[3]);
                for el in &elements {
                    let r = el.role.to_ascii_lowercase();
                    let is_modal_role = r == "dialog" || r == "alertdialog" || r == "modal";
                    let is_full_bleed_bg = el.bounds[0] == 0 && el.bounds[1] == 0 && el.bounds[2] >= disp.width as i32 && el.bounds[3] >= disp.height as i32;
                    let cx = el.bounds[0] + (el.bounds[2] / 2);
                    let cy = el.bounds[1] + (el.bounds[3] / 2);
                    let inside_bounds = !is_full_bleed_bg && cx >= mx && cx <= (mx + mw) && cy >= my && cy <= (my + mh);
                    if (is_modal_role || inside_bounds) && !modal_region.element_ids.contains(&el.id) {
                        modal_region.element_ids.push(el.id.clone());
                    }
                }
            }
            obs.regions.push(modal_region);
        }

        let state_bytes = serde_json::to_vec(&(&obs.window_id, &obs.frame_seq, &obs.active_focus_id, obs.elements.len())).unwrap_or_default();
        obs.frame_hash = format!("{:08x}", fnv1a_32(&state_bytes));

        obs
    }

    // ========================================================================
    // 2. INTERPRET & SPIDER STRUCTURAL ANALYSIS
    // ========================================================================

    /// Analyzes interface affordances and Spider structural relationships across observed elements.
    pub fn interpret(&self, obs: &EnvironmentalObservation) -> Value {
        let mut affordances = Vec::new();
        let mut raw_elements = Vec::new();

        for el in &obs.elements {
            let (aff_type, outcome, conf) = if !el.enabled {
                ("disabled_control".to_string(), "non_interactive_unless_enabled".to_string(), 0.98)
            } else {
                classify_element_affordance(&el.role, &el.label, &el.value, el.bounds)
            };
            let mut aff = BehavioralAffordance::new(el.id.clone(), aff_type, outcome, conf);
            if !el.enabled {
                aff.contextual_clues.push("element_disabled".into());
            }
            if el.focused {
                aff.contextual_clues.push("currently_focused".into());
            }
            if obs.modal_detected && obs.is_element_in_modal(&el.id) {
                aff.contextual_clues.push("inside_modal_dialog".into());
            }
            affordances.push(aff);

            raw_elements.push(json!({
                "id": el.id,
                "role": el.role,
                "label": el.label,
                "value": el.value,
                "bounds": el.bounds,
                "focused": el.focused,
                "enabled": el.enabled
            }));
        }

        let spider_relationships = analyze_spider_structural_relationships(&raw_elements);

        json!({
            "windowId": obs.window_id,
            "appName": obs.app_name,
            "layoutEpoch": obs.layout_epoch,
            "modalDetected": obs.modal_detected,
            "activeFocus": obs.active_focus_id,
            "affordances": affordances,
            "spiderStructuralRelationships": spider_relationships
        })
    }

    // ========================================================================
    // 3. SYNTHETIC SWARM COLLECTIVE CONSULTATION
    // ========================================================================

    /// Evaluates whether the situation requires collective swarm consultation
    /// and resolves multi-specialist recommendations with strict operator sovereignty.
    pub fn consult_swarm_if_needed(
        &self,
        obs: &EnvironmentalObservation,
        contract: &SpatialActionContract,
        uncertainty_reason: Option<&str>,
    ) -> Option<SwarmConsultationResponse> {
        let needs_consultation = uncertainty_reason.is_some()
            || obs.modal_detected
            || contract.target_selector.is_none()
            || contract.required_confidence > 0.85;

        if !needs_consultation {
            return None;
        }

        // Memoization cache key to prevent repeated consultation for unchanged conditions
        let cache_key = format!(
            "{}:{}:{}:{}:{}:{}",
            obs.frame_hash,
            obs.app_name,
            contract.action_name,
            contract.target_selector.as_deref().unwrap_or("*"),
            obs.modal_detected,
            uncertainty_reason.unwrap_or("none")
        );
        if let Ok(cache) = self.consultation_cache.lock() {
            if let Some(cached) = cache.get(&cache_key) {
                return Some(cached.clone());
            }
        }

        let cid = format!("consult-{}", now_ms());
        let mut req = SwarmConsultationRequest::new(cid, &contract.action_name, &obs.app_name, &obs.window_title);
        req.active_focus = obs.active_focus_id.clone();
        req.modal_blocking = obs.modal_detected;
        req.candidate_target_ids = obs.elements.iter().map(|e| e.id.clone()).collect();
        req.last_failed_action = if uncertainty_reason.is_some() {
            Some(contract.action_name.clone())
        } else {
            None
        };
        req.last_failure_reason = uncertainty_reason.map(|s| s.to_string());

        let resp = self.consult(&req);
        if let Ok(mut cache) = self.consultation_cache.lock() {
            if cache.len() >= 128 {
                cache.clear();
            }
            cache.insert(cache_key, resp.clone());
        }
        Some(resp)
    }

    // ========================================================================
    // 4. CLOSED-LOOP PIPELINE EXECUTION
    // ========================================================================

    /// Executes the full 9-stage closed-loop interaction cycle:
    /// OBSERVE -> ORIENT -> INTERPRET -> REASON -> PLAN -> ACT -> OBSERVE -> VERIFY -> LEARN.
    pub fn execute_closed_loop_cycle(
        &self,
        goal: &str,
        initial_contract: &SpatialActionContract,
    ) -> Result<Value, String> {
        // Step 1: OBSERVE & ORIENT
        let before_obs = self.observe();

        // Step 2: INTERPRET
        let interpretation = self.interpret(&before_obs);

        // Security check for destructive payloads
        if let Some(ref text) = initial_contract.payload_text {
            SafetyPolicy::check_command(text).map_err(|e| format!("Safety check failed: {e}"))?;
        }

        // Modal trap check: if modal dialog is trapping interface and target is outside modal, intercept
        let mut contract = initial_contract.clone();
        let mut modal_intercepted = false;
        if before_obs.modal_detected {
            let target_in_modal = contract.target_selector.as_ref().map(|tid| before_obs.is_element_in_modal(tid)).unwrap_or(false);
            if !target_in_modal && contract.action_name != "key" {
                // Auto-route to dismiss modal
                contract.action_name = "key".into();
                contract.keys = Some("Escape".into());
                contract.target_selector = None;
                contract.som_id = None;
                contract.coordinates = None;
                modal_intercepted = true;
            }
        }

        // Check loop breaker: prevent repeated identical failures
        let target_key = contract.target_selector.clone().unwrap_or_else(|| contract.action_name.clone());
        let current_fails = {
            let fc = self.failure_counts.lock().unwrap();
            fc.get(&target_key).copied().unwrap_or(0)
        };
        if current_fails >= 3 {
            return Err(format!(
                "Loop breaker engaged: action '{}' on target '{}' failed {} consecutive times. Breaking execution loop to prevent redundant failure.",
                contract.action_name, target_key, current_fails
            ));
        }

        // Step 3 & 4: REASON & CONSULT SWARM
        let uncertainty = if current_fails > 0 {
            Some("Previous action on this target failed or produced suspected no-op")
        } else if before_obs.modal_detected {
            Some("Modal dialog detected; requires focus containment or dismissal")
        } else {
            None
        };

        let swarm_response = self.consult_swarm_if_needed(&before_obs, &contract, uncertainty);

        // Reconcile swarm advice with operator authority: current environmental facts take strict precedence
        if let Some(ref swarm) = swarm_response {
            if current_fails >= 2 {
                // Adaptive escalation: switch to swarm recommended action
                contract.action_name = swarm.recommended_action.clone();
                // Grounding sovereignty: only adopt recommended target if initial target was unspecified or invalid
                if contract.target_selector.is_none() {
                    if let Some(ref rec_tgt) = swarm.recommended_target_id {
                        if before_obs.find_element_by_id(rec_tgt).is_some() {
                            contract.target_selector = Some(rec_tgt.clone());
                        }
                    }
                }
                contract.delivery_mode = "foreground".into();
            } else if contract.delivery_mode == "background" && current_fails >= 1 {
                contract.delivery_mode = "foreground".into();
            }
        }

        // Step 5: PLAN
        // Check if typing into an unfocused textbox; if so, plan auto-focus click
        let mut auto_focused = false;
        if (contract.action_name == "type" || contract.action_name == "set_value" || contract.action_name == "fill")
            && before_obs.active_focus_id.as_deref() != contract.target_selector.as_deref()
        {
            if let Some(ref tid) = contract.target_selector {
                let focus_click_contract = SpatialActionContract {
                    action_name: "click".into(),
                    target_selector: Some(tid.clone()),
                    delivery_mode: "foreground".into(),
                    ..SpatialActionContract::default()
                };
                if self.spatial_engine.execute_spatially_grounded_action(&focus_click_contract).is_ok() {
                    auto_focused = true;
                }
            }
        }

        // Save internal checkpoint epoch for Rewind reconciliation
        {
            let mut ckpt = self.last_checkpoint_epoch.lock().unwrap();
            *ckpt = before_obs.layout_epoch;
        }

        // Step 6: ACT (Dispatch via Double-Wrapped Spatial Engine)
        let execution_result = self.spatial_engine.execute_spatially_grounded_action(&contract)?;

        // Step 7: OBSERVE (Post-execution)
        let after_obs = self.observe();

        // Step 8: VERIFY (Post-execution Observable Assurance)
        let mut raw_before_el = Vec::new();
        for e in &before_obs.elements {
            raw_before_el.push(json!({"id": e.id, "value": e.value, "focused": e.focused, "bounds": e.bounds, "label": e.label, "enabled": e.enabled}));
        }
        let mut raw_after_el = Vec::new();
        for e in &after_obs.elements {
            raw_after_el.push(json!({"id": e.id, "value": e.value, "focused": e.focused, "bounds": e.bounds, "label": e.label, "enabled": e.enabled}));
        }
        let deltas = compute_element_deltas(&raw_before_el, &raw_after_el);

        let success = execution_result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        let receipt = execution_result.get("assuranceReceipt");
        let is_noop = receipt.and_then(|r| r.get("verdict")).and_then(|v| v.as_str()) == Some("SuspectedNoop");

        // Step 9: LEARN (Feedback & Durable Knowledge Update)
        {
            let mut kstore = self.knowledge_store.lock().unwrap();
            kstore.set_epoch(after_obs.layout_epoch);
            if success && !is_noop {
                kstore.record_outcome(&target_key, &after_obs.app_name, true, after_obs.layout_epoch);
                let step_desc = format!("{}:{}", contract.action_name, target_key);
                if let Ok(mut steps) = self.session_steps.lock() {
                    if auto_focused {
                        steps.push(format!("click:{target_key}"));
                    }
                    if modal_intercepted {
                        steps.push("key:Escape".into());
                    }
                    steps.push(step_desc);
                    if steps.len() > 32 {
                        steps.remove(0);
                    }
                    if auto_focused || modal_intercepted {
                        let proc_id = format!("proc_{}_{}", after_obs.app_name.to_ascii_lowercase(), target_key);
                        let proc_steps = if auto_focused {
                            vec![format!("click:{target_key}"), format!("{}:{target_key}", contract.action_name)]
                        } else {
                            vec!["key:Escape".into(), format!("{}:{target_key}", initial_contract.action_name)]
                        };
                        kstore.register_procedure(LearnedWorkflowProcedure::new(
                            proc_id,
                            &after_obs.app_name,
                            goal,
                            proc_steps,
                            after_obs.layout_epoch,
                        ));
                    }
                }
                let mut fc = self.failure_counts.lock().unwrap();
                fc.remove(&target_key);
            } else {
                kstore.record_outcome(&target_key, &after_obs.app_name, false, after_obs.layout_epoch);
                let mut fc = self.failure_counts.lock().unwrap();
                let c = fc.entry(target_key.clone()).or_insert(0);
                *c += 1;
            }
        }

        // Environmental state reconciliation check
        let reconciliation = {
            let kstore = self.knowledge_store.lock().unwrap();
            let ckpt = *self.last_checkpoint_epoch.lock().unwrap();
            let hash_drift = before_obs.frame_hash != after_obs.frame_hash && !success;
            kstore.reconcile_state(ckpt, after_obs.layout_epoch, hash_drift)
        };

        // Failure cause diagnosis if needed
        let failure_diagnosis = if !success || is_noop {
            let resolved_role = contract
                .target_selector
                .as_ref()
                .and_then(|tid| before_obs.find_element_by_id(tid))
                .map(|el| if !el.enabled { "disabled" } else { el.role.as_str() })
                .unwrap_or("generic");
            let (cause, escalation) = diagnose_failure_cause(
                &contract.action_name,
                &contract.delivery_mode,
                resolved_role,
                before_obs.active_focus_id.is_some(),
                after_obs.active_focus_id.is_some(),
                before_obs.frame_hash != after_obs.frame_hash,
                before_obs.modal_detected,
            );
            Some(json!({ "rootCause": cause, "escalationSuggestion": escalation }))
        } else {
            None
        };

        Ok(json!({
            "goal": goal,
            "success": success && !is_noop,
            "pipeline": "OBSERVE_ORIENT_INTERPRET_REASON_PLAN_ACT_VERIFY_LEARN",
            "executedContract": contract,
            "executionReceipt": receipt,
            "environmentalDeltas": deltas,
            "interpretation": interpretation,
            "swarmConsultation": swarm_response,
            "failureDiagnosis": failure_diagnosis,
            "reconciliation": reconciliation,
            "beforeEpoch": before_obs.layout_epoch,
            "afterEpoch": after_obs.layout_epoch
        }))
    }
}

// ============================================================================
// CONTRACTUAL SOVEREIGNTY IMPLEMENTATIONS
// ============================================================================

impl IEnvironmentalObserver for ComputerUseOrchestrator {
    fn observe_environment(&self) -> EnvironmentalObservation {
        self.observe()
    }

    fn detect_modal_trap(&self, obs: &EnvironmentalObservation) -> Option<String> {
        if obs.modal_detected {
            obs.modal_container_id.clone()
        } else {
            None
        }
    }
}

impl ISwarmIntelligenceConsultant for ComputerUseOrchestrator {
    fn consult(&self, req: &SwarmConsultationRequest) -> SwarmConsultationResponse {
        let mut insights = Vec::new();

        // 0. Query Durable Behavioral Knowledge Store for patterns and multi-step procedures
        let (learned_pattern, learned_procedure) = if let Ok(kstore) = self.knowledge_store.lock() {
            (
                kstore.lookup_pattern(&req.target_app, &req.task_objective),
                kstore.lookup_procedure(&req.target_app, &req.task_objective),
            )
        } else {
            (None, None)
        };

        let mut historical_conflict_detected = false;
        if let Some(ref pat) = learned_pattern {
            // Check for conflict between historical pattern and live environmental facts
            let modal_conflict = req.modal_blocking && pat.recommended_modality != "key";
            let target_missing_conflict = !req.candidate_target_ids.is_empty()
                && pat.app_pattern != "*"
                && !req.candidate_target_ids.iter().any(|id| id == &pat.pattern_id || id == &pat.ui_trigger || req.task_objective.contains(id));
            let prior_failure_conflict = req.last_failure_reason.is_some()
                && req.last_failed_action.as_deref() == Some(pat.recommended_modality.as_str());

            historical_conflict_detected = modal_conflict || target_missing_conflict || prior_failure_conflict;

            insights.push(SpecialistInsight::new(
                "HistoricalKnowledge",
                format!(
                    "Retrieved proven pattern '{}' (confidence: {:.2}, successes: {}, modality: '{}')",
                    pat.pattern_id, pat.confidence, pat.success_count, pat.recommended_modality
                ),
                pat.confidence,
                "BehavioralKnowledgeStore",
            ));

            if historical_conflict_detected {
                insights.push(SpecialistInsight::new(
                    "ConflictResolver",
                    "Live environmental state contradicts historical pattern; prioritizing live observation and recovery authority over historical assumption",
                    0.96,
                    "KnowledgeAuthorityGate",
                ));
            }
        }

        // 1. VisualPerceiver Specialist Insight
        insights.push(SpecialistInsight::new(
            "VisualPerceiver",
            format!("Screen has {} interactive targets; primary application is '{}'", req.candidate_target_ids.len(), req.target_app),
            0.92,
            "SyntheticVisionModel",
        ));

        // 2. StructuralAnalyst Specialist Insight
        if req.modal_blocking {
            insights.push(SpecialistInsight::new(
                "StructuralAnalyst",
                "Active modal layer traps keyboard focus; backdrop blocks background click-through",
                0.95,
                "SpiderInterfaceTree",
            ));
        } else {
            insights.push(SpecialistInsight::new(
                "StructuralAnalyst",
                "Hierarchical navigation bar and content viewport aligned without structural occlusion",
                0.88,
                "SpiderInterfaceTree",
            ));
        }

        // 3. WorkflowNavigator Specialist Insight (with conflict resolution authority)
        let rec_action = if req.modal_blocking {
            "key"
        } else if let Some(ref pat) = learned_pattern {
            if !historical_conflict_detected && pat.confidence >= 0.85 {
                pat.recommended_modality.as_str()
            } else if req.task_objective.contains("type") || req.task_objective.contains("input") {
                "type"
            } else {
                "click"
            }
        } else if req.task_objective.contains("type") || req.task_objective.contains("input") {
            "type"
        } else {
            "click"
        };
        let wf_finding = if let Some(ref proc) = learned_procedure {
            format!(
                "Recommended action for objective '{}' is '{}' (workflow procedure '{}': {:?})",
                req.task_objective, rec_action, proc.procedure_id, proc.steps
            )
        } else {
            format!("Recommended action for objective '{}' is '{}'", req.task_objective, rec_action)
        };
        insights.push(SpecialistInsight::new(
            "WorkflowNavigator",
            wf_finding,
            0.85,
            "WorkflowStateMachine",
        ));

        // 4. RiskAssessor Specialist Insight
        let is_high_risk = req.task_objective.contains("delete") || req.task_objective.contains("rm ") || req.task_objective.contains("format");
        let risk_level = if is_high_risk { ConsultationRiskLevel::High } else { ConsultationRiskLevel::Low };
        insights.push(SpecialistInsight::new(
            "RiskAssessor",
            if is_high_risk { "High risk destructive action flagged; requires confirmation fence" } else { "Standard non-destructive action" },
            0.99,
            "SafetySentinel",
        ));

        // 5. RecoverySpecialist Specialist Insight
        let recovery_advice = if let Some(ref reason) = req.last_failure_reason {
            format!("Escalate delivery mode to 'foreground' and verify focus; prior error: {reason}")
        } else {
            "Preemptive fallback: Keyboard Tab navigation available if pointer delivery encounters no-op".into()
        };
        insights.push(SpecialistInsight::new(
            "RecoverySpecialist",
            recovery_advice.clone(),
            0.90,
            "AdaptiveRecoveryLadder",
        ));

        let candidate_target = if let Some(ref pat) = learned_pattern {
            req.candidate_target_ids
                .iter()
                .find(|id| id.as_str() == pat.pattern_id.as_str() || id.as_str() == pat.ui_trigger.as_str())
                .cloned()
                .or_else(|| {
                    req.candidate_target_ids.iter().find(|id| {
                        let lower_id = id.to_ascii_lowercase();
                        req.task_objective.to_ascii_lowercase().split_whitespace().any(|word| word.len() > 2 && lower_id.contains(word))
                    }).cloned()
                })
        } else {
            req.candidate_target_ids.iter().find(|id| {
                let lower_id = id.to_ascii_lowercase();
                req.task_objective.to_ascii_lowercase().split_whitespace().any(|word| word.len() > 2 && lower_id.contains(word))
            }).cloned().or_else(|| req.candidate_target_ids.first().cloned())
        };

        let mut resp = SwarmConsultationResponse::autonomous_recommendation(
            &req.consultation_id,
            rec_action,
            candidate_target,
            None,
            0.89,
        );
        resp.specialist_insights = insights;
        resp.risk_level = risk_level;
        resp.recovery_recommendation = Some(recovery_advice);
        resp.requires_human_confirmation = is_high_risk;
        resp
    }
}

impl IComputerUseOrchestrator for ComputerUseOrchestrator {
    fn execute_closed_loop(&self, goal: &str, contract: &SpatialActionContract) -> Result<Value, String> {
        self.execute_closed_loop_cycle(goal, contract)
    }
}

impl IEnvironmentalDiffer for ComputerUseOrchestrator {
    fn compute_deltas(&self, before: &[Value], after: &[Value]) -> Value {
        compute_element_deltas(before, after)
    }
}

impl ISpatialGeometry for ComputerUseOrchestrator {
    fn compute_union_area(&self, rects: &[crate::plumbing::spatial_geometry::SpatialRect]) -> f64 {
        crate::plumbing::spatial_geometry::calculate_rectangles_union_area(rects)
    }
}
