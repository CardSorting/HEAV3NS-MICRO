// [LAYER: CORE]
#[cfg(test)]
mod tests {
    use crate::core::spatial_engine::SpatialInteractionEngine;
    use crate::core::tool_registry::ToolRegistry;
    use crate::domain::computer_use::VirtualDisplayState;
    use crate::domain::memory::MemoryStore;
    use crate::domain::spatial_mediator::SpatialActionMediator;
    use crate::domain::types::computer_use_types::UiElement;
    use crate::domain::types::runtime::ExecutionAuthorityLevel;
    use crate::domain::types::spatial_types::{AssuranceVerdict, SpatialActionContract, SpatialError, SpatialScene, SpatialTarget, TargetProvenance};
    use crate::domain::types::tool_schedule::ScheduledToolCall;
    use crate::plumbing::spatial_geometry::{normalize_coordinate_value, validate_trajectory, SpatialPoint, SpatialRect, ViewportTransform};
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    // ========================================================================
    // 1. GEOMETRY & COORDINATE TRANSFORMATION TESTS
    // ========================================================================

    #[test]
    fn test_spatial_rect_primitives_and_safe_center() {
        let r1 = SpatialRect::new(100.0, 100.0, 200.0, 50.0);
        assert!(r1.is_valid());
        assert_eq!(r1.area(), 10000.0);
        assert_eq!(r1.center(), SpatialPoint::new(200.0, 125.0));

        // Safe center with 15% inset margin
        let safe = r1.safe_center(0.15);
        assert_eq!(safe, SpatialPoint::new(200.0, 125.0));
        assert!(r1.contains_point(&safe));

        // Edge point vs safe interior point
        let edge_pt = SpatialPoint::new(100.0, 100.0);
        assert!(r1.contains_point(&edge_pt));
        let outside_pt = SpatialPoint::new(99.0, 100.0);
        assert!(!r1.contains_point(&outside_pt));

        // Invalid degenerate rect
        let invalid = SpatialRect::new(10.0, 10.0, 0.0, -5.0);
        assert!(!invalid.is_valid());
        assert_eq!(invalid.area(), 0.0);
    }

    #[test]
    fn test_spatial_intersection_iou_and_occlusion_ratio() {
        let base = SpatialRect::new(0.0, 0.0, 100.0, 100.0);
        let occluder_half = SpatialRect::new(50.0, 0.0, 100.0, 100.0);
        assert!(base.intersects(&occluder_half));

        let inter = base.intersection(&occluder_half).unwrap();
        assert_eq!(inter, SpatialRect::new(50.0, 0.0, 50.0, 100.0));
        assert_eq!(inter.area(), 5000.0);

        // Occlusion ratio should be exactly 50%
        let occ = base.occlusion_ratio(&occluder_half);
        assert!((occ - 0.50).abs() < 1e-6);

        // IoU: intersection=5000, union=15000 -> 1/3
        let iou = base.intersection_over_union(&occluder_half);
        assert!((iou - (1.0 / 3.0)).abs() < 1e-6);

        // Fully occluding rect
        let full_occluder = SpatialRect::new(-10.0, -10.0, 200.0, 200.0);
        assert_eq!(base.occlusion_ratio(&full_occluder), 1.0);

        // Non-overlapping rect
        let separate = SpatialRect::new(200.0, 200.0, 50.0, 50.0);
        assert!(!base.intersects(&separate));
        assert_eq!(base.occlusion_ratio(&separate), 0.0);
    }

    #[test]
    fn test_directional_spatial_relationships() {
        let left_btn = SpatialRect::new(50.0, 100.0, 100.0, 40.0);
        let right_btn = SpatialRect::new(200.0, 100.0, 100.0, 40.0);
        let below_box = SpatialRect::new(50.0, 200.0, 250.0, 80.0);

        assert!(right_btn.is_to_right_of(&left_btn));
        assert!(left_btn.is_to_left_of(&right_btn));
        assert!(below_box.is_below(&left_btn));
        assert!(left_btn.is_above(&below_box));
    }

