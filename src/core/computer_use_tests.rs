// [LAYER: CORE]
#[cfg(test)]
mod tests {
    use crate::core::computer_use_orchestrator::ComputerUseOrchestrator;
    use crate::core::spatial_engine::SpatialInteractionEngine;
    use crate::core::tool_registry::ToolRegistry;
    use crate::domain::computer_use::VirtualDisplayState;
    use crate::domain::types::computer_use_types::UiElement;
    use crate::domain::interfaces::{IBehavioralKnowledgeStore, IEnvironmentalObserver, ISwarmIntelligenceConsultant};
    use crate::domain::memory::MemoryStore;
    use crate::domain::types::environmental_types::{IEnvironmentalObservation, RegionKind};
    use crate::domain::types::spatial_types::SpatialActionContract;
    use crate::domain::types::swarm_intelligence_types::{ConsultationRiskLevel, SwarmConsultationRequest};
    use crate::infrastructure::behavioral_store::BehavioralKnowledgeStore;
    use crate::plumbing::environmental_diff::{compute_element_deltas, diagnose_failure_cause};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    fn setup_orchestrator() -> (ComputerUseOrchestrator, Arc<Mutex<VirtualDisplayState>>, Arc<Mutex<MemoryStore>>) {
        let mem = Arc::new(Mutex::new(MemoryStore::new()));
        let disp = Arc::new(Mutex::new(VirtualDisplayState::new()));
        let spatial = Arc::new(SpatialInteractionEngine::new(Arc::clone(&disp)));
        let mem_sink = {
            let m = Arc::clone(&mem);
            Arc::new(move |k: &str, v: &str, cat: &str| {
                if let Ok(mut lock) = m.lock() {
                    lock.save_memory(k, v, cat);
                }
            })
        };
        let kstore = Arc::new(Mutex::new(BehavioralKnowledgeStore::new(Some(mem_sink))));
        let orch = ComputerUseOrchestrator::new(Arc::clone(&disp), spatial, kstore);
        (orch, disp, mem)
    }

    #[test]
    fn test_environmental_observation_and_region_categorization() {
        let (orch, _, _) = setup_orchestrator();
        let obs = orch.observe_environment();

        assert_eq!(obs.window_id, "win-main-1");
        assert_eq!(obs.app_name, "Heav3nsCode");
        assert!(!obs.is_modal_blocking());
        assert!(!obs.elements.is_empty());
        assert!(obs.regions.iter().any(|r| r.kind == RegionKind::ContentArea));
        assert!(obs.find_element_by_id("btn-run").is_some());
    }

    #[test]
    fn test_spider_structural_analysis_and_affordances() {
        let (orch, _, _) = setup_orchestrator();
        let obs = orch.observe();
        let interpretation = orch.interpret(&obs);

        assert_eq!(interpretation["appName"], "Heav3nsCode");
        assert!(interpretation["layoutEpoch"].as_u64().unwrap() >= 1);

        let affs = interpretation["affordances"].as_array().expect("affordances array");
        assert!(!affs.is_empty());

        let run_btn = affs.iter().find(|a| a["target_id"] == "btn-run").expect("btn-run affordance");
        assert_eq!(run_btn["affordance_type"], "submit_action");
        assert!(run_btn["confidence"].as_f64().unwrap() >= 0.85);

        let cmd_inp = affs.iter().find(|a| a["target_id"] == "inp-cmd").expect("inp-cmd affordance");
        assert_eq!(cmd_inp["affordance_type"], "text_input");
    }

    #[test]
    fn test_modal_dialog_detection_and_auto_intercept() {
        let (orch, disp, _) = setup_orchestrator();

        // Add a blocking modal dialog to display
        {
            let mut d = disp.lock().unwrap();
            let _ = d.execute_action(&json!({
                "action": "register_window",
                "id": "win-modal-alert",
                "title": "Critical Confirmation Dialog",
                "app": "SystemDialog"
            }));
        }

        let obs = orch.observe();
        assert!(obs.modal_detected);
        assert!(orch.detect_modal_trap(&obs).is_some());

        // Attempting to click an element outside the modal gets auto-routed to Escape key
        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-run".into()),
            ..SpatialActionContract::default()
        };

