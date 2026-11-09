// [LAYER: PLUMBING]
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Type aliases for environmental diff and analysis payloads.
pub type ElementDelta = Value;
pub type AffordanceClassification = Value;
pub type FailureDiagnosis = Value;

/// Calculates the structural, visual, and state difference between two snapshots of UI elements.
pub fn compute_element_deltas(
    before_elements: &[Value],
    after_elements: &[Value],
) -> Value {
    let mut before_map: BTreeMap<String, &Value> = BTreeMap::new();
    for el in before_elements {
        if let Some(id) = el.get("id").and_then(|v| v.as_str()) {
            before_map.insert(id.to_string(), el);
        }
    }

    let mut after_map: BTreeMap<String, &Value> = BTreeMap::new();
    for el in after_elements {
        if let Some(id) = el.get("id").and_then(|v| v.as_str()) {
            after_map.insert(id.to_string(), el);
        }
    }

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    for (id, after_el) in &after_map {
        match before_map.get(id) {
            None => added.push((*after_el).clone()),
            Some(before_el) => {
                let val_changed = before_el.get("value") != after_el.get("value");
                let focus_changed = before_el.get("focused") != after_el.get("focused");
                let bounds_changed = before_el.get("bounds") != after_el.get("bounds");
                let label_changed = before_el.get("label") != after_el.get("label");
                let enabled_changed = before_el.get("enabled") != after_el.get("enabled");
                if val_changed || focus_changed || bounds_changed || label_changed || enabled_changed {
                    modified.push(json!({
                        "id": id,
                        "valueChanged": val_changed,
                        "focusChanged": focus_changed,
                        "boundsChanged": bounds_changed,
                        "labelChanged": label_changed,
                        "enabledChanged": enabled_changed,
                        "beforeValue": before_el.get("value"),
                        "afterValue": after_el.get("value"),
                        "afterFocused": after_el.get("focused")
                    }));
                }
            }
        }
    }

    for (id, before_el) in &before_map {
        if !after_map.contains_key(id) {
            removed.push((*before_el).clone());
        }
    }

    let has_mutation = !added.is_empty() || !removed.is_empty() || !modified.is_empty();
    json!({
        "hasObservableMutation": has_mutation,
        "addedCount": added.len(),
        "removedCount": removed.len(),
        "modifiedCount": modified.len(),
        "added": added,
        "removed": removed,
        "modified": modified
    })
}

/// Infers behavioral affordance from role, label, value, and geometry without external I/O.
pub fn classify_element_affordance(
    role: &str,
    label: &str,
    value: &str,
    bounds: [i32; 4],
) -> (String, String, f32) {
    let r = role.trim().to_ascii_lowercase();
    let l = label.trim().to_ascii_lowercase();
    let _v = value.trim();

    // Check for disabled constraint
    if r.contains("disabled") || l.contains("(disabled)") || l.ends_with(" disabled") {
        return ("disabled_control".into(), "Control is currently disabled and rejects input".into(), 0.98);
    }

    // Check for modal dismiss / close cues
    if l == "close" || l == "cancel" || l == "dismiss" || l == "×" || l == "x" || l.contains("exit") {
        return ("dismissable".into(), "Closes or dismisses the active modal dialog".into(), 0.95);
    }

    // Check for search / filter cues
    if r == "searchbox" || l.contains("search") || l.contains("find") || l.contains("filter") {
        return ("search_input".into(), "Accepts search query and updates visible results".into(), 0.92);
    }

    // Check for submission / primary CTA cues
    if l == "submit" || l == "save" || l == "confirm" || l == "ok" || l == "apply" || l.starts_with("run") || l.contains("submit") || l.contains("save") || l.contains("verify") {
        return ("submit_action".into(), "Commits form changes or executes command".into(), 0.90);
    }

    // Check for navigation cues
    if r == "link" || l.contains("home") || l.contains("docs") || l.contains("nav") || l.starts_with("http") {
        return ("navigational".into(), "Navigates to another view, document, or URL".into(), 0.88);
    }

    // Check for input-capable textbox
    if matches!(r.as_str(), "textbox" | "input" | "textarea" | "combobox" | "text") {
        return ("text_input".into(), "Accepts keyboard keystrokes and text entry".into(), 0.95);
    }

    // Check for clickable controls
    if matches!(r.as_str(), "button" | "checkbox" | "radio" | "tab" | "menuitem" | "switch" | "slider") {
        return ("clickable".into(), "Toggles state or triggers command on click".into(), 0.85);
    }

    // Geometry heuristic: very wide or tall scrollable containers
    if bounds[2] > 600 && bounds[3] > 400 {
        return ("scrollable_container".into(), "Scrollable content viewport".into(), 0.70);
    }

    ("generic_interactive".into(), "Inspect or click to evaluate outcome".into(), 0.50)
}

