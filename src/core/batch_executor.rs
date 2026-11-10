// [LAYER: CORE]
use crate::core::spatial_engine::SpatialInteractionEngine;
use crate::core::tool_registry::ToolRegistry;
use crate::domain::interfaces::ISwarmIntelligenceConsultant;
use crate::domain::types::runtime::{CancellationToken, ExecutionAuthorityLevel};
use crate::domain::types::spatial_types::SpatialActionContract;
use crate::domain::types::swarm_intelligence_types::SwarmConsultationRequest;
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::infrastructure::vfs_store::SkillsIngestor;
use crate::plumbing::text_tokens::canonical_tool_name;
use futures::future::join_all;
use serde_json::{json, Map, Value};
use std::path::Path;

pub const MAX_UNLOCKED_DEFERRED_TOOLS: usize = 8;
pub const MAX_DECLARATIVE_BATCH_STEPS: usize = 32;

impl ToolRegistry {
    /// Records a deferred specialist tool in the bounded 8-slot LRU active schema window (`Codewhale tool_search` eviction invariant).
    pub fn touch_unlocked_lru(&self, spec_name: &'static str) {
        let mut unlocked = self.unlocked_tools.lock().unwrap();
        let mut lru = self.unlocked_lru.lock().unwrap();
        let mut evicted = self.evicted_tools.lock().unwrap();

        lru.retain(|&n| n != spec_name);
        lru.push(spec_name);
        unlocked.insert(spec_name);
        evicted.retain(|&n| n != spec_name);

        while lru.len() > MAX_UNLOCKED_DEFERRED_TOOLS {
            let oldest = lru.remove(0);
            unlocked.remove(oldest);
            if !evicted.contains(&oldest) {
                evicted.push(oldest);
            }
        }
    }

    /// Searches deferred specialist tools by keyword/name and unlocks matching schemas into the 8-slot LRU set.
    pub fn search_and_unlock_tools(&self, query: &str, limit: usize, forbid_swarm: bool) -> Value {
        let capped_limit = limit.clamp(1, MAX_UNLOCKED_DEFERRED_TOOLS);
        let terms: Vec<String> = query
            .split(|c: char| c.is_whitespace() || c == ',' || c == '|')
            .map(|t| t.trim().to_ascii_lowercase())
            .filter(|t| !t.is_empty())
            .collect();

        let mut scored: Vec<(usize, &'static str, &'static str)> = Vec::new();
        for spec in self.list_specs() {
            if spec.is_core || (forbid_swarm && spec.name == "engage_swarm") {
                continue;
            }
            let hay_name = spec.name.to_ascii_lowercase();
            let hay_desc = spec.description.to_ascii_lowercase();
            let mut score = 0usize;
            if terms.is_empty() {
                score = 1;
            } else {
                for term in &terms {
                    if hay_name == *term {
                        score += 20;
                    } else if hay_name.contains(term) {
                        score += 10;
                    } else if hay_desc.contains(term) {
                        score += 4;
                    }
                }
            }
            if score > 0 {
                scored.push((score, spec.name, spec.description));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));

        let mut unlocked_now = Vec::new();
        for (_, name, desc) in scored.into_iter().take(capped_limit) {
            self.touch_unlocked_lru(name);
            unlocked_now.push(json!({
                "name": name,
                "description": desc,
            }));
        }

        let active_deferred: Vec<&'static str> = self.unlocked_lru.lock().unwrap().clone();
        let evicted_refs: Vec<&'static str> = self.evicted_tools.lock().unwrap().clone();
        json!({
            "query": query,
            "matchedCount": unlocked_now.len(),
            "unlocked": unlocked_now,
            "activeDeferredTools": active_deferred,
            "evictedToolReferences": evicted_refs,
            "lruCap": MAX_UNLOCKED_DEFERRED_TOOLS,
        })
    }

