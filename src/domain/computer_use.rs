// [LAYER: DOMAIN]
pub use crate::domain::types::computer_use_types::{IUiElement, IVirtualWindow, UiElement, VirtualWindow};
use crate::plumbing::display_telemetry::{apply_annotation_mutation, compute_action_ledger_health, compute_preview_delta, extract_dom_ui_tuples, query_action_ledger_dsl};
use crate::plumbing::hash_and_math::{fnv1a_32, now_ms};
use crate::plumbing::vision_and_dom::{expression_targets_blocked_url, extract_key_sequence, generate_synthetic_svg, is_blocked_key_combo, is_blocked_type_pattern, normalize_cua_action, parse_coordinate_value, parse_drag_points, parse_scroll_deltas, redact_cdp_url, snapshot_html_dom, validate_navigation_url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub type DisplayFrameSeq = u64;

/// Contractual sovereignty interface for virtual displays.
pub trait IVirtualDisplay {
    fn current_frame(&self) -> Value;
    fn display_resolution(&self) -> (u32, u32);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualDisplayState {
    pub width: u32,
    pub height: u32,
    pub cursor: [i32; 2],
    pub active_window_id: String,
    pub frame_seq: u64,
    pub windows: Vec<VirtualWindow>,
    pub action_ledger: Vec<Value>,
    pub undo_stack: Vec<String>,
    pub redo_stack: Vec<String>,
    pub pending_dialog: Option<String>,
    pub lease_holder: String,
    pub lease_viewer_id: Option<String>,
    pub lease_epoch: u64,
    pub last_capture_hash: String,
    pub dedup_streak: u8,
    pub last_target_app: String,
    pub preview_inventory: BTreeMap<String, Value>,
    pub annotations: Vec<Value>,
    pub clipboard: String,
    pub mouse_pressed: bool,
    #[serde(default)]
    pub scroll_offset: [i32; 2],
}

impl Default for VirtualDisplayState {
    fn default() -> Self { Self::new() }
}

fn el(id: &str, som_id: u32, role: &str, label: &str, value: &str, bounds: [i32; 4], focused: bool) -> UiElement {
    UiElement { id: id.into(), som_id, role: role.into(), label: label.into(), value: value.into(), bounds, enabled: true, focused }
}

impl VirtualDisplayState {
    pub fn new() -> Self {
        let default_html = "<html><head><title>HEAV3NS-MICRO Sovereign Workbench</title></head><body><button id=\"btn-run\" role=\"button\" aria-label=\"Run Flight Verification\">Run Flight</button><input id=\"inp-cmd\" type=\"text\" placeholder=\"Enter command or goal...\"/><a id=\"lnk-docs\" href=\"https://heav3ns.local/docs\">Architecture Docs</a><button id=\"btn-swarm\" role=\"button\" aria-label=\"Engage Legion Swarm\">Engage Swarm</button></body></html>".to_string();
        let main_win = VirtualWindow {
            id: "win-main-1".into(), title: "HEAV3NS-MICRO Sovereign Workbench".into(), app: "Heav3nsCode".into(),
            bounds: [0, 0, 1440, 900], focused: true, z_order: 1, url: "https://heav3ns.local/workbench".into(), html: default_html,
            elements: vec![
                el("btn-run", 1, "button", "Run Flight Verification", "", [40, 80, 200, 40], false), el("inp-cmd", 2, "textbox", "Command Input", "", [260, 80, 480, 40], true),
                el("lnk-docs", 3, "link", "Architecture Docs", "https://heav3ns.local/docs", [760, 80, 180, 40], false), el("btn-swarm", 4, "button", "Engage Legion Swarm", "", [960, 80, 200, 40], false),
            ],
        };
        let term_win = VirtualWindow {
            id: "win-term-2".into(), title: "Sovereign Terminal Multiplexer".into(), app: "Terminal".into(),
            bounds: [80, 480, 1280, 380], focused: false, z_order: 0, url: "about:blank".into(),
            html: "<html><head><title>Terminal</title></head><body><textarea id=\"term-stdin\" aria-label=\"Terminal Input\"></textarea></body></html>".into(),
            elements: vec![el("term-stdin", 5, "textbox", "Terminal Input", "$ cargo test --quiet", [100, 520, 1240, 320], false)],
        };
        Self {
            width: 1440, height: 900, cursor: [300, 100], active_window_id: "win-main-1".into(), frame_seq: 1,
            windows: vec![main_win, term_win], action_ledger: Vec::new(), undo_stack: Vec::new(), redo_stack: Vec::new(),
            pending_dialog: None, lease_holder: "agent".into(), lease_viewer_id: None, lease_epoch: 1,
            last_capture_hash: String::new(), dedup_streak: 0, last_target_app: "Heav3nsCode".into(),
            preview_inventory: BTreeMap::new(), annotations: Vec::new(), clipboard: String::new(), mouse_pressed: false,
            scroll_offset: [0, 0],
        }
    }

    fn assert_agent_may_act(&self, args: &Value) -> Result<(), String> {
        let actor = args.get("actor").and_then(|v| v.as_str()).unwrap_or("agent");
        if self.lease_holder == "human" && actor != "human" {
            return Err(format!("human_has_control: Human viewer {:?} holds desktop lease (epoch {})", self.lease_viewer_id, self.lease_epoch));
        }
        if let Some(exp) = args.get("expected_epoch").and_then(|v| v.as_u64()) {
            if exp != self.lease_epoch {
                return Err(format!("stale_lease_epoch: Expected epoch {} but current lease epoch is {}", exp, self.lease_epoch));
            }
        }
        Ok(())
    }

    fn save_undo_checkpoint(&mut self) {
        if let Ok(snap) = serde_json::to_string(&(&self.windows, &self.cursor, &self.active_window_id)) {
            if self.undo_stack.len() >= 32 { self.undo_stack.remove(0); }
            self.undo_stack.push(snap); self.redo_stack.clear();
        }
    }

    pub fn undo(&mut self) -> Result<Value, String> {
        let prev = self.undo_stack.pop().ok_or_else(|| "Undo stack is empty".to_string())?;
        if let Ok(cur) = serde_json::to_string(&(&self.windows, &self.cursor, &self.active_window_id)) { self.redo_stack.push(cur); }
        let (wins, cursor, active): (Vec<VirtualWindow>, [i32; 2], String) = serde_json::from_str(&prev).map_err(|e| format!("Undo error: {e}"))?;
        self.windows = wins; self.cursor = cursor; self.active_window_id = active; self.frame_seq += 1;
        self.record_action("undo", "success", 1, json!({"restoredFrame": self.frame_seq}));
        Ok(self.capture_frame())
    }

    pub fn redo(&mut self) -> Result<Value, String> {
        let next = self.redo_stack.pop().ok_or_else(|| "Redo stack is empty".to_string())?;
        if let Ok(cur) = serde_json::to_string(&(&self.windows, &self.cursor, &self.active_window_id)) { self.undo_stack.push(cur); }
        let (wins, cursor, active): (Vec<VirtualWindow>, [i32; 2], String) = serde_json::from_str(&next).map_err(|e| format!("Redo error: {e}"))?;
        self.windows = wins; self.cursor = cursor; self.active_window_id = active; self.frame_seq += 1;
        self.record_action("redo", "success", 1, json!({"restoredFrame": self.frame_seq}));
        Ok(self.capture_frame())
    }

    fn record_action(&mut self, action: &str, status: &str, duration_ms: u64, detail: Value) {
        let seq = self.action_ledger.len() + 1;
        self.action_ledger.push(json!({
            "seq": seq, "action": action, "window": self.active_window_id, "status": status,
            "durationMs": duration_ms, "timestampMs": now_ms(), "cursor": self.cursor, "detail": detail
        }));
    }

    pub fn capture_frame(&self) -> Value {
        self.render_capture("som", 1, false, None)
    }

    fn render_capture(&self, mode: &str, scale: u32, unchanged: bool, hash_override: Option<String>) -> Value {
        let active_win = self.windows.iter().find(|w| w.id == self.active_window_id).or_else(|| self.windows.first());
        let title = active_win.map(|w| w.title.as_str()).unwrap_or("Display");
        let svg = generate_synthetic_svg(self.width, self.height, title);
        let frame_hash = hash_override.unwrap_or_else(|| format!("{:08x}", fnv1a_32(svg.as_bytes())));
        let som_elements: Vec<Value> = if mode == "vision" {
            Vec::new()
        } else {
            active_win
                .map(|w| w.elements.iter().map(|el| el.to_som_json(self.frame_seq, true)).collect())
                .unwrap_or_default()
        };
        let max_x = active_win.and_then(|w| w.elements.iter().map(|e| e.bounds[0] + e.bounds[2]).max()).unwrap_or(0) as u32;
        let eff_scale = if scale > 1 { scale } else if max_x > self.width { 2 } else { 1 };
        json!({
            "mode": mode, "frameSeq": self.frame_seq, "frameHash": frame_hash, "screenUnchanged": unchanged,
            "resolution": [self.width, self.height], "boundsScale": eff_scale, "cursor": self.cursor,
            "scrollOffset": self.scroll_offset,
            "activeWindowId": self.active_window_id, "targetApp": self.last_target_app, "windowCount": self.windows.len(),
            "lease": {"holder": self.lease_holder, "epoch": self.lease_epoch},
            "setOfMarksCount": som_elements.len(), "setOfMarks": som_elements,
            "annotations": self.annotations,
            "svgPreview": if mode == "ax" { Value::Null } else { Value::String(svg) }
        })
    }

    fn resolve_token_or_som(&self, args: &Value) -> Result<Option<u32>, String> {
        if let Some(tok) = args.get("element_token").and_then(|v| v.as_str()) {
            let stripped = tok.strip_prefix('s').ok_or_else(|| format!("Invalid element_token '{tok}'"))?;
            let (hex_seq, idx_str) = stripped.split_once(':').ok_or_else(|| format!("Malformed element_token '{tok}'"))?;
            let idx = idx_str.parse::<u32>().map_err(|_| format!("Invalid index in element_token '{tok}'"))?;
            let cur_hex = format!("{:x}", self.frame_seq);
            if hex_seq != cur_hex && args.get("strict_token").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Err(format!("stale_element_token: Token '{tok}' belongs to frame s{hex_seq}, current is s{cur_hex}"));
            }
            return Ok(Some(idx));
        }
        Ok(args.get("som_id").or_else(|| args.get("mark")).and_then(|v| v.as_u64()).map(|n| n as u32))
    }

    fn validate_delivery_and_target(&mut self, args: &Value) -> Result<String, String> {
        let delivery = args.get("delivery_mode").and_then(|v| v.as_str()).unwrap_or("background").to_ascii_lowercase();
        let bring_front = args.get("bring_to_front").and_then(|v| v.as_bool()).unwrap_or(false);
        if bring_front && delivery != "foreground" {
            return Err("bring_to_front_requires_foreground: Set delivery_mode='foreground' when bring_to_front=true".to_string());
        }
        if let Some(req_app) = args.get("app").and_then(|v| v.as_str()) {
            if !req_app.is_empty() && !req_app.eq_ignore_ascii_case(&self.last_target_app) {
                return Err(format!("input_target_mismatch: Input targeted app '{}' but active target is '{}'", req_app, self.last_target_app));
            }
        }
        if delivery == "foreground" {
            let active_id = self.active_window_id.clone();
            for w in &mut self.windows {
                if w.id == active_id { w.focused = true; w.z_order += 1; }
            }
        }
        Ok(delivery)
    }

    pub fn execute_action(&mut self, args: &Value) -> Result<Value, String> {
        let raw_act = args.get("action").and_then(|v| v.as_str()).unwrap_or("capture").trim().to_ascii_lowercase();
        let btn = args.get("button").and_then(|v| v.as_str()).unwrap_or("");
        let count = args.get("click_count").and_then(|v| v.as_u64()).unwrap_or(1);
        let action = if btn == "right" { "right_click".to_string() }
            else if btn == "middle" { "middle_click".to_string() }
            else if count == 2 { "double_click".to_string() }
            else if count >= 3 { "triple_click".to_string() }
            else { normalize_cua_action(&raw_act).to_string() };
        let norm_1000 = args.get("coord_space").and_then(|v| v.as_str()) == Some("normalized_1000");
        match action.as_str() {
            "lease_acquire" | "lease_release" | "lease_status" => {
                if action == "lease_acquire" {
                    self.lease_holder = args.get("holder").and_then(|v| v.as_str()).unwrap_or("human").to_string();
                    self.lease_viewer_id = args.get("viewer_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                    self.lease_epoch += 1;
                } else if action == "lease_release" {
                    self.lease_holder = "agent".into();
                    self.lease_viewer_id = None;
                    self.lease_epoch += 1;
                }
                Ok(json!({ "holder": self.lease_holder, "viewerId": self.lease_viewer_id, "epoch": self.lease_epoch }))
            }
            "capture" | "capture_frame" | "screenshot" | "snapshot" => {
                let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("som");
                let scale = args.get("scale").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let state_sig = serde_json::to_string(&(&self.windows, &self.active_window_id, mode)).unwrap_or_default();
                let cur_hash = format!("{:08x}", fnv1a_32(state_sig.as_bytes()));
                let unchanged = if !self.last_capture_hash.is_empty() && self.last_capture_hash == cur_hash && self.dedup_streak < 2 {
                    self.dedup_streak += 1;
                    true
                } else {
                    self.last_capture_hash = cur_hash.clone();
                    self.dedup_streak = 0;
                    false
                };
                self.record_action("capture", "success", 1, json!({"mode": mode, "screenUnchanged": unchanged}));
                Ok(self.render_capture(mode, scale, unchanged, Some(cur_hash)))
            }
            "list_windows" | "windows" => {
                let list: Vec<Value> = self.windows.iter().map(|w| json!({
                    "id": w.id, "title": w.title, "app": w.app, "bounds": w.bounds,
                    "focused": w.focused, "elementCount": w.elements.len(), "url": redact_cdp_url(&w.url)
                })).collect();
                Ok(json!({ "activeWindowId": self.active_window_id, "targetApp": self.last_target_app, "windows": list }))
            }
            "list_apps" | "apps" => {
                let apps: Vec<Value> = self.windows.iter().map(|w| json!({"app": w.app, "windowId": w.id, "focused": w.focused})).collect();
                Ok(json!({ "activeApp": self.last_target_app, "apps": apps }))
            }
            "focus_app" => {
                self.assert_agent_may_act(args)?;
                let app_name = args.get("app").or_else(|| args.get("target")).and_then(|v| v.as_str()).ok_or_else(|| "Missing 'app' for focus_app".to_string())?;
                let win_id = self.windows.iter().find(|w| w.app.eq_ignore_ascii_case(app_name)).map(|w| w.id.clone()).ok_or_else(|| format!("App '{app_name}' not found"))?;
                self.execute_action(&json!({"action": "focus_window", "window_id": win_id}))
            }
            "read_window_below" => {
                let win_id = args.get("window_id").and_then(|v| v.as_str()).unwrap_or(&self.active_window_id);
                let win = self.windows.iter().find(|w| w.id == win_id).or_else(|| self.windows.first()).ok_or_else(|| "No window available".to_string())?;
                Ok(json!({
                    "windowId": win.id, "title": win.title, "app": win.app, "bounds": win.bounds,
                    "zOrder": win.z_order, "focused": win.focused, "url": redact_cdp_url(&win.url),
                    "elementCount": win.elements.len(), "leaseHolder": self.lease_holder, "leaseEpoch": self.lease_epoch
                }))
            }
            "list_elements" | "elements" | "som" => {
                let win_id = args.get("window_id").and_then(|v| v.as_str()).unwrap_or(&self.active_window_id);
                let win = self.windows.iter().find(|w| w.id == win_id).ok_or_else(|| format!("Window '{win_id}' not found"))?;
                Ok(json!({ "windowId": win.id, "title": win.title, "elements": win.elements }))
            }
            "drive_preview" => self.execute_drive_preview(args),
            "annotate_preview" | "annotate" => self.execute_annotate_preview(args),
            "focus_window" | "focus" => {
                self.assert_agent_may_act(args)?;
                let target_id = args.get("window_id").or_else(|| args.get("id")).or_else(|| args.get("target"))
                    .and_then(|v| v.as_str()).ok_or_else(|| "Missing 'window_id' for focus_window".to_string())?;
                let target_app = self.windows.iter().find(|w| w.id == target_id).map(|w| w.app.clone()).ok_or_else(|| format!("Window '{target_id}' not found"))?;
                self.save_undo_checkpoint();
                for w in &mut self.windows {
                    w.focused = w.id == target_id;
                    if w.focused { w.z_order += 1; }
                }
                self.active_window_id = target_id.to_string();
                self.last_target_app = target_app;
                self.frame_seq += 1;
                self.record_action("focus_window", "success", 2, json!({"windowId": target_id, "app": self.last_target_app}));
                Ok(self.capture_frame())
            }
            "click" | "double_click" | "triple_click" | "right_click" | "middle_click" => {
                self.assert_agent_may_act(args)?;
                let delivery = self.validate_delivery_and_target(args)?;
                let som_id = self.resolve_token_or_som(args)?;
                let target_elem = args.get("element_id").or_else(|| args.get("ref")).or_else(|| args.get("target")).and_then(|v| v.as_str());
                self.save_undo_checkpoint();
                let mut resolved_id = String::new();
                let mut disabled_id = String::new();
                let raw_x = parse_coordinate_value(args.get("x"), self.width, self.cursor[0], norm_1000);
                let raw_y = parse_coordinate_value(args.get("y"), self.height, self.cursor[1], norm_1000);
                let mut click_xy = [raw_x, raw_y];
                let active_id = self.active_window_id.clone();
                for win in &mut self.windows {
                    let is_active = win.id == active_id;
                    if !is_active && (!win.is_modal_window() || !resolved_id.is_empty() || !disabled_id.is_empty()) {
                        continue;
                    }
                    for el in &mut win.elements {
                        if el.matches_selector(target_elem, som_id, Some(click_xy)) {
                            let (pt, ok) = el.apply_click_interaction();
                            click_xy = pt;
                            if ok { resolved_id = el.id.clone(); } else { disabled_id = el.id.clone(); }
                        } else if is_active {
                            el.focused = false;
                        }
                    }
                }
                self.cursor = [click_xy[0].clamp(0, self.width as i32), click_xy[1].clamp(0, self.height as i32)];
                self.frame_seq += 1;
                let verdict = if !disabled_id.is_empty() {
                    json!({"decision": "abort", "effect": "target_disabled", "recommended": "enable_prerequisite_control"})
                } else if !resolved_id.is_empty() {
                    json!({"decision": "done", "effect": "confirmed", "recommended": Value::Null})
                } else {
                    json!({"decision": "escalate", "effect": "suspected_noop", "recommended": if delivery == "background" { "foreground" } else { "px" }})
                };
                self.record_action(&action, "success", 3, json!({"cursor": self.cursor, "elementId": resolved_id, "disabledElementId": disabled_id, "deliveryMode": delivery, "verdict": verdict}));
                Ok(json!({ "action": action, "deliveryMode": delivery, "clickedAt": self.cursor, "resolvedElementId": resolved_id, "targetDisabled": !disabled_id.is_empty(), "verdict": verdict, "frame": self.capture_frame() }))
            }
            "type" | "set_value" | "fill" => {
                self.assert_agent_may_act(args)?;
                let delivery = self.validate_delivery_and_target(args)?;
                let vault_key = args.get("vault_key").and_then(|v| v.as_str());
                let text = if let Some(vk) = vault_key {
                    format!("[VAULT_SECRET:{vk}]")
                } else {
                    args.get("text").or_else(|| args.get("value")).and_then(|v| v.as_str()).unwrap_or("").to_string()
                };
                if is_blocked_type_pattern(&text) {
                    return Err("blocked_type_pattern: Refusing to type destructive shell payload into GUI target".to_string());
                }
                let som_id = self.resolve_token_or_som(args)?;
                let target_elem = args.get("element_id").or_else(|| args.get("ref")).or_else(|| args.get("target")).and_then(|v| v.as_str());
                self.save_undo_checkpoint();
                let mut updated_id = String::new();
                let mut disabled_id = String::new();
                let active_id = self.active_window_id.clone();
                let overwrite = action == "set_value" || action == "fill" || vault_key.is_some();
                for win in &mut self.windows {
                    let is_active = win.id == active_id;
                    if !is_active && (!win.is_modal_window() || !updated_id.is_empty() || !disabled_id.is_empty()) {
                        continue;
                    }
                    if let Some(el) = win.elements.iter_mut().find(|e| {
                        e.matches_selector(target_elem, som_id, None)
                            || (is_active && target_elem.is_none() && som_id.is_none() && e.focused)
                    }) {
                        if el.apply_text_mutation(&text, overwrite) {
                            updated_id = el.id.clone();
                        } else {
                            disabled_id = el.id.clone();
                        }
                    }
                }
                self.frame_seq += 1;
                let verdict = if !disabled_id.is_empty() {
                    json!({"decision": "abort", "effect": "target_disabled", "recommended": "enable_prerequisite_control"})
                } else if !updated_id.is_empty() {
                    json!({"decision": "done", "effect": "confirmed", "recommended": Value::Null})
                } else {
                    json!({"decision": "escalate", "effect": "suspected_noop", "recommended": "set_value"})
                };
                self.record_action(&action, "success", 2, json!({"elementId": updated_id, "disabledElementId": disabled_id, "vaultRedacted": vault_key.is_some(), "deliveryMode": delivery, "verdict": verdict}));
                Ok(json!({ "action": action, "deliveryMode": delivery, "elementId": updated_id, "targetDisabled": !disabled_id.is_empty(), "newValue": text, "vaultRedacted": vault_key.is_some(), "verdict": verdict, "frame": self.capture_frame() }))
            }
            "move" => {
                self.assert_agent_may_act(args)?;
                let rx = parse_coordinate_value(args.get("x"), self.width, self.cursor[0], norm_1000);
                let ry = parse_coordinate_value(args.get("y"), self.height, self.cursor[1], norm_1000);
                self.cursor = [rx.clamp(0, self.width as i32), ry.clamp(0, self.height as i32)];
                Ok(json!({ "action": "move", "cursor": self.cursor, "frameSeq": self.frame_seq }))
            }
            "hover" => {
                self.assert_agent_may_act(args)?;
                let hx = parse_coordinate_value(args.get("x"), self.width, self.cursor[0], norm_1000);
                let hy = parse_coordinate_value(args.get("y"), self.height, self.cursor[1], norm_1000);
                self.cursor = [hx.clamp(0, self.width as i32), hy.clamp(0, self.height as i32)];
                let hovered_el = self.windows.iter().find(|w| w.id == self.active_window_id)
                    .and_then(|w| w.elements.iter().find(|e| self.cursor[0] >= e.bounds[0] && self.cursor[0] <= e.bounds[0] + e.bounds[2] && self.cursor[1] >= e.bounds[1] && self.cursor[1] <= e.bounds[1] + e.bounds[3]))
                    .map(|e| json!({"id": e.id, "role": e.role, "label": e.label}));
                self.record_action("hover", "success", 1, json!({"cursor": self.cursor, "hovered": hovered_el}));
                Ok(json!({ "action": "hover", "cursor": self.cursor, "hovered": hovered_el, "frameSeq": self.frame_seq }))
            }
            "mouse_down" => {
                self.assert_agent_may_act(args)?;
                self.mouse_pressed = true;
                self.record_action("mouse_down", "success", 1, json!({"cursor": self.cursor}));
                Ok(json!({ "action": "mouse_down", "cursor": self.cursor, "mousePressed": true }))
            }
            "mouse_up" => {
                self.assert_agent_may_act(args)?;
                self.mouse_pressed = false;
                self.record_action("mouse_up", "success", 1, json!({"cursor": self.cursor}));
                Ok(json!({ "action": "mouse_up", "cursor": self.cursor, "mousePressed": false }))
            }
            "key" | "hotkey" => {
                self.assert_agent_may_act(args)?;
                let delivery = self.validate_delivery_and_target(args)?;
                let keys = extract_key_sequence(args);
                if is_blocked_key_combo(&keys) {
                    return Err(format!("blocked_key_combo: Refusing destructive or session-locking shortcut '{keys}'"));
                }
                let k_norm = keys.trim().to_ascii_lowercase();
                if k_norm == "escape" {
                    self.pending_dialog = None;
                    if let Some(pos) = self.windows.iter().position(|w| w.id != "win-main-1" && w.is_modal_window()) {
                        self.windows.remove(pos);
                        if let Some(main) = self.windows.iter_mut().find(|w| w.id == "win-main-1") { main.focused = true; self.active_window_id = "win-main-1".into(); }
                    } else if let Some(win) = self.windows.iter_mut().find(|w| w.id == self.active_window_id) {
                        win.elements.retain(|e| !matches!(e.role.to_ascii_lowercase().as_str(), "dialog" | "alertdialog" | "modal"));
                        for e in &mut win.elements { e.focused = false; }
                    }
                } else if let Some(win) = self.windows.iter_mut().find(|w| w.id == self.active_window_id) {
                    win.apply_window_key(&k_norm, &mut self.clipboard, &mut self.cursor);
                }
                self.frame_seq += 1;
                let verdict = json!({"decision": "verify_fresh_state", "effect": "unverifiable", "recommended": Value::Null});
                self.record_action("key", "success", 1, json!({"keys": keys, "deliveryMode": delivery}));
                Ok(json!({ "action": "key", "keys": keys, "deliveryMode": delivery, "verdict": verdict, "frame": self.capture_frame() }))
            }
            "clipboard_write" | "copy_to_clipboard" => {
                let text = args.get("text").or_else(|| args.get("value")).and_then(|v| v.as_str()).unwrap_or("");
                self.clipboard = text.to_string();
                self.record_action("clipboard_write", "success", 1, json!({"length": self.clipboard.len()}));
                Ok(json!({ "action": "clipboard_write", "length": self.clipboard.len(), "clipboard": self.clipboard }))
            }
            "clipboard_read" | "get_clipboard" => {
                self.record_action("clipboard_read", "success", 1, json!({"length": self.clipboard.len()}));
                Ok(json!({ "action": "clipboard_read", "length": self.clipboard.len(), "clipboard": self.clipboard }))
            }
            "clipboard_paste" | "paste" => {
                self.assert_agent_may_act(args)?;
                self.save_undo_checkpoint();
                let clip_text = self.clipboard.clone();
                let mut updated_id = String::new();
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == self.active_window_id) {
                    if let Some(el) = win.elements.iter_mut().find(|e| e.focused) {
                        el.value.push_str(&clip_text);
                        updated_id = el.id.clone();
                    }
                }
                self.frame_seq += 1;
                let verdict = if !updated_id.is_empty() {
                    json!({"decision": "done", "effect": "confirmed", "recommended": Value::Null})
                } else {
                    json!({"decision": "escalate", "effect": "no_focused_element", "recommended": "focus_or_click_first"})
                };
                self.record_action("clipboard_paste", "success", 2, json!({"pastedLength": clip_text.len(), "elementId": updated_id, "verdict": verdict}));
                Ok(json!({ "action": "clipboard_paste", "pastedLength": clip_text.len(), "elementId": updated_id, "verdict": verdict, "frame": self.capture_frame() }))
            }
            "find_elements" | "ax_query" | "query_elements" => {
                let win_id = args.get("window_id").and_then(|v| v.as_str()).unwrap_or(&self.active_window_id);
                let win = self.windows.iter().find(|w| w.id == win_id).or_else(|| self.windows.first()).ok_or_else(|| "No window available".to_string())?;
                let matched = win.query_window_elements(self.frame_seq, args);
                Ok(json!({ "windowId": win.id, "queryCount": matched.len(), "elements": matched }))
            }
            "wait" => {
                let ms = args.get("duration_ms").or_else(|| args.get("ms")).and_then(|v| v.as_u64()).unwrap_or(100).min(5000);
                self.record_action("wait", "success", ms, json!({"waitedMs": ms}));
                Ok(json!({ "action": "wait", "waitedMs": ms, "frameSeq": self.frame_seq }))
            }
            "drag" => {
                self.assert_agent_may_act(args)?;
                self.save_undo_checkpoint();
                let (from_pt, to_pt) = parse_drag_points(args, self.width, self.height, self.cursor, norm_1000);
                self.cursor = to_pt;
                self.frame_seq += 1;
                self.record_action("drag", "success", 4, json!({"from": from_pt, "to": self.cursor}));
                Ok(json!({ "action": "drag", "from": from_pt, "to": self.cursor, "frame": self.capture_frame() }))
            }
            "scroll" => {
                self.assert_agent_may_act(args)?;
                if args.get("x").is_some() || args.get("y").is_some() {
                    let sx = parse_coordinate_value(args.get("x"), self.width, self.cursor[0], norm_1000);
                    let sy = parse_coordinate_value(args.get("y"), self.height, self.cursor[1], norm_1000);
                    self.cursor = [sx.clamp(0, self.width as i32), sy.clamp(0, self.height as i32)];
                }
                let (delta_x, delta_y) = parse_scroll_deltas(args);
                self.scroll_offset[0] += delta_x;
                self.scroll_offset[1] += delta_y;
                self.frame_seq += 1;
                self.record_action("scroll", "success", 2, json!({"delta": [delta_x, delta_y], "scrollOffset": self.scroll_offset, "cursor": self.cursor}));
                Ok(json!({ "action": "scroll", "delta": [delta_x, delta_y], "scrollOffset": self.scroll_offset, "cursor": self.cursor, "frameSeq": self.frame_seq }))
            }
            "window_minimize" | "minimize_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args.get("window_id").and_then(|v| v.as_str()).unwrap_or(&self.active_window_id).to_string();
                let mut switched = false;
                for w in &mut self.windows {
                    if w.id == win_id {
                        w.focused = false;
                        w.z_order = 0;
                        switched = true;
                    }
                }
                if switched {
                    if let Some(other) = self.windows.iter_mut().find(|w| w.id != win_id) {
                        other.focused = true;
                        other.z_order += 1;
                        self.active_window_id = other.id.clone();
                        self.last_target_app = other.app.clone();
                    }
                }
                self.record_action("window_minimize", "success", 1, json!({"windowId": win_id, "active": self.active_window_id}));
                Ok(self.capture_frame())
            }
            "window_maximize" | "maximize_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args.get("window_id").and_then(|v| v.as_str()).unwrap_or(&self.active_window_id);
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == win_id) { win.bounds = [0, 0, self.width as i32, self.height as i32]; }
                self.record_action("window_maximize", "success", 1, json!({"windowId": win_id}));
                Ok(self.capture_frame())
            }
            "window_restore" | "restore_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args.get("window_id").and_then(|v| v.as_str()).unwrap_or(&self.active_window_id);
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == win_id) { win.bounds = [120, 120, 1024, 640]; win.focused = true; }
                self.record_action("window_restore", "success", 1, json!({"windowId": win_id}));
                Ok(self.capture_frame())
            }
            "window_resize" | "resize_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args.get("window_id").and_then(|v| v.as_str()).unwrap_or(&self.active_window_id);
                let nw = args.get("width").and_then(|v| v.as_i64()).unwrap_or(800) as i32;
                let nh = args.get("height").and_then(|v| v.as_i64()).unwrap_or(600) as i32;
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == win_id) { win.bounds[2] = nw; win.bounds[3] = nh; }
                self.record_action("window_resize", "success", 1, json!({"windowId": win_id, "bounds": [nw, nh]}));
                Ok(self.capture_frame())
            }
            "window_move" | "move_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args.get("window_id").and_then(|v| v.as_str()).unwrap_or(&self.active_window_id);
                let nx = args.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let ny = args.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == win_id) { win.bounds[0] = nx; win.bounds[1] = ny; }
                self.record_action("window_move", "success", 1, json!({"windowId": win_id, "position": [nx, ny]}));
                Ok(self.capture_frame())
            }
            "window_close" | "close_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args.get("window_id").and_then(|v| v.as_str()).unwrap_or(&self.active_window_id).to_string();
                self.windows.retain(|w| w.id != win_id);
                if self.active_window_id == win_id {
                    if let Some(first) = self.windows.first_mut() {
                        first.focused = true;
                        self.active_window_id = first.id.clone();
                        self.last_target_app = first.app.clone();
                    } else {
                        self.active_window_id.clear();
                        self.last_target_app.clear();
                    }
                }
                self.record_action("window_close", "success", 1, json!({"closedId": win_id, "active": self.active_window_id}));
                Ok(self.capture_frame())
            }
            "register_window" => {
                self.save_undo_checkpoint();
                let id = args.get("window_id").or_else(|| args.get("id")).and_then(|v| v.as_str()).unwrap_or("win-custom").to_string();
                let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("Custom Window").to_string();
                let app = args.get("app").and_then(|v| v.as_str()).unwrap_or("CustomApp").to_string();
                for w in &mut self.windows { w.focused = false; }
                self.windows.push(VirtualWindow {
                    id: id.clone(), title, app: app.clone(), bounds: [120, 120, 1024, 640], focused: true,
                    z_order: (self.windows.len() + 1) as u32, url: "about:blank".into(),
                    html: "<html><body></body></html>".into(), elements: Vec::new(),
                });
                self.active_window_id = id.clone();
                self.last_target_app = app;
                self.frame_seq += 1;
                self.record_action("register_window", "success", 2, json!({"windowId": id}));
                Ok(self.capture_frame())
            }
            "search_dsl" | "query" => Ok(self.search_dsl(args.get("query").and_then(|v| v.as_str()).unwrap_or(""))),
            "audit_health" | "health" | "metrics" => Ok(self.audit_health()),
            "undo" => self.undo(),
            "redo" => self.redo(),
            other => Err(format!("Unsupported computer_use action '{other}'.")),
        }
    }

    fn execute_drive_preview(&mut self, args: &Value) -> Result<Value, String> {
        let cmd = args.get("command").or_else(|| args.get("sub_action")).and_then(|v| v.as_str()).unwrap_or("elements").to_ascii_lowercase();
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
            "click" => self.execute_action(&json!({"action": "click", "ref": args.get("ref").or_else(|| args.get("element_id"))})),
            "fill" => self.execute_action(&json!({"action": "set_value", "ref": args.get("ref").or_else(|| args.get("element_id")), "value": args.get("value").or_else(|| args.get("text"))})),
            _ => Ok(self.capture_frame()),
        }
    }

    fn execute_annotate_preview(&mut self, args: &Value) -> Result<Value, String> {
        let op = args.get("sub_action").or_else(|| args.get("op")).and_then(|v| v.as_str()).unwrap_or("add").to_ascii_lowercase();
        let target_ref = args.get("ref").or_else(|| args.get("element_id")).and_then(|v| v.as_str()).unwrap_or("btn-run");
        let note = args.get("note").or_else(|| args.get("text")).and_then(|v| v.as_str()).unwrap_or("");
        Ok(apply_annotation_mutation(&mut self.annotations, &op, target_ref, note))
    }

    pub fn execute_browser_action(&mut self, args: &Value) -> Result<Value, String> {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("snapshot").trim().to_ascii_lowercase();
        match action.as_str() {
            "navigate" | "goto" | "open" => {
                let url = args.get("url").and_then(|v| v.as_str()).ok_or_else(|| "Missing 'url' for browser navigate".to_string())?;
                let validated = validate_navigation_url(url)?;
                self.save_undo_checkpoint();
                self.annotations.clear();
                self.preview_inventory.clear();
                let safe_u = redact_cdp_url(&validated);
                let html = args.get("html").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| {
                    format!("<html><head><title>Page: {safe_u}</title></head><body><main><h1>Navigated to {safe_u}</h1><button id=\"nav-cta\" role=\"button\" aria-label=\"Continue\">Continue</button><input id=\"search-box\" type=\"search\" placeholder=\"Search page...\"/></main></body></html>")
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
            "click" => self.execute_action(&json!({ "action": "click", "element_id": args.get("selector").or_else(|| args.get("element_id")).and_then(|v| v.as_str()).unwrap_or(""), "som_id": args.get("som_id") })),
            "type" | "fill" => self.execute_action(&json!({ "action": "set_value", "element_id": args.get("selector").or_else(|| args.get("element_id")).and_then(|v| v.as_str()).unwrap_or(""), "som_id": args.get("som_id"), "text": args.get("text").or_else(|| args.get("value")).and_then(|v| v.as_str()).unwrap_or("") })),
            "dialog" => {
                let accept = args.get("accept").and_then(|v| v.as_bool()).unwrap_or(true);
                let prev = self.pending_dialog.take();
                self.record_action("browser_dialog", "success", 1, json!({"accepted": accept, "dialog": prev}));
                Ok(json!({ "handled": true, "accepted": accept, "previousDialog": prev }))
            }
            "eval" | "evaluate" => {
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