    #[test]
    fn test_viewport_scroll_and_dpr_transformations() {
        let vp = ViewportTransform::new(1000.0, 800.0)
            .with_scroll(100.0, 200.0)
            .with_dpr(2.0);

        let elem_doc = SpatialRect::new(150.0, 250.0, 200.0, 50.0);

        // In viewport space: (150-100, 250-200) = (50, 50)
        let vp_rect = vp.element_to_viewport_rect(&elem_doc);
        assert_eq!(vp_rect.x, 50.0);
        assert_eq!(vp_rect.y, 50.0);
        assert_eq!(vp_rect.width, 200.0);
        assert_eq!(vp_rect.height, 50.0);

        // Screen space with window offset (10, 20) and 2x DPR:
        // ((50+10)*2, (50+20)*2) = (120, 140), width = 400, height = 100
        let screen_rect = vp.viewport_to_screen_rect(&vp_rect, 10.0, 20.0);
        assert_eq!(screen_rect.x, 120.0);
        assert_eq!(screen_rect.y, 140.0);
        assert_eq!(screen_rect.width, 400.0);
        assert_eq!(screen_rect.height, 100.0);

        // Visibility ratio
        assert_eq!(vp.visibility_ratio(&elem_doc), 1.0);

        // Element far out of view (y = 5000)
        let offscreen_elem = SpatialRect::new(100.0, 5000.0, 200.0, 50.0);
        assert_eq!(vp.visibility_ratio(&offscreen_elem), 0.0);
        let (_scroll_dx, scroll_dy) = vp.compute_centering_scroll_delta(&offscreen_elem);
        assert!((scroll_dy - (5025.0 - 400.0 - 200.0)).abs() < 1e-4);
    }

    #[test]
    fn test_universal_coordinate_normalization() {
        let dim = 1440.0;
        // 1. Raw integer/float
        assert_eq!(normalize_coordinate_value(Some(&json!(500)), dim, 0.0, false), 500.0);
        // 2. Normalized unit [0.0, 1.0]
        assert_eq!(normalize_coordinate_value(Some(&json!(0.5)), dim, 0.0, false), 720.0);
        // 3. Percentage string
        assert_eq!(normalize_coordinate_value(Some(&json!("25%")), dim, 0.0, false), 360.0);
        // 4. Pixel suffix string
        assert_eq!(normalize_coordinate_value(Some(&json!("400px")), dim, 0.0, false), 400.0);
        // 5. Normalized 1000
        assert_eq!(normalize_coordinate_value(Some(&json!(500.0)), dim, 0.0, true), 720.0);
    }

    #[test]
    fn test_trajectory_validation_and_exclusion_zones() {
        let screen = SpatialRect::new(0.0, 0.0, 1440.0, 900.0);
        let modal_exclusion = SpatialRect::new(500.0, 200.0, 440.0, 500.0);

        // Safe trajectory around the exclusion
        let p_start = SpatialPoint::new(100.0, 100.0);
        let p_safe = SpatialPoint::new(300.0, 100.0);
        assert!(validate_trajectory(&p_start, &p_safe, &screen, &[modal_exclusion], 10).is_ok());

        // Dangerous trajectory cutting straight through modal exclusion
        let p_end_penetrating = SpatialPoint::new(800.0, 400.0);
        let res = validate_trajectory(&p_start, &p_end_penetrating, &screen, &[modal_exclusion], 20);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("forbidden exclusion zone"));