/// Identifies whether any active window or element represents an unhandled modal or alert dialog.
pub fn detect_modal_or_dialog(
    titles: &[&str],
    roles: &[&str],
    labels: &[&str],
) -> Option<String> {
    for (idx, role) in roles.iter().enumerate() {
        let r = role.to_ascii_lowercase();
        if r == "dialog" || r == "alertdialog" || r == "modal" {
            let label = labels.get(idx).copied().unwrap_or("Modal Dialog");
            return Some(label.to_string());
        }
    }
    for title in titles {
        let t = title.to_ascii_lowercase();
        if t.contains("dialog") || t.contains("alert") || t.contains("confirm") || t.contains("permission") {
            return Some((*title).to_string());
        }
    }
    None
}

/// Diagnoses root cause of a failed or suspected no-op action and suggests an adaptive escalation strategy.
pub fn diagnose_failure_cause(
    action: &str,
    delivery_mode: &str,
    target_role: &str,
    was_focused_before: bool,
    was_focused_after: bool,
    value_changed: bool,
    modal_blocking: bool,
) -> (String, String) {
    if modal_blocking {
        return (
            "modal_dialog_intercepted_event".into(),
            "Dismiss blocking modal with Escape key or click its Cancel/Close button".into(),
        );
    }

    let act = action.to_ascii_lowercase();
    if act == "type" || act == "set_value" || act == "fill" {
        if !was_focused_before && !was_focused_after && !value_changed {
            return (
                "target_lacked_focus".into(),
                "Execute click on target element to acquire keyboard focus before typing".into(),
            );
        }
        if !value_changed {
            return (
                "synthetic_input_not_accepted".into(),
                "Escalate to explicit fill/set_value or clipboard paste sequence".into(),
            );
        }
    }

    if act == "click" || act == "double_click" {
        if delivery_mode == "background" {
            return (
                "background_event_ignored_by_window".into(),
                "Escalate delivery_mode to 'foreground' and bring target window to front".into(),
            );
        }
        let r = target_role.to_ascii_lowercase();
        if matches!(r.as_str(), "textbox" | "input") && !was_focused_after {
            return (
                "click_failed_to_focus".into(),
                "Use Tab key navigation to cycle focus directly to target element".into(),
            );
        }
    }

    (
        "unverifiable_external_side_effect".into(),
        "Query fresh Set-of-Marks frame to verify actual screen change".into(),
    )
}

/// Calibrates confidence score based on provenance, visibility, and historical success.
pub fn calibrate_confidence_score(
    base_confidence: f32,
    visibility_ratio: f64,
    occlusion_ratio: f64,
    success_count: u32,
    failure_count: u32,
) -> f32 {
    let mut score = base_confidence * (visibility_ratio as f32).clamp(0.1, 1.0);
    // Penalize occlusion
    score *= (1.0 - (occlusion_ratio as f32 * 0.8)).clamp(0.2, 1.0);
    // Factor in historical track record
    let total = success_count + failure_count;
    if total > 0 {
        let historical_ratio = success_count as f32 / total as f32;
        score = (score * 0.6) + (historical_ratio * 0.4);
    }
    score.clamp(0.05, 1.0)
}