        let result = orch.execute_closed_loop_cycle("Click run during modal", &contract).expect("execution");
        assert!(result["pipeline"].as_str().is_some());
        assert_eq!(result["executedContract"]["action_name"], "key");
        assert_eq!(result["executedContract"]["keys"], "Escape");
    }

    #[test]
    fn test_closed_loop_observe_to_learn_lifecycle_success() {
        let (orch, _, _) = setup_orchestrator();

        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-run".into()),
            delivery_mode: "foreground".into(),
            ..SpatialActionContract::default()
        };

        let res = orch.execute_closed_loop_cycle("Trigger flight verification", &contract).expect("cycle");
        assert_eq!(res["pipeline"], "OBSERVE_ORIENT_INTERPRET_REASON_PLAN_ACT_VERIFY_LEARN");
        assert!(res["success"].as_bool().unwrap());
        assert_eq!(res["executionReceipt"]["verdict"], "Confirmed");

        // Verify behavioral knowledge store recorded the success
        let kstore = orch.knowledge_store.lock().unwrap();
        let pat = kstore.lookup_pattern("Heav3nsCode", "btn-run");
        assert!(pat.is_some());
        assert!(pat.unwrap().success_count >= 1);
    }

    #[test]
    fn test_closed_loop_auto_focus_before_typing() {
        let (orch, disp, _) = setup_orchestrator();

        // Ensure inp-cmd is not focused initially
        {
            let mut d = disp.lock().unwrap();
            if let Some(w) = d.windows.iter_mut().find(|w| w.id == "win-main-1") {
                for el in &mut w.elements {
                    el.focused = false;
                }
            }
        }

        let contract = SpatialActionContract {
            action_name: "type".into(),
            target_selector: Some("inp-cmd".into()),
            payload_text: Some("cargo check".into()),
            delivery_mode: "foreground".into(),
            ..SpatialActionContract::default()
        };

        let res = orch.execute_closed_loop_cycle("Type command into unfocused input", &contract).expect("cycle");
        assert!(res["success"].as_bool().unwrap());

        // Verify inp-cmd received the value
        let d = disp.lock().unwrap();
        let el = d.windows[0].elements.iter().find(|e| e.id == "inp-cmd").unwrap();
        assert!(el.value.contains("cargo check"));
    }

    #[test]
    fn test_failure_diagnosis_and_adaptive_escalation() {
        let (cause, escalation) = diagnose_failure_cause(
            "click",
            "background",
            "button",
            false,
            false,
            false,
            false,
        );
        assert_eq!(cause, "background_event_ignored_by_window");
        assert!(escalation.contains("foreground"));

        let (type_cause, type_esc) = diagnose_failure_cause(
            "type",
            "foreground",
            "textbox",
            false,
            false,
            false,
            false,
        );
        assert_eq!(type_cause, "target_lacked_focus");
        assert!(type_esc.contains("click on target element"));
    }

    #[test]
    fn test_synthetic_swarm_consultation_specialists() {
        let (orch, _, _) = setup_orchestrator();

        let req = SwarmConsultationRequest::new("consult-1", "delete workspace", "Heav3nsCode", "Workbench");
        let resp = orch.consult(&req);

        assert_eq!(resp.consultation_id, "consult-1");
        assert!(resp.confidence >= 0.85);
        assert_eq!(resp.risk_level, ConsultationRiskLevel::High);
        assert!(resp.requires_human_confirmation);

        // Verify all 5 specialists contributed insights
        let roles: Vec<&str> = resp.specialist_insights.iter().map(|s| s.specialist_role.as_str()).collect();
        assert!(roles.contains(&"VisualPerceiver"));
        assert!(roles.contains(&"StructuralAnalyst"));
        assert!(roles.contains(&"WorkflowNavigator"));
        assert!(roles.contains(&"RiskAssessor"));
        assert!(roles.contains(&"RecoverySpecialist"));
    }

    #[test]
    fn test_durable_knowledge_store_confidence_calibration_and_memory_sync() {
        let mem = Arc::new(Mutex::new(MemoryStore::new()));
        let mem_sink = {
            let m = Arc::clone(&mem);
            Arc::new(move |k: &str, v: &str, cat: &str| {
                if let Ok(mut lock) = m.lock() {
                    lock.save_memory(k, v, cat);
                }
            })
        };
        let mut kstore = BehavioralKnowledgeStore::new(Some(mem_sink));

        let initial_pat = kstore.lookup_pattern("Heav3nsCode", "flight_verify").unwrap();
        let init_conf = initial_pat.confidence;

        // Record a success
        kstore.record_outcome("flight_verify", "Heav3nsCode", true, 2);
        let updated_pat = kstore.lookup_pattern("Heav3nsCode", "flight_verify").unwrap();
        assert!(updated_pat.confidence >= init_conf);
        assert_eq!(updated_pat.success_count, 2);

        // Verify synced to MemoryStore
        let mem_lock = mem.lock().unwrap();
        let memories = mem_lock.search_memories("flight_verify");
        assert!(!memories.is_empty());
        assert_eq!(memories[0].category, "behavioral_intelligence");
    }

    #[test]
    fn test_environmental_reconciliation_acknowledges_external_drift() {
        let (_, _, _) = setup_orchestrator();
        let kstore = BehavioralKnowledgeStore::new(None);

        // Case 1: Synchronized
        let sync_report = kstore.reconcile_state(5, 5, false);
        assert!(!sync_report.state_diverged);
        assert!(!sync_report.external_drift_detected);

        // Case 2: Diverged with external drift
        let div_report = kstore.reconcile_state(4, 5, true);
        assert!(div_report.state_diverged);
        assert!(div_report.external_drift_detected);
        assert!(!div_report.divergence_reasons.is_empty());
    }

    #[test]
    fn test_tool_registry_native_computer_use_orchestration() {
        let mem = Arc::new(Mutex::new(MemoryStore::new()));
        let registry = ToolRegistry::new(mem);

        // Test action: "observe"
        let spec = registry.get_spec("computer_use").expect("computer_use spec");
        assert_eq!(spec.name, "computer_use");

        // Test action: "consult_swarm" via orchestrator
        let req = SwarmConsultationRequest::new("consult-t", "navigate docs", "Heav3nsCode", "Workbench");
        let swarm_resp = registry.computer_orchestrator.consult(&req);
        assert_eq!(swarm_resp.recommended_action, "click");
        assert_eq!(swarm_resp.risk_level, ConsultationRiskLevel::Low);

        // Test action: "orchestrate" closed loop cycle
        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-swarm".into()),
            delivery_mode: "foreground".into(),
            ..SpatialActionContract::default()
        };
        let cycle_res = registry.computer_orchestrator.execute_closed_loop_cycle("Engage swarm", &contract).expect("cycle");
        assert!(cycle_res["success"].as_bool().unwrap());
    }

    #[test]
    fn test_element_deltas_computation() {
        let before = vec![
            json!({"id": "el-1", "value": "old", "focused": true, "bounds": [0, 0, 100, 40]}),
            json!({"id": "el-2", "value": "", "focused": false, "bounds": [0, 50, 100, 40]}),
        ];
        let after = vec![
            json!({"id": "el-1", "value": "new", "focused": false, "bounds": [0, 0, 100, 40]}),
            json!({"id": "el-3", "value": "spawned", "focused": true, "bounds": [0, 100, 100, 40]}),
        ];

        let deltas = compute_element_deltas(&before, &after);
        assert_eq!(deltas["addedCount"], 1);
        assert_eq!(deltas["removedCount"], 1);
        assert_eq!(deltas["modifiedCount"], 1);
    }

    #[test]
    fn test_rectangles_union_area_exact_overlap() {
        use crate::plumbing::spatial_geometry::{calculate_rectangles_union_area, SpatialRect};

        // 3 identical rectangles of area 100
        let identical = vec![
            SpatialRect::new(0.0, 0.0, 10.0, 10.0),
            SpatialRect::new(0.0, 0.0, 10.0, 10.0),
            SpatialRect::new(0.0, 0.0, 10.0, 10.0),
        ];
        let union_identical = calculate_rectangles_union_area(&identical);
        assert!((union_identical - 100.0).abs() < 1e-6, "3 identical 10x10 rects must have union area 100, got {union_identical}");

        // Staggered 10x10 rectangles overlapping by 5x10: [0..10], [5..15], [10..20]
        let staggered = vec![
            SpatialRect::new(0.0, 0.0, 10.0, 10.0),
            SpatialRect::new(5.0, 0.0, 10.0, 10.0),
            SpatialRect::new(10.0, 0.0, 10.0, 10.0),
        ];
        let union_staggered = calculate_rectangles_union_area(&staggered);
        assert!((union_staggered - 200.0).abs() < 1e-6, "3 staggered rects spanning 0..20 must have area 200, got {union_staggered}");
    }

    #[test]
    fn test_occlusion_ignores_non_blocking_and_transparent_overlays() {
        let (orch, disp, _) = setup_orchestrator();

        // Add a non-blocking full-screen overlay div with role 'transparent_overlay' at z_order 999
        {
            let mut d = disp.lock().unwrap();
            if let Some(w) = d.windows.iter_mut().find(|w| w.id == "win-main-1") {
                w.elements.push(crate::domain::computer_use::UiElement {
                    id: "overlay-mask".into(),
                    som_id: 99,
                    role: "transparent_overlay".into(),
                    label: "Adversarial Transparent Mask".into(),
                    value: "".into(),
                    bounds: [0, 0, 1440, 900],
                    enabled: false,
                    focused: false,
                });
            }
        }

        // Target 'btn-run' is under the full-screen overlay
        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-run".into()),
            delivery_mode: "foreground".into(),
            ..SpatialActionContract::default()
        };

        let res = orch.execute_closed_loop_cycle("Click run despite non-blocking overlay", &contract).expect("cycle");
        assert!(res["success"].as_bool().unwrap(), "Interaction must succeed through non-blocking overlays");
    }

    #[test]
    fn test_keyboard_action_grounding_without_target() {
        let (orch, _, _) = setup_orchestrator();

        let key_contract = SpatialActionContract {
            action_name: "key".into(),
            keys: Some("Escape".into()),
            target_selector: None,
            ..SpatialActionContract::default()
        };

        let res = orch.execute_closed_loop_cycle("Press global Escape key", &key_contract).expect("cycle");
        assert!(res["success"].as_bool().unwrap(), "Global key event without target selector must ground to window");
    }

    #[test]
    fn test_modal_internal_element_interaction_not_intercepted() {
        let (orch, disp, _) = setup_orchestrator();

        // Register a modal window with an OK button inside it
        {
            let mut d = disp.lock().unwrap();
            let _ = d.execute_action(&json!({
                "action": "register_window",
                "id": "win-modal-alert",
                "title": "Action Confirmation Dialog",
                "app": "SystemDialog"
            }));
            if let Some(w) = d.windows.iter_mut().find(|w| w.id == "win-modal-alert") {
                w.bounds = [200, 150, 600, 400];
                w.elements.push(crate::domain::computer_use::UiElement {
                    id: "btn-confirm".into(),
                    som_id: 10,
                    role: "button".into(),
                    label: "Confirm Action".into(),
                    value: "".into(),
                    bounds: [300, 350, 120, 40],
                    enabled: true,
                    focused: false,
                });
            }
        }

        let obs = orch.observe();
        assert!(obs.modal_detected);
        assert!(obs.is_element_in_modal("btn-confirm"), "btn-confirm must be identified as inside modal dialog");

        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-confirm".into()),
            delivery_mode: "foreground".into(),
            ..SpatialActionContract::default()
        };

        let res = orch.execute_closed_loop_cycle("Confirm modal action", &contract).expect("cycle");
        assert_eq!(res["executedContract"]["action_name"], "click", "Click inside modal must NOT be rewritten to Escape");
        assert_eq!(res["executedContract"]["target_selector"].as_str(), Some("btn-confirm"));
    }

    #[test]
    fn test_swarm_consultation_incorporates_learned_behavioral_knowledge() {
        let (orch, _, _) = setup_orchestrator();

        // Consult for an objective that matches seeded behavioral conventions ("search_bar")
        let req = SwarmConsultationRequest::new("consult-search", "search_bar", "Workbench", "Main");
        let resp = orch.consult(&req);

        let roles: Vec<&str> = resp.specialist_insights.iter().map(|s| s.specialist_role.as_str()).collect();
        assert!(roles.contains(&"HistoricalKnowledge"), "HistoricalKnowledge specialist must contribute learned conventions");
    }

    #[test]
    fn test_loop_breaker_aborts_excessive_repeated_failures() {
        let (orch, _, _) = setup_orchestrator();

        // Seed 3 consecutive failures on this target
        {
            let mut fc = orch.failure_counts.lock().unwrap();
            fc.insert("btn-stuck".into(), 3);
        }

        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-stuck".into()),
            ..SpatialActionContract::default()
        };

        let res = orch.execute_closed_loop_cycle("Execute stuck action", &contract);
        assert!(res.is_err(), "Loop breaker must halt repeated failures");
        assert!(res.unwrap_err().contains("Loop breaker engaged"));
    }

    #[test]
    fn test_set_of_marks_tokens_hash_and_som_prefix() {
        let (orch, _, _) = setup_orchestrator();

        // Target with #1 and som:1
        let contract_hash = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("#1".into()),
            delivery_mode: "foreground".into(),
            ..SpatialActionContract::default()
        };
        let res_hash = orch.execute_closed_loop_cycle("Click via #1 badge", &contract_hash).expect("cycle");
        assert!(res_hash["success"].as_bool().unwrap());

        let contract_som = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("som:1".into()),
            delivery_mode: "foreground".into(),
            ..SpatialActionContract::default()
        };
        let res_som = orch.execute_closed_loop_cycle("Click via som:1 badge", &contract_som).expect("cycle");
        assert!(res_som["success"].as_bool().unwrap());
    }

    #[test]
    fn test_background_element_under_modal_window_is_not_falsely_classified_as_inside_modal() {
        let (orch, disp, _) = setup_orchestrator();

        // Add a button in the main window at (400, 300) and a separate modal dialog window covering (200..800, 150..550)
        {
            let mut d = disp.lock().unwrap();
            if let Some(main_win) = d.windows.first_mut() {
                main_win.elements.push(crate::domain::computer_use::UiElement {
                    id: "btn-bg-under-modal".into(),
                    som_id: 9,
                    role: "button".into(),
                    label: "Background Action".into(),
                    value: "".into(),
                    bounds: [400, 300, 120, 40],
                    enabled: true,
                    focused: false,
                });
            }
            let _ = d.execute_action(&json!({
                "action": "register_window",
                "id": "win-modal-overlay",
                "title": "Security Alert Dialog",
                "app": "SystemDialog"
            }));
            if let Some(modal_win) = d.windows.iter_mut().find(|w| w.id == "win-modal-overlay") {
                modal_win.bounds = [200, 150, 600, 400];
                modal_win.elements.push(crate::domain::computer_use::UiElement {
                    id: "btn-modal-ok".into(),
                    som_id: 11,
                    role: "button".into(),
                    label: "OK".into(),
                    value: "".into(),
                    bounds: [420, 320, 80, 36],
                    enabled: true,
                    focused: false,
                });
            }
        }

        let obs = orch.observe();
        assert!(obs.modal_detected);
        assert!(obs.is_element_in_modal("btn-modal-ok"), "Modal window button must be inside modal");
        assert!(
            !obs.is_element_in_modal("btn-bg-under-modal"),
            "Main window button geometrically underneath modal window must NOT be classified as inside modal"
        );

        // Attempting to click the occluded background button must auto-intercept with Escape
        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-bg-under-modal".into()),
            ..SpatialActionContract::default()
        };
        let res = orch.execute_closed_loop_cycle("Click occluded background button", &contract).expect("cycle");
        assert_eq!(res["executedContract"]["action_name"], "key", "Must auto-intercept with Escape");
        assert_eq!(res["executedContract"]["keys"].as_str(), Some("Escape"));
    }

    #[test]
    fn test_swarm_conflict_resolution_and_consultation_memoization_cache() {
        let (orch, _, _) = setup_orchestrator();

        // Record a custom pattern for AppX recommending "click" on "btn-deploy"
        {
            let mut ks = orch.knowledge_store.lock().unwrap();
            ks.record_outcome("btn-deploy", "AppX", true, 1);
            ks.record_outcome("btn-deploy", "AppX", true, 2);
            let pat = ks.lookup_pattern("AppX", "btn-deploy").expect("pattern");
            assert_eq!(pat.recommended_modality, "click", "Dynamically learned clickable pattern must default to click, never background");
        }

        // Now consult when a modal dialog is blocking (creating a conflict between historical 'click' and live modal 'key')
        let mut req = SwarmConsultationRequest::new("consult-conflict", "btn-deploy", "AppX", "Main");
        req.modal_blocking = true;
        req.candidate_target_ids = vec!["btn-deploy".into(), "btn-modal-close".into()];

        let resp = orch.consult(&req);
        assert_eq!(resp.recommended_action, "key", "Live modal state must override historical click pattern");
        let roles: Vec<&str> = resp.specialist_insights.iter().map(|s| s.specialist_role.as_str()).collect();
        assert!(roles.contains(&"ConflictResolver"), "ConflictResolver insight must be emitted when live state contradicts history");

        // Verify memoization cache in consult_swarm_if_needed
        let obs = orch.observe();
        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: None,
            ..SpatialActionContract::default()
        };
        let c1 = orch.consult_swarm_if_needed(&obs, &contract, Some("test_uncertainty")).expect("c1");
        let c2 = orch.consult_swarm_if_needed(&obs, &contract, Some("test_uncertainty")).expect("c2");
        assert_eq!(c1.consultation_id, c2.consultation_id, "Identical environmental state must hit consultation memoization cache");
    }

    #[test]
    fn test_spider_extended_relationships_and_workflow_procedure_synthesis() {
        let elements = vec![
            json!({"id": "lbl-user", "role": "label", "label": "Username", "bounds": [20, 100, 80, 24]}),
            json!({"id": "inp-user", "role": "textbox", "label": "Enter username", "bounds": [110, 100, 200, 28]}),
            json!({"id": "btn-submit", "role": "button", "label": "Submit", "bounds": [320, 100, 90, 28]}),
            json!({"id": "dlg-confirm", "role": "dialog", "label": "Confirm", "bounds": [100, 80, 400, 300]}),
            json!({"id": "btn-close", "role": "button", "label": "Close", "bounds": [450, 90, 40, 24]}),
        ];
        let rels = crate::plumbing::environmental_diff::analyze_spider_structural_relationships(&elements);
        assert!(rels.iter().any(|r| r["relation"] == "label_for" && r["sourceId"] == "lbl-user" && r["targetId"] == "inp-user"));
        assert!(rels.iter().any(|r| r["relation"] == "form_submission_group" && r["sourceId"] == "inp-user" && r["targetId"] == "btn-submit"));
        assert!(rels.iter().any(|r| r["relation"] == "modal_dismiss_control" && r["sourceId"] == "btn-close" && r["targetId"] == "dlg-confirm"));

        // Verify compound auto-focus + type synthesizes a LearnedWorkflowProcedure
        let (orch, disp, _) = setup_orchestrator();
        {
            let mut d = disp.lock().unwrap();
            if let Some(w) = d.windows.first_mut() {
                for e in &mut w.elements {
                    e.focused = false;
                }
            }
        }
        let type_contract = SpatialActionContract {
            action_name: "type".into(),
            target_selector: Some("inp-cmd".into()),
            payload_text: Some("workflow synthesis test".into()),
            delivery_mode: "foreground".into(),
            ..SpatialActionContract::default()
        };
        let res = orch.execute_closed_loop_cycle("Type workflow prompt", &type_contract).expect("cycle");
        assert!(res["success"].as_bool().unwrap());

        let ks = orch.knowledge_store.lock().unwrap();
        let proc = ks.lookup_procedure("Heav3nsCode", "Type workflow prompt").expect("synthesized procedure");
        assert_eq!(proc.steps, vec!["click:inp-cmd".to_string(), "type:inp-cmd".to_string()]);
    }

    #[test]
    fn test_disabled_element_guard_checkbox_toggle_and_extended_ledger_dsl() {
        let (orch, disp, _) = setup_orchestrator();
        {
            let mut d = disp.lock().unwrap();
            if let Some(w) = d.windows.first_mut() {
                w.elements.push(UiElement {
                    id: "btn-disabled".into(),
                    som_id: 20,
                    role: "button".into(),
                    label: "Deploy Production".into(),
                    value: "".into(),
                    bounds: [40, 200, 180, 40],
                    enabled: false,
                    focused: false,
                });
                w.elements.push(UiElement {
                    id: "chk-strict".into(),
                    som_id: 21,
                    role: "checkbox".into(),
                    label: "Strict Mode".into(),
                    value: "false".into(),
                    bounds: [260, 200, 140, 32],
                    enabled: true,
                    focused: false,
                });
            }
        }

        // 1. Interpret classifies disabled element as disabled_control even without "disabled" in label
        let obs = orch.observe();
        let interp = orch.interpret(&obs);
        let affs = interp["affordances"].as_array().unwrap();
        let dis_aff = affs.iter().find(|a| a["target_id"] == "btn-disabled").unwrap();
        assert_eq!(dis_aff["affordance_type"], "disabled_control");

        // 2. Clicking disabled element in VirtualDisplayState returns target_disabled verdict
        {
            let mut d = disp.lock().unwrap();
            let click_dis = d.execute_action(&json!({"action": "click", "element_id": "btn-disabled"})).unwrap();
            assert_eq!(click_dis["targetDisabled"], true);
            assert_eq!(click_dis["verdict"]["effect"], "target_disabled");

            // 3. Clicking checkbox toggles value false -> true, and pressing Space toggles true -> false
            let click_chk = d.execute_action(&json!({"action": "click", "element_id": "chk-strict", "delivery_mode": "foreground"})).unwrap();
            assert_eq!(click_chk["resolvedElementId"], "chk-strict");
            let chk_val = d.windows[0].elements.iter().find(|e| e.id == "chk-strict").unwrap().value.clone();
            assert_eq!(chk_val, "true");

            let _ = d.execute_action(&json!({"action": "key", "keys": "Space"})).unwrap();
            let chk_val_after_space = d.windows[0].elements.iter().find(|e| e.id == "chk-strict").unwrap().value.clone();
            assert_eq!(chk_val_after_space, "false");

            // 4. Extended search_dsl queries by element: and mode:
            let dsl = d.search_dsl("action:click element:chk-strict mode:foreground");
            assert_eq!(dsl["matchCount"], 1);
        }
    }
}
