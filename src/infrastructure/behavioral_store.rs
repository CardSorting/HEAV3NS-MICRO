// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::{IBehavioralKnowledgeStore, IBehavioralStore, MemoryPersistenceCallback};
use crate::domain::types::learned_knowledge_types::{EnvironmentalReconciliationReport, LearnedInteractionPattern, LearnedWorkflowProcedure};
use crate::plumbing::hash_and_math::now_ms;
use std::collections::BTreeMap;

/// Concrete, durable behavioral knowledge store implementing [`IBehavioralStore`].
/// Maintains learned interface affordances, interaction procedures, and explicit state reconciliation
/// across session horizons and agent turns, backed by an optional decoupled memory sink.
#[derive(Clone)]
pub struct BehavioralKnowledgeStore {
    pub patterns: BTreeMap<String, LearnedInteractionPattern>,
    pub procedures: BTreeMap<String, LearnedWorkflowProcedure>,
    pub memory_sink: Option<MemoryPersistenceCallback>,
    pub current_epoch: u64,
}

impl std::fmt::Debug for BehavioralKnowledgeStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BehavioralKnowledgeStore")
            .field("patterns", &self.patterns)
            .field("procedures", &self.procedures)
            .field("has_memory_sink", &self.memory_sink.is_some())
            .field("current_epoch", &self.current_epoch)
            .finish()
    }
}

impl Default for BehavioralKnowledgeStore {
    fn default() -> Self {
        Self::new(None)
    }
}

impl BehavioralKnowledgeStore {
    pub extern "Rust" fn new(memory_sink: Option<MemoryPersistenceCallback>) -> Self {
        let mut store = Self {
            patterns: BTreeMap::new(),
            procedures: BTreeMap::new(),
            memory_sink,
            current_epoch: 1,
        };
        store.seed_standard_conventions();
        store
    }

    /// Seeds industry-standard, battle-tested UI behavioral conventions.
    fn seed_standard_conventions(&mut self) {
        let conventions = vec![
            ("search_bar", "*", "textbox[role=searchbox]", "text_input", "type"),
            ("submit_btn", "*", "button[name=submit]", "submit_action", "click"),
            ("close_modal", "*", "button[label=close]", "dismissable", "click"),
            ("nav_link", "*", "link", "navigational", "click"),
            ("terminal_stdin", "Terminal", "textarea[label=Terminal Input]", "text_input", "type"),
            ("flight_verify", "Heav3nsCode", "button[id=btn-run]", "submit_action", "click"),
            ("swarm_engage", "Heav3nsCode", "button[id=btn-swarm]", "submit_action", "click"),
        ];

        for (id, app, trigger, affordance, mod_act) in conventions {
            let mut pat = LearnedInteractionPattern::new(id, app, trigger, affordance);
            pat.recommended_modality = mod_act.to_string();
            pat.confidence = 0.90;
            self.patterns.insert(id.to_string(), pat);
        }

        let procedures = vec![
            (
                "proc_focus_and_type",
                "*",
                "type into input field",
                vec!["click:target_input".into(), "type:target_input".into()],
            ),
            (
                "proc_dismiss_modal_and_retry",
                "*",
                "dismiss blocking modal dialog",
                vec!["key:Escape".into(), "verify:modal_closed".into(), "click:target".into()],
            ),
            (
                "proc_search_and_submit",
                "*",
                "search query and submit",
                vec!["click:search_bar".into(), "type:search_bar".into(), "key:Enter".into()],
            ),
        ];

        for (pid, app, goal, steps) in procedures {
            self.procedures.insert(
                pid.to_string(),
                LearnedWorkflowProcedure::new(pid, app, goal, steps, 1),
            );
        }
    }

    pub fn set_epoch(&mut self, epoch: u64) {
        self.current_epoch = epoch;
    }

    pub fn register_procedure(&mut self, proc: LearnedWorkflowProcedure) {
        self.procedures.insert(proc.procedure_id.clone(), proc);
    }