/// Extracts structural relationships (parent, siblings, children, label-for) between elements.
pub fn analyze_spider_structural_relationships(
    elements: &[Value],
) -> Vec<Value> {
    let mut relationships = Vec::new();
    let mut id_set = BTreeSet::new();
    for el in elements {
        if let Some(id) = el.get("id").and_then(|v| v.as_str()) {
            id_set.insert(id.to_string());
        }
    }

    for el in elements {
        let id = el.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let role = el.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let label = el.get("label").and_then(|v| v.as_str()).unwrap_or("");
        let bounds = el.get("bounds").and_then(|v| v.as_array());

        // Check if an input field is adjacent to a label, icon, or submit button
        if matches!(role, "textbox" | "input" | "textarea" | "combobox" | "searchbox") {
            let b_coords = bounds.and_then(|b| {
                if b.len() == 4 {
                    Some((b[0].as_i64().unwrap_or(0), b[1].as_i64().unwrap_or(0), b[2].as_i64().unwrap_or(0), b[3].as_i64().unwrap_or(0)))
                } else {
                    None
                }
            });
            for other in elements {
                let other_id = other.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let other_role = other.get("role").and_then(|v| v.as_str()).unwrap_or("");
                let other_label = other.get("label").and_then(|v| v.as_str()).unwrap_or("");
                if other_id != id && (other_role == "button" || other_role == "label" || other_role == "text") {
                    let text = format!("{other_label} {label}").to_lowercase();
                    if text.contains("search") || text.contains("find") {
                        relationships.push(json!({
                            "sourceId": id,
                            "targetId": other_id,
                            "relation": "associated_search_pair",
                            "semantic": "Input field accompanied by search action button"
                        }));
                    } else if other_role == "button" {
                        let btn_l = other_label.trim().to_ascii_lowercase();
                        if btn_l == "submit" || btn_l == "save" || btn_l == "confirm" || btn_l == "ok" || btn_l.starts_with("run") {
                            relationships.push(json!({
                                "sourceId": id,
                                "targetId": other_id,
                                "relation": "form_submission_group",
                                "semantic": format!("Input '{}' submits via primary control '{}'", id, other_id)
                            }));
                        }
                    } else if (other_role == "label" || other_role == "text") && !other_label.trim().is_empty() {
                        if let (Some((ix, iy, _iw, ih)), Some(ob)) = (b_coords, other.get("bounds").and_then(|v| v.as_array())) {
                            if ob.len() == 4 {
                                let (lx, ly, lw, lh) = (ob[0].as_i64().unwrap_or(0), ob[1].as_i64().unwrap_or(0), ob[2].as_i64().unwrap_or(0), ob[3].as_i64().unwrap_or(0));
                                let left_adjacent = lx + lw <= ix + 24 && (ix - (lx + lw)) <= 160 && (ly - iy).abs() <= ih.max(lh);
                                let top_adjacent = ly + lh <= iy + 12 && (iy - (ly + lh)) <= 56 && (lx - ix).abs() <= 80;
                                if left_adjacent || top_adjacent {
                                    relationships.push(json!({
                                        "sourceId": other_id,
                                        "targetId": id,
                                        "relation": "label_for",
                                        "semantic": format!("Label '{}' describes input field '{}'", other_label, id)
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Geometric containment and modal dismiss relation
        if let Some(b) = bounds {
            if b.len() == 4 {
                let (x, y, w, h) = (b[0].as_i64().unwrap_or(0), b[1].as_i64().unwrap_or(0), b[2].as_i64().unwrap_or(0), b[3].as_i64().unwrap_or(0));
                let is_modal_container = matches!(role, "dialog" | "alertdialog" | "modal");
                for other in elements {
                    let oid = other.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if oid != id {
                        if let Some(ob) = other.get("bounds").and_then(|v| v.as_array()) {
                            if ob.len() == 4 {
                                let (ox, oy, ow, oh) = (ob[0].as_i64().unwrap_or(0), ob[1].as_i64().unwrap_or(0), ob[2].as_i64().unwrap_or(0), ob[3].as_i64().unwrap_or(0));
                                if ox >= x && oy >= y && ox + ow <= x + w && oy + oh <= y + h {
                                    relationships.push(json!({
                                        "sourceId": id,
                                        "targetId": oid,
                                        "relation": "contains",
                                        "semantic": format!("Container '{}' spatially contains child '{}'", id, oid)
                                    }));
                                    if is_modal_container {
                                        let ol = other.get("label").and_then(|v| v.as_str()).unwrap_or("").trim().to_ascii_lowercase();
                                        if ol == "close" || ol == "cancel" || ol == "dismiss" || ol == "×" || ol == "x" {
                                            relationships.push(json!({
                                                "sourceId": oid,
                                                "targetId": id,
                                                "relation": "modal_dismiss_control",
                                                "semantic": format!("Control '{}' dismisses modal dialog '{}'", oid, id)
                                            }));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    relationships
}
