// [LAYER: PLUMBING]
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Contractual sovereignty interface for action-ledger telemetry evaluation.
pub trait IDisplayTelemetry {
    fn evaluate_ledger_health(&self, ledger: &[Value]) -> Value;
}

/// Contractual sovereignty interface for preview element inventory diffing.
pub trait IPreviewDiffer {
    fn compute_delta(&self, current: &BTreeMap<String, Value>, previous: &BTreeMap<String, Value>) -> Value;
}

pub type LedgerQueryFilter = String;

/// Computes a full or delta preview inventory update for `drive_preview`.
pub fn compute_preview_delta(
    current: &BTreeMap<String, Value>,
    previous: &mut BTreeMap<String, Value>,
    full: bool,
) -> Value {
    if full {
        *previous = current.clone();
        let list: Vec<Value> = current.values().cloned().collect();
        return json!({ "mode": "full", "count": list.len(), "elements": list });
    }
    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    for (k, v) in current {
        match previous.get(k) {
            None => added.push(v.clone()),
            Some(old) if old != v => changed.push(v.clone()),
            _ => {}
        }
    }
    for k in previous.keys() {
        if !current.contains_key(k) {
            removed.push(k.clone());
        }
    }
    *previous = current.clone();
    json!({ "mode": "delta", "added": added, "changed": changed, "removed": removed })
}

/// Applies an annotation mutation (`add`, `pin`, `hold`, `remove`, `unpin`, `clear`) to the display overlay list.
pub fn apply_annotation_mutation(annotations: &mut Vec<Value>, op: &str, target_ref: &str, note: &str) -> Value {
    match op {
        "add" | "pin" | "hold" => {
            annotations.retain(|a| a.get("ref").and_then(|v| v.as_str()) != Some(target_ref));
            annotations.push(json!({
                "ref": target_ref,
                "kind": if op == "hold" { "hold" } else { "pin" },
                "note": note
            }));
        }
        "remove" | "unpin" => annotations.retain(|a| a.get("ref").and_then(|v| v.as_str()) != Some(target_ref)),
        "clear" => annotations.clear(),
        _ => {}
    }
    json!({ "op": op, "annotationCount": annotations.len(), "annotations": annotations })
}

