// [LAYER: DOMAIN]
pub use crate::domain::types::computer_use_types::{
    IUiElement, IVirtualWindow, UiElement, VirtualWindow,
};
use crate::plumbing::display_telemetry::{
    apply_annotation_mutation, compute_action_ledger_health, compute_preview_delta,
    extract_dom_ui_tuples, query_action_ledger_dsl,
};
use crate::plumbing::vision_and_dom::{
    expression_targets_blocked_url, redact_cdp_url, snapshot_html_dom,
    validate_navigation_url,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::domain::computer_use::{el, VirtualDisplayState};

impl VirtualDisplayState {
    pub(crate) fn execute_drive_preview(&mut self, args: &Value) -> Result<Value, String> {
        let cmd = args
            .get("command")
            .or_else(|| args.get("sub_action"))
            .and_then(|v| v.as_str())
            .unwrap_or("elements")
            .to_ascii_lowercase();
        match cmd.as_str() {
            "elements" => {
                let full = args.get("full").and_then(|v| v.as_bool()).unwrap_or(false) || self.preview_inventory.is_empty();
                let win = self.windows.iter().find(|w| w.id == self.active_window_id).or_else(|| self.windows.first()).ok_or_else(|| "No window".to_string())?;
                let mut current = BTreeMap::new();
                for e in &win.elements {
                    current.insert(e.id.clone(), json!({"ref": e.id, "role": e.role, "label": e.label, "value": e.value, "focused": e.focused}));
                }
                Ok(compute_preview_delta(&current, &mut self.preview_inventory, full))
            }
            "click" => self.execute_action(&forward_cua_context(json!({"action": "click", "ref": args.get("ref").or_else(|| args.get("element_id"))}), args)),
            "fill" => self.execute_action(&forward_cua_context(json!({"action": "set_value", "ref": args.get("ref").or_else(|| args.get("element_id")), "value": args.get("value").or_else(|| args.get("text"))}), args)),
            _ => Ok(self.capture_frame()),
        }
    }

    pub(crate) fn execute_annotate_preview(&mut self, args: &Value) -> Result<Value, String> {
        self.assert_agent_may_act(args)?;
        let op = args
            .get("sub_action")
            .or_else(|| args.get("op"))
            .and_then(|v| v.as_str())
            .unwrap_or("add")
            .to_ascii_lowercase();
        let target_ref = args
            .get("ref")
            .or_else(|| args.get("element_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("btn-run");
        let note = args
            .get("note")
            .or_else(|| args.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        Ok(apply_annotation_mutation(
            &mut self.annotations,
            &op,
            target_ref,
            note,
        ))
    }



    pub fn execute_browser_action(&mut self, args: &Value) -> Result<Value, String> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("snapshot")
            .trim()
            .to_ascii_lowercase();
        match action.as_str() {
            "navigate" | "goto" | "open" => {
                self.assert_agent_may_act(args)?;
                let url = args.get("url").and_then(|v| v.as_str()).ok_or_else(|| "Missing 'url' for browser navigate".to_string())?;
                let validated = validate_navigation_url(url)?;
                self.save_undo_checkpoint();
                self.annotations.clear();
                self.preview_inventory.clear();
                let safe_u = redact_cdp_url(&validated);
                let escaped_u = safe_u.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;");
                let html = args.get("html").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| {
                    format!("<html><head><title>Page: {escaped_u}</title></head><body><main><h1>Navigated to {escaped_u}</h1><button id=\"nav-cta\" role=\"button\" aria-label=\"Continue\">Continue</button><input id=\"search-box\" type=\"search\" placeholder=\"Search page...\"/></main></body></html>")
                });
                let dom_snap = snapshot_html_dom(&html, &validated, 64);
                let sync_dom = args.get("sync_elements").and_then(|v| v.as_bool()).unwrap_or(false);
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == self.active_window_id) {
                    win.url = validated.clone();
                    win.html = html.clone();
                    if sync_dom || win.elements.is_empty() {
                        if let Some(t) = dom_snap.get("title").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                            win.title = t.to_string();
                        }
                        win.elements = extract_dom_ui_tuples(&dom_snap)
                            .into_iter()
                            .map(|(id, som_id, role, label, value, bounds)| el(&id, som_id, &role, &label, &value, bounds, false))
                            .collect();
                    }
                }
                self.frame_seq += 1;
                self.record_action("browser_navigate", "success", 4, json!({"url": safe_u}));
                Ok(dom_snap)
            }
            "snapshot" | "dom" => {
                let win = self.windows.iter().find(|w| w.id == self.active_window_id).or_else(|| self.windows.first());
                let url = args.get("url").and_then(|v| v.as_str()).or_else(|| win.map(|w| w.url.as_str())).unwrap_or("about:blank");
                let html = args.get("html").and_then(|v| v.as_str()).or_else(|| win.map(|w| w.html.as_str())).unwrap_or("<html></html>");
                let max_el = args.get("max_elements").and_then(|v| v.as_u64()).unwrap_or(64) as usize;
                Ok(snapshot_html_dom(html, url, max_el))
            }
            "click" => self.execute_action(&forward_cua_context(json!({ "action": "click", "element_id": args.get("selector").or_else(|| args.get("element_id")).and_then(|v| v.as_str()).unwrap_or(""), "som_id": args.get("som_id") }), args)),
            "type" | "fill" => self.execute_action(&forward_cua_context(json!({ "action": "set_value", "element_id": args.get("selector").or_else(|| args.get("element_id")).and_then(|v| v.as_str()).unwrap_or(""), "som_id": args.get("som_id"), "text": args.get("text").or_else(|| args.get("value")).and_then(|v| v.as_str()).unwrap_or("") }), args)),
            "dialog" => {
                self.assert_agent_may_act(args)?;
                let accept = args.get("accept").and_then(|v| v.as_bool()).unwrap_or(true);
                let prev = self.pending_dialog.take();
                self.record_action("browser_dialog", "success", 1, json!({"accepted": accept, "dialog": prev}));
                Ok(json!({ "handled": true, "accepted": accept, "previousDialog": prev }))
            }
            "eval" | "evaluate" => {
                self.assert_agent_may_act(args)?;
                let expr = args.get("expression").or_else(|| args.get("script")).and_then(|v| v.as_str()).unwrap_or("document.title");
                if let Some(blocked_reason) = expression_targets_blocked_url(expr) {
                    return Err(format!("Blocked unsafe URL literal in browser eval: {blocked_reason}"));
                }
                let win = self.windows.iter().find(|w| w.id == self.active_window_id);
                let result = match expr.trim() {
                    "document.title" => json!(win.map(|w| w.title.clone()).unwrap_or_default()),
                    "location.href" | "window.location.href" => json!(win.map(|w| redact_cdp_url(&w.url)).unwrap_or_default()),
                    _ => json!({"evaluated": expr, "status": "ok"}),
                };
                Ok(json!({ "expression": expr, "result": result }))
            }
            "validate_url" => {
                let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
                match validate_navigation_url(url) {
                    Ok(clean) => Ok(json!({ "valid": true, "url": redact_cdp_url(&clean) })),
                    Err(reason) => Ok(json!({ "valid": false, "reason": reason })),
                }
            }
            other => Err(format!("Unsupported browser_use action '{other}'.")),
        }
    }

    pub fn search_dsl(&self, query: &str) -> Value {
        query_action_ledger_dsl(&self.action_ledger, query)
    }

    pub extern "Rust" fn audit_health(&self) -> Value {
        compute_action_ledger_health(
            &self.action_ledger,
            self.undo_stack.len(),
            self.redo_stack.len(),
            &self.active_window_id,
            self.windows.len(),
            self.clipboard.len(),
            self.mouse_pressed,
            &self.lease_holder,
            self.lease_epoch,
        )
    }
}

pub(crate) fn forward_cua_context(mut base: Value, args: &Value) -> Value {
    if let Some(obj) = base.as_object_mut() {
        for key in ["actor", "expected_epoch", "delivery_mode"] {
            if let Some(v) = args.get(key) {
                obj.insert(key.to_string(), v.clone());
            }
        }
    }
    base
}