        // Out-of-bounds start
        let p_oob = SpatialPoint::new(-10.0, 50.0);
        assert!(validate_trajectory(&p_oob, &p_safe, &screen, &[], 5).is_err());
    }

    // ========================================================================
    // 2. WRAPPER A (SPATIAL GROUNDING) & WRAPPER B (EXECUTION ASSURANCE) TESTS
    // ========================================================================

    #[test]
    fn test_wrapper_a_grounding_success_and_confidence() {
        let mut med = SpatialActionMediator::new(1440, 900, "win-1");
        let mut scene = SpatialScene::new(1440, 900, "win-1");
        let mut btn = SpatialTarget::new("btn-submit", Some(1), "button", "Submit Order", [100, 100, 160, 40]);
        btn.confidence = 0.95;
        scene.targets.push(btn);
        med.update_scene(scene);

        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-submit".into()),
            required_confidence: 0.80,
            ..Default::default()
        };

        let result = med.ground_action(&contract);
        assert!(result.success);
        let grounded = result.grounded.unwrap();
        assert_eq!(grounded.target.id, "btn-submit");
        assert_eq!(grounded.safe_point, [180, 120]); // safe center with inset
        assert_eq!(grounded.occlusion_ratio, 0.0);
        assert!(!grounded.requires_scroll);
    }

    #[test]
    fn test_wrapper_a_rejects_stale_layout_epoch() {
        let mut med = SpatialActionMediator::new(1440, 900, "win-1");
        med.invalidate_layout(); // epoch = 2
        med.invalidate_layout(); // epoch = 3

        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-submit".into()),
            expected_epoch: Some(1), // Stale epoch expectation!
            ..Default::default()
        };

        let result = med.ground_action(&contract);
        assert!(!result.success);
        match result.error.unwrap() {
            SpatialError::StaleTargetEpoch { expected, current } => {
                assert_eq!(expected, 1);
                assert_eq!(current, 3);
            }
            other => panic!("Expected StaleTargetEpoch, got: {:?}", other),
        }
    }

    #[test]
    fn test_wrapper_a_detects_occluded_target() {
        let mut med = SpatialActionMediator::new(1440, 900, "win-1");
        let mut scene = SpatialScene::new(1440, 900, "win-1");

        // Target under a modal dialog
        let mut base_btn = SpatialTarget::new("btn-under", Some(1), "button", "Save", [100, 100, 200, 50]);
        base_btn.z_order = 1;

        // Modal overlay covering 80% of button
        let mut modal = SpatialTarget::new("modal-dialog", Some(2), "dialog", "Alert", [100, 100, 180, 50]);
        modal.z_order = 2; // Higher z-order

        scene.targets.push(base_btn);
        scene.targets.push(modal);
        med.update_scene(scene);

        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-under".into()),
            max_occlusion: 0.50, // max 50% occlusion tolerated
            ..Default::default()
        };

        let result = med.ground_action(&contract);
        assert!(!result.success);
        match result.error.unwrap() {
            SpatialError::TargetOccluded { target_id, occlusion_ratio, occluded_by } => {
                assert_eq!(target_id, "btn-under");
                assert_eq!(occluded_by, "modal-dialog");
                assert!(occlusion_ratio >= 0.80);
            }
            other => panic!("Expected TargetOccluded, got: {:?}", other),
        }
    }

    #[test]
    fn test_wrapper_b_precondition_role_and_containment_enforcement() {
        let med = SpatialActionMediator::new(1440, 900, "win-1");

        // 1. Role mismatch: Attempting to type into a non-input element (e.g. static link)
        let link_target = SpatialTarget::new("lnk-home", Some(1), "link", "Home", [50, 50, 100, 30]);
        let grounded = crate::domain::spatial_action::GroundedSpatialTarget {
            target: link_target,
            safe_point: [100, 65],
            screen_bounds: [50, 50, 100, 30],
            visibility_ratio: 1.0,
            occlusion_ratio: 0.0,
            occluding_target_id: None,
            confidence: 1.0,
            requires_scroll: false,
            scroll_delta: (0.0, 0.0),
        };

        let type_contract = SpatialActionContract {
            action_name: "type".into(),
            payload_text: Some("malicious string".into()),
            ..Default::default()
        };

        let pre_err = med.assure_pre_execution(&grounded, &type_contract);
        assert!(pre_err.is_err());
        match pre_err.unwrap_err() {
            SpatialError::ActionNotPermittedForRole { role, action } => {
                assert_eq!(role, "link");
                assert_eq!(action, "type");
            }
            other => panic!("Expected ActionNotPermittedForRole, got: {:?}", other),
        }

        // 2. Disabled target rejection
        let mut disabled_btn = SpatialTarget::new("btn-dis", Some(2), "button", "Submit", [50, 50, 100, 30]);
        disabled_btn.enabled = false;
        let mut grounded_disabled = grounded.clone();
        grounded_disabled.target = disabled_btn;
        let click_contract = SpatialActionContract {
            action_name: "click".into(),
            ..Default::default()
        };
        assert!(matches!(med.assure_pre_execution(&grounded_disabled, &click_contract), Err(SpatialError::TargetDisabled(_))));
    }

    #[test]
    fn test_wrapper_b_observable_postcondition_verification() {
        let mut med = SpatialActionMediator::new(1440, 900, "win-1");

        let mut before_scene = SpatialScene::new(1440, 900, "win-1");
        let btn_before = SpatialTarget::new("btn-play", Some(1), "button", "Play", [100, 100, 100, 40]);
        before_scene.targets.push(btn_before);

        let grounded = crate::domain::spatial_action::GroundedSpatialTarget {
            target: before_scene.targets[0].clone(),
            safe_point: [150, 120],
            screen_bounds: [100, 100, 100, 40],
            visibility_ratio: 1.0,
            occlusion_ratio: 0.0,
            occluding_target_id: None,
            confidence: 1.0,
            requires_scroll: false,
            scroll_delta: (0.0, 0.0),
        };

        // Scenario A: Confirmed state change (focus updated)
        let mut after_confirmed = before_scene.clone();
        after_confirmed.targets[0].focused = true;
        let receipt_confirmed = med.verify_post_execution("click", Some(&grounded), &before_scene, &after_confirmed, 2);
        assert_eq!(receipt_confirmed.verdict, AssuranceVerdict::Confirmed);
        assert!(receipt_confirmed.postconditions_met);

        // Scenario B: Suspected no-op (no focus update, no frame change)
        let after_noop = before_scene.clone();
        let receipt_noop = med.verify_post_execution("click", Some(&grounded), &before_scene, &after_noop, 2);
        assert_eq!(receipt_noop.verdict, AssuranceVerdict::SuspectedNoop);
        assert!(!receipt_noop.postconditions_met);
        assert!(receipt_noop.recovery_suggestion.is_some());
    }

    // ========================================================================
    // 3. CROSS-MODAL RECONCILIATION & DIRECTIONAL NAVIGATION TESTS
    // ========================================================================

    #[test]
    fn test_cross_modal_dom_and_visual_reconciliation() {
        let disp = Arc::new(Mutex::new(VirtualDisplayState::new()));
        let engine = SpatialInteractionEngine::new(disp);

        let dom_snapshot = vec![
            json!({ "somId": 1, "id": "btn-run", "role": "button", "label": "Run Flight Verification" }),
            json!({ "somId": 2, "id": "inp-cmd", "role": "textbox", "label": "Command Input" }),
            json!({ "somId": 99, "id": "new-dom-link", "role": "link", "label": "New Ingested Link" }),
        ];

        let reconciled_scene = engine.reconcile_dom_and_visual(&dom_snapshot);
        assert_eq!(reconciled_scene.targets.len(), 5); // 4 initial window elements + 1 new ingested DOM element

        // Matched visual target receives DomTree provenance
        let btn_run = reconciled_scene.find_by_id("btn-run").unwrap();
        assert_eq!(btn_run.provenance, TargetProvenance::DomTree);
        assert_eq!(btn_run.confidence, 1.0);

        // New DOM element synthesized properly
        let new_link = reconciled_scene.find_by_id("new-dom-link").unwrap();
        assert_eq!(new_link.role, "link");
        assert_eq!(new_link.provenance, TargetProvenance::DomTree);
        assert!(new_link.confidence >= 0.80);
    }

    #[test]
    fn test_spatial_directional_queries() {
        let disp = Arc::new(Mutex::new(VirtualDisplayState::new()));
        let engine = SpatialInteractionEngine::new(disp);

        // Default display layout:
        // btn-run: [40, 80, 200, 40]
        // inp-cmd: [260, 80, 480, 40] -> to the right of btn-run
        // lnk-docs: [760, 80, 180, 40] -> to the right of inp-cmd
        let right_of_btn = engine.query_directional("btn-run", "right_of").unwrap();
        assert_eq!(right_of_btn.id, "inp-cmd");

        let left_of_docs = engine.query_directional("lnk-docs", "left_of").unwrap();
        // Closest element to the left of lnk-docs is inp-cmd
        assert_eq!(left_of_docs.id, "inp-cmd");
    }

    // ========================================================================
    // 4. INTEGRATION & BENCHMARKS (TOOL DISPATCH & PERFORMANCE)
    // ========================================================================

    #[tokio::test]
    async fn test_tool_registry_spatial_interact_lifecycle() {
        let mem = Arc::new(Mutex::new(MemoryStore::new()));
        let reg = ToolRegistry::new(mem);
        let ws = std::env::temp_dir();

        // 1. Spatial ground action
        let call_ground = ScheduledToolCall {
            id: "sp1".into(),
            name: "spatial_interact".into(),
            args: json!({
                "action": "ground",
                "target_id": "btn-run"
            }),
        };
        let res_ground = reg.execute_single(&call_ground, &ws, ExecutionAuthorityLevel::Autonomous, None).await;
        assert!(res_ground.success);
        assert_eq!(res_ground.output["success"], true);
        assert_eq!(res_ground.output["grounded"]["target"]["id"], "btn-run");

        // 2. Spatial execute with Double-Wrapper mediation
        let call_exec = ScheduledToolCall {
            id: "sp2".into(),
            name: "spatial_interact".into(),
            args: json!({
                "action": "execute",
                "target_action": "click",
                "target_id": "btn-run"
            }),
        };
        let res_exec = reg.execute_single(&call_exec, &ws, ExecutionAuthorityLevel::Autonomous, None).await;
        assert!(res_exec.success);
        assert_eq!(res_exec.output["action"], "click");
        assert_eq!(res_exec.output["groundedTarget"]["id"], "btn-run");
        assert!(res_exec.output["assuranceReceipt"]["postconditions_met"].as_bool().unwrap());

        // 3. Directional query via tool
        let call_query = ScheduledToolCall {
            id: "sp3".into(),
            name: "spatial_interact".into(),
            args: json!({
                "action": "query",
                "origin_id": "btn-run",
                "direction": "right"
            }),
        };
        let res_query = reg.execute_single(&call_query, &ws, ExecutionAuthorityLevel::Autonomous, None).await;
        assert!(res_query.success);
        assert_eq!(res_query.output["matched"]["id"], "inp-cmd");

        // 4. Audit health tool
        let call_audit = ScheduledToolCall {
            id: "sp4".into(),
            name: "spatial_interact".into(),
            args: json!({
                "action": "audit"
            }),
        };
        let res_audit = reg.execute_single(&call_audit, &ws, ExecutionAuthorityLevel::Autonomous, None).await;
        assert!(res_audit.success);
        assert_eq!(res_audit.output["healthy"], true);
        assert!(res_audit.output["confirmedActions"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn test_microbenchmark_spatial_grounding_throughput() {
        let mut med = SpatialActionMediator::new(1440, 900, "win-1");
        let mut scene = SpatialScene::new(1440, 900, "win-1");
        for i in 0..100 {
            scene.targets.push(SpatialTarget::new(
                format!("target-{}", i),
                Some(i as u32),
                "button",
                format!("Target Button {}", i),
                [(i % 10) * 100, (i / 10) * 50, 80, 30],
            ));
        }
        med.update_scene(scene);

        let contract = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("target-55".into()),
            ..Default::default()
        };

        // Warm up
        let _ = med.ground_action(&contract);

        // Measure 10,000 groundings
        let iters = 10_000;
        let start = Instant::now();
        for _ in 0..iters {
            let res = med.ground_action(&contract);
            assert!(res.success);
        }
        let elapsed = start.elapsed();
        let per_op_micros = elapsed.as_micros() as f64 / iters as f64;
        println!("Spatial Grounding Benchmark: {:.3} µs / operation ({} ops in {:?})", per_op_micros, iters, elapsed);
        assert!(per_op_micros < 10.0, "Spatial grounding must execute under 10 µs; got {:.3} µs", per_op_micros);
    }

    #[test]
    fn test_offscreen_auto_scroll_and_multi_step_spatial_plan_execution() {
        let disp = Arc::new(Mutex::new(VirtualDisplayState::new()));
        {
            let mut d = disp.lock().unwrap();
            d.windows[0].elements.push(UiElement {
                som_id: 77,
                id: "inp-offscreen".into(),
                role: "textbox".into(),
                label: "Offscreen Telemetry Input".into(),
                value: String::new(),
                bounds: [120, 1800, 320, 40],
                enabled: true,
                focused: false,
            });
            d.windows[0].elements.push(UiElement {
                som_id: 78,
                id: "btn-offscreen".into(),
                role: "button".into(),
                label: "Offscreen Deploy Button".into(),
                value: String::new(),
                bounds: [120, 2600, 200, 40],
                enabled: true,
                focused: false,
            });
        }

        let engine = SpatialInteractionEngine::new(disp.clone());

        // 1. Multi-step plan execution (scroll into view -> click to focus -> type value)
        let type_contract = SpatialActionContract {
            action_name: "type".into(),
            target_selector: Some("inp-offscreen".into()),
            payload_text: Some("plus-ultra-77".into()),
            ..Default::default()
        };
        let plan_res = engine.execute_plan(&type_contract).unwrap();
        assert_eq!(plan_res["success"], true);
        assert_eq!(plan_res["plannedSteps"], 3);
        assert_eq!(plan_res["stepCount"], 3);
        assert_eq!(plan_res["steps"][0]["action"], "scroll");
        assert_eq!(plan_res["steps"][1]["action"], "click");
        assert_eq!(plan_res["steps"][2]["action"], "type");

        {
            let d = disp.lock().unwrap();
            assert!(d.scroll_offset[1] > 0, "Expected positive vertical scroll offset after offscreen plan");
            let el = d.windows[0].elements.iter().find(|e| e.id == "inp-offscreen").unwrap();
            assert!(el.focused);
            assert_eq!(el.value, "plus-ultra-77");
        }

        // 2. Direct off-screen click with automatic pre-scroll remediation
        let click_offscreen = SpatialActionContract {
            action_name: "click".into(),
            target_selector: Some("btn-offscreen".into()),
            ..Default::default()
        };
        let click_res = engine.execute_spatially_grounded_action(&click_offscreen).unwrap();
        assert_eq!(click_res["success"], true);
        assert_eq!(click_res["autoScrolled"], true);
        assert_eq!(click_res["groundedTarget"]["visibilityRatio"], 1.0);
    }
}