/// Evaluates structured and free-text DSL queries against the display action ledger.
/// Supports `action:<name>`, `window:<id>`, `status:<status>`, `element:<id>`, `mode:<delivery>`,
/// `duration<<ms>`, `duration><ms>`, and free-text terms.
pub fn query_action_ledger_dsl(ledger: &[Value], query: &str) -> Value {
    let mut act_f: Option<String> = None;
    let mut win_f: Option<String> = None;
    let mut stat_f: Option<String> = None;
    let mut elem_f: Option<String> = None;
    let mut mode_f: Option<String> = None;
    let mut max_d: Option<u64> = None;
    let mut min_d: Option<u64> = None;
    let mut free_terms: Vec<String> = Vec::new();

    for token in query.split_whitespace() {
        if let Some(r) = token.strip_prefix("action:") {
            act_f = Some(r.to_ascii_lowercase());
        } else if let Some(r) = token.strip_prefix("window:") {
            win_f = Some(r.to_string());
        } else if let Some(r) = token.strip_prefix("status:") {
            stat_f = Some(r.to_ascii_lowercase());
        } else if let Some(r) = token.strip_prefix("element:") {
            elem_f = Some(r.to_ascii_lowercase());
        } else if let Some(r) = token.strip_prefix("mode:").or_else(|| token.strip_prefix("delivery:")) {
            mode_f = Some(r.to_ascii_lowercase());
        } else if let Some(r) = token.strip_prefix("duration<") {
            max_d = r.parse::<u64>().ok();
        } else if let Some(r) = token.strip_prefix("duration>") {
            min_d = r.parse::<u64>().ok();
        } else if !token.is_empty() {
            free_terms.push(token.to_ascii_lowercase());
        }
    }

    let matched: Vec<Value> = ledger
        .iter()
        .filter(|e| {
            let act_ok = act_f
                .as_ref()
                .map(|f| e.get("action").and_then(|v| v.as_str()).unwrap_or("").eq_ignore_ascii_case(f))
                .unwrap_or(true);
            let win_ok = win_f
                .as_ref()
                .map(|f| e.get("window").and_then(|v| v.as_str()).unwrap_or("") == f)
                .unwrap_or(true);
            let stat_ok = stat_f
                .as_ref()
                .map(|f| e.get("status").and_then(|v| v.as_str()).unwrap_or("").eq_ignore_ascii_case(f))
                .unwrap_or(true);
            let elem_ok = elem_f
                .as_ref()
                .map(|f| {
                    e.get("detail")
                        .and_then(|d| d.get("elementId"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .contains(f)
                })
                .unwrap_or(true);
            let mode_ok = mode_f
                .as_ref()
                .map(|f| {
                    e.get("detail")
                        .and_then(|d| d.get("deliveryMode"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .eq_ignore_ascii_case(f)
                })
                .unwrap_or(true);
            let dur = e.get("durationMs").and_then(|v| v.as_u64()).unwrap_or(0);
            let max_ok = max_d.map(|d| dur < d).unwrap_or(true);
            let min_ok = min_d.map(|d| dur > d).unwrap_or(true);
            let terms_ok = if free_terms.is_empty() {
                true
            } else {
                let hay = e.to_string().to_ascii_lowercase();
                free_terms.iter().all(|t| hay.contains(t))
            };
            act_ok && win_ok && stat_ok && elem_ok && mode_ok && max_ok && min_ok && terms_ok
        })
        .cloned()
        .collect();

    json!({
        "query": query,
        "totalActions": ledger.len(),
        "matchCount": matched.len(),
        "matches": matched
    })
}

/// Computes latency percentiles (p50/p95), success rates, and health metrics across the action ledger.
pub fn compute_action_ledger_health(
    ledger: &[Value],
    undo_depth: usize,
    redo_depth: usize,
    active_window_id: &str,
    window_count: usize,
    clipboard_len: usize,
    mouse_pressed: bool,
    lease_holder: &str,
    lease_epoch: u64,
) -> Value {
    let total = ledger.len();
    let successes = ledger
        .iter()
        .filter(|e| e.get("status").and_then(|v| v.as_str()) == Some("success"))
        .count();
    let mut durations: Vec<u64> = ledger
        .iter()
        .filter_map(|e| e.get("durationMs").and_then(|v| v.as_u64()))
        .collect();
    durations.sort_unstable();
    let p50 = durations.get(durations.len() / 2).copied().unwrap_or(1);
    let p95 = durations.get((durations.len() * 95) / 100).copied().unwrap_or(2);
    let success_rate = if total == 0 { 1.0 } else { successes as f64 / total as f64 };
    json!({
        "healthy": success_rate >= 0.95,
        "totalActions": total,
        "successRate": success_rate,
        "p50LatencyMs": p50,
        "p95LatencyMs": p95,
        "undoDepth": undo_depth,
        "redoDepth": redo_depth,
        "activeWindowId": active_window_id,
        "windowCount": window_count,
        "clipboardLength": clipboard_len,
        "mousePressed": mouse_pressed,
        "leaseHolder": lease_holder,
        "leaseEpoch": lease_epoch
    })
}

/// Extracts `(id, som_id, role, label, value, bounds)` tuples from a DOM snapshot
/// so `browser_navigate` can synchronize the window's interactive `UiElement` list.
pub fn extract_dom_ui_tuples(dom_snap: &Value) -> Vec<(String, u32, String, String, String, [i32; 4])> {
    let mut out = Vec::new();
    if let Some(arr) = dom_snap.get("elements").and_then(|v| v.as_array()) {
        for (idx, item) in arr.iter().enumerate() {
            let som_id = item.get("somId").and_then(|v| v.as_u64()).unwrap_or((idx + 1) as u64) as u32;
            let raw_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let id = if raw_id.is_empty() { format!("dom-el-{som_id}") } else { raw_id.to_string() };
            let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("generic").to_string();
            let label = item.get("label").and_then(|v| v.as_str()).unwrap_or(&id).to_string();
            let value = item.get("href").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let col = (idx % 3) as i32;
            let row = (idx / 3) as i32;
            let bounds = [40 + col * 360, 80 + row * 56, 320, 40];
            out.push((id, som_id, role, label, value, bounds));
        }
    }
    out
}

