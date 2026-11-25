// [LAYER: CORE]
use crate::domain::interfaces::{
    IBehavioralKnowledgeStore, IComputerUseOrchestrator, IEnvironmentalDiffer,
    IEnvironmentalObserver, ISpatialGeometry, ISwarmIntelligenceConsultant,
};
use crate::domain::types::environmental_types::EnvironmentalObservation;
use crate::domain::types::spatial_types::SpatialActionContract;
use crate::domain::types::swarm_intelligence_types::{
    ConsultationRiskLevel, SpecialistInsight, SwarmConsultationRequest, SwarmConsultationResponse,
};
use crate::plumbing::environmental_diff::compute_element_deltas;
use serde_json::Value;
use crate::core::computer_use_orchestrator::ComputerUseOrchestrator;

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
                && !req.candidate_target_ids.iter().any(|id| {
                    id == &pat.pattern_id
                        || id == &pat.ui_trigger
                        || req.task_objective.contains(id)
                });
            let prior_failure_conflict = req.last_failure_reason.is_some()
                && req.last_failed_action.as_deref() == Some(pat.recommended_modality.as_str());

            historical_conflict_detected =
                modal_conflict || target_missing_conflict || prior_failure_conflict;

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
            format!(
                "Screen has {} interactive targets; primary application is '{}'",
                req.candidate_target_ids.len(),
                req.target_app
            ),
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
            format!(
                "Recommended action for objective '{}' is '{}'",
                req.task_objective, rec_action
            )
        };
        insights.push(SpecialistInsight::new(
            "WorkflowNavigator",
            wf_finding,
            0.85,
            "WorkflowStateMachine",
        ));

        // 4. RiskAssessor Specialist Insight
        let is_high_risk = req.task_objective.contains("delete")
            || req.task_objective.contains("rm ")
            || req.task_objective.contains("format");
        let risk_level = if is_high_risk {
            ConsultationRiskLevel::High
        } else {
            ConsultationRiskLevel::Low
        };
        insights.push(SpecialistInsight::new(
            "RiskAssessor",
            if is_high_risk {
                "High risk destructive action flagged; requires confirmation fence"
            } else {
                "Standard non-destructive action"
            },
            0.99,
            "SafetySentinel",
        ));

        // 5. RecoverySpecialist Specialist Insight
        let recovery_advice = if let Some(ref reason) = req.last_failure_reason {
            format!(
                "Escalate delivery mode to 'foreground' and verify focus; prior error: {reason}"
            )
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
                .find(|id| {
                    id.as_str() == pat.pattern_id.as_str() || id.as_str() == pat.ui_trigger.as_str()
                })
                .cloned()
                .or_else(|| {
                    req.candidate_target_ids
                        .iter()
                        .find(|id| {
                            let lower_id = id.to_ascii_lowercase();
                            req.task_objective
                                .to_ascii_lowercase()
                                .split_whitespace()
                                .any(|word| word.len() > 2 && lower_id.contains(word))
                        })
                        .cloned()
                })
        } else {
            req.candidate_target_ids
                .iter()
                .find(|id| {
                    let lower_id = id.to_ascii_lowercase();
                    req.task_objective
                        .to_ascii_lowercase()
                        .split_whitespace()
                        .any(|word| word.len() > 2 && lower_id.contains(word))
                })
                .cloned()
                .or_else(|| req.candidate_target_ids.first().cloned())
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
    fn execute_closed_loop(
        &self,
        goal: &str,
        contract: &SpatialActionContract,
    ) -> Result<Value, String> {
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