    pub fn lookup_procedure(&self, app: &str, goal: &str) -> Option<LearnedWorkflowProcedure> {
        let lower_goal = goal.to_ascii_lowercase();
        for proc in self.procedures.values() {
            if (proc.app == "*" || proc.app.eq_ignore_ascii_case(app))
                && (lower_goal.contains(&proc.goal.to_ascii_lowercase())
                    || proc.goal.to_ascii_lowercase().split_whitespace().all(|w| w.len() <= 3 || lower_goal.contains(w)))
            {
                return Some(proc.clone());
            }
        }
        None
    }

    pub fn list_procedures(&self) -> Vec<LearnedWorkflowProcedure> {
        self.procedures.values().cloned().collect()
    }

    pub fn list_patterns(&self) -> Vec<LearnedInteractionPattern> {
        self.patterns.values().cloned().collect()
    }
}

impl IBehavioralKnowledgeStore for BehavioralKnowledgeStore {
    fn record_outcome(&mut self, pattern_id: &str, app: &str, success: bool, epoch: u64) {
        let timestamp = now_ms();
        if let Some(pat) = self.patterns.get_mut(pattern_id) {
            if success {
                pat.record_success(epoch, timestamp);
            } else {
                pat.record_failure(epoch, timestamp);
            }
        } else {
            let lower_id = pattern_id.to_ascii_lowercase();
            let aff = if lower_id.contains("input") || lower_id.contains("txt") || lower_id.contains("search") || lower_id.contains("editor") {
                "text_input"
            } else {
                "clickable"
            };
            let mut pat = LearnedInteractionPattern::new(pattern_id, app, pattern_id, aff);
            if success {
                pat.record_success(epoch, timestamp);
            } else {
                pat.record_failure(epoch, timestamp);
            }
            self.patterns.insert(pattern_id.to_string(), pat);
        }

        // Mirror high-confidence verified patterns to decoupled memory sink
        if let Some(ref sink) = self.memory_sink {
            if let Some(pat) = self.patterns.get(pattern_id) {
                if pat.confidence >= 0.85 && pat.success_count >= 2 {
                    let key = format!("learned_ui_pattern:{app}:{pattern_id}");
                    let payload = serde_json::json!({
                        "patternId": pat.pattern_id,
                        "app": pat.app_pattern,
                        "affordance": pat.affordance,
                        "modality": pat.recommended_modality,
                        "confidence": pat.confidence,
                        "successCount": pat.success_count,
                        "provenanceEpoch": pat.provenance_epoch
                    });
                    sink(&key, &payload.to_string(), "behavioral_intelligence");
                }
            }
        }
    }

    fn lookup_pattern(&self, app: &str, trigger: &str) -> Option<LearnedInteractionPattern> {
        // Direct match by ID
        if let Some(pat) = self.patterns.get(trigger) {
            if pat.app_pattern == "*" || pat.app_pattern.eq_ignore_ascii_case(app) {
                return Some(pat.clone());
            }
        }

        // Heuristic match across registered patterns
        for pat in self.patterns.values() {
            if (pat.app_pattern == "*" || pat.app_pattern.eq_ignore_ascii_case(app))
                && (pat.ui_trigger.contains(trigger) || trigger.contains(&pat.ui_trigger))
            {
                return Some(pat.clone());
            }
        }
        None
    }

    fn reconcile_state(&self, recorded_epoch: u64, current_epoch: u64, hash_drift: bool) -> EnvironmentalReconciliationReport {
        if recorded_epoch == current_epoch && !hash_drift {
            return EnvironmentalReconciliationReport::synchronized(current_epoch);
        }

        let mut reasons = Vec::new();
        if recorded_epoch != current_epoch {
            reasons.push(format!("Layout epoch shifted from {recorded_epoch} to {current_epoch}"));
        }
        if hash_drift {
            reasons.push("Visual display state hash divergence detected (external side effects occurred)".into());
        }

        EnvironmentalReconciliationReport::diverged(
            recorded_epoch,
            current_epoch,
            hash_drift,
            reasons,
            "Capture fresh Set-of-Marks frame and re-ground target against current epoch",
        )
    }
}

impl IBehavioralStore for BehavioralKnowledgeStore {}