    /// Executes a declarative `execute_tools` batch (up to 32 steps) using dependency-safe wave parallelism
    /// while preserving full posture, read-before-write, Seatbelt, and secret-redaction enforcement per step.
    pub async fn execute_declarative_batch(
        &self,
        calls_val: &Value,
        stop_on_error: bool,
        cwd: &Path,
        authority: ExecutionAuthorityLevel,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<(Value, bool), String> {
        let items = calls_val
            .as_array()
            .ok_or_else(|| "execute_tools requires 'calls' to be a non-empty array of { tool, args } objects".to_string())?;
        if items.is_empty() {
            return Err("execute_tools 'calls' array cannot be empty".to_string());
        }
        if items.len() > MAX_DECLARATIVE_BATCH_STEPS {
            return Err(format!(
                "execute_tools exceeds maximum step budget ({} > {})",
                items.len(),
                MAX_DECLARATIVE_BATCH_STEPS
            ));
        }

        let mut scheduled = Vec::with_capacity(items.len());
        for (idx, item) in items.iter().enumerate() {
            let obj = item
                .as_object()
                .ok_or_else(|| format!("execute_tools step #{idx} must be a JSON object"))?;
            let raw_name = obj
                .get("tool")
                .or_else(|| obj.get("name"))
                .and_then(Value::as_str)
                .ok_or_else(|| format!("execute_tools step #{idx} missing 'tool' string"))?;
            let canonical = canonical_tool_name(raw_name);
            if canonical == "execute_tools" {
                return Err("Nested execute_tools recursion is forbidden".to_string());
            }
            if canonical == "engage_swarm" {
                return Err("engage_swarm cannot be invoked inside execute_tools".to_string());
            }
            let step_args = obj
                .get("args")
                .or_else(|| obj.get("arguments"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            scheduled.push(ScheduledToolCall {
                id: format!("batch_step_{}", idx + 1),
                name: raw_name.to_string(),
                args: step_args,
            });
        }

        let waves = self.partition_waves(&scheduled, cwd);
        let wave_count = waves.len();
        let mut step_outputs = Vec::with_capacity(scheduled.len());
        let mut any_denied = false;
        let mut halted_early = false;

        for wave in waves {
            if cancel_token.map(|t| t.is_cancelled()).unwrap_or(false) {
                halted_early = true;
                break;
            }
            let wave_records = if wave.len() == 1 {
                vec![Box::pin(self.execute_single(&wave[0], cwd, authority, cancel_token)).await]
            } else {
                let futs = wave
                    .iter()
                    .map(|c| Box::pin(self.execute_single(c, cwd, authority, cancel_token)));
                join_all(futs).await
            };
            let mut wave_failed = false;
            for rec in wave_records {
                if rec.denied == Some(true) {
                    any_denied = true;
                }
                if !rec.success {
                    wave_failed = true;
                }
                let governed = self.governor.lock().unwrap().govern(&rec.output);
                step_outputs.push(json!({
                    "stepId": rec.call_id,
                    "tool": rec.name,
                    "success": rec.success,
                    "denied": rec.denied.unwrap_or(false),
                    "durationMs": rec.duration_ms,
                    "error": rec.error,
                    "output": governed.output_text,
                    "spillId": governed.spill_id,
                }));
            }
            if stop_on_error && wave_failed {
                halted_early = true;
                break;
            }
        }

        let succeeded = step_outputs
            .iter()
            .filter(|s| s.get("success").and_then(Value::as_bool) == Some(true))
            .count();
        Ok((
            json!({
                "totalRequested": scheduled.len(),
                "executedCount": step_outputs.len(),
                "succeededCount": succeeded,
                "waveCount": wave_count,
                "haltedEarly": halted_early,
                "steps": step_outputs,
            }),
            any_denied,
        ))
    }

    pub(crate) fn dispatch_computer_use_tool(&self, args: &Map<String, Value>) -> Result<(Value, bool), String> {
        let act = args.get("action").and_then(|v| v.as_str()).unwrap_or("capture").trim().to_ascii_lowercase();
        match act.as_str() {
            "orchestrate" | "closed_loop" => {
                let goal = args.get("goal").or_else(|| args.get("objective")).and_then(|v| v.as_str()).unwrap_or("Execute computer use operation");
                let contract = SpatialActionContract {
                    action_name: args.get("target_action").or_else(|| args.get("sub_action")).and_then(|v| v.as_str()).unwrap_or("click").to_string(),
                    target_selector: args.get("target_id").or_else(|| args.get("selector")).or_else(|| args.get("ref")).or_else(|| args.get("element_id")).and_then(|v| v.as_str()).map(String::from),
                    som_id: args.get("som_id").and_then(|v| v.as_u64()).map(|n| n as u32),
                    delivery_mode: args.get("delivery_mode").and_then(|v| v.as_str()).unwrap_or("background").to_string(),
                    payload_text: args.get("text").or_else(|| args.get("value")).and_then(|v| v.as_str()).map(String::from),
                    keys: args.get("keys").or_else(|| args.get("key")).and_then(|v| v.as_str()).map(String::from),
                    ..SpatialActionContract::default()
                };
                Ok((self.computer_orchestrator.execute_closed_loop_cycle(goal, &contract)?, false))
            }
            "observe" | "env_observe" => Ok((serde_json::to_value(self.computer_orchestrator.observe()).unwrap_or(Value::Null), false)),
            "interpret" | "env_interpret" => {
                let obs = self.computer_orchestrator.observe();
                Ok((self.computer_orchestrator.interpret(&obs), false))
            }
            "consult_swarm" | "swarm_consult" => {
                let goal = args.get("goal").or_else(|| args.get("objective")).and_then(|v| v.as_str()).unwrap_or("Environmental interpretation");
                let app = args.get("app").and_then(|v| v.as_str()).unwrap_or("Heav3nsCode");
                let req = SwarmConsultationRequest::new("consult-cli", goal, app, "Active Window");
                Ok((serde_json::to_value(self.computer_orchestrator.consult(&req)).unwrap_or(Value::Null), false))
            }
            _ => {
                let val_args = Value::Object(args.clone());
                let mut disp = self.display_state.lock().map_err(|_| "Display lock poisoned")?;
                Ok((disp.execute_action(&val_args)?, false))
            }
        }
    }

    pub(crate) fn dispatch_spatial_tool(&self, args: &Map<String, Value>) -> Result<(Value, bool), String> {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("execute");
        let contract = SpatialActionContract {
            action_name: args.get("target_action").or_else(|| args.get("sub_action")).and_then(|v| v.as_str()).unwrap_or("click").to_string(),
            target_selector: args.get("target_id").or_else(|| args.get("selector")).or_else(|| args.get("ref")).and_then(|v| v.as_str()).map(String::from),
            som_id: args.get("som_id").and_then(|v| v.as_u64()).map(|n| n as u32),
            coordinates: if args.get("x").is_some() && args.get("y").is_some() {
                Some([args.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0), args.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0)])
            } else {
                None
            },
            coordinate_space: args.get("coordinate_space").and_then(|v| v.as_str()).unwrap_or("screen").to_string(),
            delivery_mode: args.get("delivery_mode").and_then(|v| v.as_str()).unwrap_or("background").to_string(),
            required_confidence: args.get("required_confidence").and_then(|v| v.as_f64()).unwrap_or(0.50) as f32,
            max_occlusion: args.get("max_occlusion").and_then(|v| v.as_f64()).unwrap_or(0.60),
            expected_epoch: args.get("expected_epoch").and_then(|v| v.as_u64()),
            payload_text: args.get("text").or_else(|| args.get("value")).and_then(|v| v.as_str()).map(String::from),
            keys: args.get("keys").or_else(|| args.get("key")).and_then(|v| v.as_str()).map(String::from),
            to_coordinates: None,
            exclusion_zones: Vec::new(),
            auto_scroll_if_needed: true,
        };
        match action {
            "ground" | "plan" => {
                let disp = self.display_state.lock().map_err(|_| "Display lock poisoned")?;
                let fresh = SpatialInteractionEngine::extract_scene_from_display(&disp);
                let mut med = self.spatial_engine.mediator.lock().map_err(|_| "Mediator lock poisoned")?;
                med.update_scene(fresh);
                let out = if action == "ground" {
                    serde_json::to_value(med.ground_action(&contract)).unwrap_or(Value::Null)
                } else {
                    serde_json::to_value(med.plan_action(&contract)).unwrap_or(Value::Null)
                };
                Ok((out, false))
            }
            "execute_plan" | "run_plan" => Ok((self.spatial_engine.execute_plan(&contract)?, false)),
            "query" => {
                let origin = args.get("origin_id").or_else(|| args.get("target_id")).and_then(|v| v.as_str()).unwrap_or("");
                let dir = args.get("direction").and_then(|v| v.as_str()).unwrap_or("right");
                Ok((json!({ "origin": origin, "direction": dir, "matched": self.spatial_engine.query_directional(origin, dir) }), false))
            }
            "reconcile" => {
                let dom_els = args.get("interactiveElements").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                Ok((serde_json::to_value(self.spatial_engine.reconcile_dom_and_visual(&dom_els)).unwrap_or(Value::Null), false))
            }
            "audit" => Ok((self.spatial_engine.audit_health(), false)),
            "orchestrate" | "closed_loop" => {
                let goal = args.get("goal").or_else(|| args.get("objective")).and_then(|v| v.as_str()).unwrap_or("Spatial closed-loop execution");
                Ok((self.computer_orchestrator.execute_closed_loop_cycle(goal, &contract)?, false))
            }
            _ => Ok((self.spatial_engine.execute_spatially_grounded_action(&contract)?, false)),
        }
    }

    pub(crate) fn dispatch_skill_forge_tool(&self, args: &Map<String, Value>, cwd: &Path) -> Result<(Value, bool), String> {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list_dag");
        let val_args = Value::Object(args.clone());
        let mut cat = self.skill_catalog.lock().map_err(|_| "Skill catalog lock poisoned")?;
        let res = match action {
            "list_dag" | "dag" | "list" => cat.evaluate_dag(),
            "forge" | "create" | "wizard" => cat.forge_skill(&val_args)?,
            "apply_pack" | "power_up" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator");
                let pack = args.get("pack").and_then(|v| v.as_str()).unwrap_or("tdd-enforcer");
                cat.apply_power_up_pack(id, pack)?
            }
            "clone" | "fork" => {
                let src = args.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator");
                let new_id = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("computer-use-fork");
                cat.clone_skill(src, new_id, None)?
            }
            "lint" => cat.lint_and_autofix_skill(args.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator"), false)?,
            "autofix" | "doctor" => cat.lint_and_autofix_skill(args.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator"), true)?,
            "sync_vault" | "sync" => {
                let mut ingested = Vec::new();
                for (fname, raw) in SkillsIngestor::read_drop_vault_files(cwd) {
                    if let Ok(item) = cat.ingest_multi_format(&fname, &raw) {
                        ingested.push(item);
                    }
                }
                json!({ "syncedCount": ingested.len(), "totalCatalogSkills": cat.nodes.len(), "activeSoul": cat.active_soul.name, "ingested": ingested })
            }
            "export" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("computer-use-operator");
                let fmt = args.get("format").and_then(|v| v.as_str()).unwrap_or("markdown");
                cat.export_node(id, fmt)?
            }
            "ingest" => {
                let fname = args.get("id").and_then(|v| v.as_str()).unwrap_or("imported.md");
                let content = args.get("content").or_else(|| args.get("prompt")).and_then(|v| v.as_str()).unwrap_or("");
                cat.ingest_multi_format(fname, content)?
            }
            "soul_status" | "soul" => json!({ "activeSoul": cat.active_soul, "competencies": cat.competency_scores }),
            other => return Err(format!("Unsupported skill_forge action '{other}'")),
        };
        Ok((res, false))
    }
}
