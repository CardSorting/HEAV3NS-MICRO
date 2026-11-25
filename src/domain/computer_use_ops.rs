// [LAYER: DOMAIN]
pub use crate::domain::types::computer_use_types::{
    IUiElement, IVirtualWindow, UiElement, VirtualWindow,
};
use crate::plumbing::hash_and_math::fnv1a_32;
use crate::plumbing::vision_and_dom::{
    extract_key_sequence,
    is_blocked_key_combo, is_blocked_type_pattern, normalize_cua_action, parse_coordinate_value,
    parse_drag_points, parse_scroll_deltas, redact_cdp_url,
};
use serde_json::{json, Value};

use crate::domain::computer_use::VirtualDisplayState;
use crate::domain::computer_use_browser::forward_cua_context;


impl VirtualDisplayState {
    pub(crate) fn execute_extended_action(&mut self, action: &str, args: &Value, norm_1000: bool) -> Result<Value, String> {
        match action {
            "type" | "set_value" | "fill" => {
                self.assert_agent_may_act(args)?;
                let delivery = self.validate_delivery_and_target(args)?;
                let vault_key = args.get("vault_key").and_then(|v| v.as_str());
                let text = if let Some(vk) = vault_key {
                    format!("[VAULT_SECRET:{vk}]")
                } else {
                    args.get("text")
                        .or_else(|| args.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };
                if is_blocked_type_pattern(&text) {
                    return Err("blocked_type_pattern: Refusing to type destructive shell payload into GUI target".to_string());
                }
                let som_id = self.resolve_token_or_som(args)?;
                let target_elem = args
                    .get("element_id")
                    .or_else(|| args.get("ref"))
                    .or_else(|| args.get("target"))
                    .and_then(|v| v.as_str());
                self.save_undo_checkpoint();
                let mut updated_id = String::new();
                let mut disabled_id = String::new();
                let active_id = self.active_window_id.clone();
                let overwrite = action == "set_value" || action == "fill" || vault_key.is_some();
                for win in &mut self.windows {
                    let is_active = win.id == active_id;
                    if !is_active
                        && (!win.is_modal_window()
                            || !updated_id.is_empty()
                            || !disabled_id.is_empty())
                    {
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
                Ok(
                    json!({ "action": action, "deliveryMode": delivery, "elementId": updated_id, "targetDisabled": !disabled_id.is_empty(), "newValue": text, "vaultRedacted": vault_key.is_some(), "verdict": verdict, "frame": self.capture_frame() }),
                )
            }
            "move" => {
                self.assert_agent_may_act(args)?;
                let rx =
                    parse_coordinate_value(args.get("x"), self.width, self.cursor[0], norm_1000);
                let ry =
                    parse_coordinate_value(args.get("y"), self.height, self.cursor[1], norm_1000);
                self.cursor = [
                    rx.clamp(0, self.width as i32),
                    ry.clamp(0, self.height as i32),
                ];
                Ok(json!({ "action": "move", "cursor": self.cursor, "frameSeq": self.frame_seq }))
            }
            "hover" => {
                self.assert_agent_may_act(args)?;
                let hx =
                    parse_coordinate_value(args.get("x"), self.width, self.cursor[0], norm_1000);
                let hy =
                    parse_coordinate_value(args.get("y"), self.height, self.cursor[1], norm_1000);
                self.cursor = [
                    hx.clamp(0, self.width as i32),
                    hy.clamp(0, self.height as i32),
                ];
                let hovered_el = self
                    .windows
                    .iter()
                    .find(|w| w.id == self.active_window_id)
                    .and_then(|w| {
                        w.elements.iter().find(|e| {
                            self.cursor[0] >= e.bounds[0]
                                && self.cursor[0] <= e.bounds[0].saturating_add(e.bounds[2])
                                && self.cursor[1] >= e.bounds[1]
                                && self.cursor[1] <= e.bounds[1].saturating_add(e.bounds[3])
                        })
                    })
                    .map(|e| json!({"id": e.id, "role": e.role, "label": e.label}));
                self.record_action(
                    "hover",
                    "success",
                    1,
                    json!({"cursor": self.cursor, "hovered": hovered_el}),
                );
                Ok(
                    json!({ "action": "hover", "cursor": self.cursor, "hovered": hovered_el, "frameSeq": self.frame_seq }),
                )
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
                    if let Some(pos) = self
                        .windows
                        .iter()
                        .position(|w| w.id != "win-main-1" && w.is_modal_window())
                    {
                        let removed_id = self.windows.remove(pos).id;
                        if let Some(main) = self.windows.iter_mut().find(|w| w.id == "win-main-1") {
                            main.focused = true;
                            self.active_window_id = "win-main-1".into();
                            self.last_target_app = main.app.clone();
                        } else if self.active_window_id == removed_id {
                            if let Some(first) = self.windows.first_mut() {
                                first.focused = true;
                                self.active_window_id = first.id.clone();
                                self.last_target_app = first.app.clone();
                            } else {
                                self.active_window_id.clear();
                                self.last_target_app.clear();
                            }
                        }
                    } else if let Some(win) = self
                        .windows
                        .iter_mut()
                        .find(|w| w.id == self.active_window_id)
                    {
                        win.elements.retain(|e| {
                            !matches!(
                                e.role.to_ascii_lowercase().as_str(),
                                "dialog" | "alertdialog" | "modal"
                            )
                        });
                        for e in &mut win.elements {
                            e.focused = false;
                        }
                    }
                } else if let Some(win) = self
                    .windows
                    .iter_mut()
                    .find(|w| w.id == self.active_window_id)
                {
                    win.apply_window_key(&k_norm, &mut self.clipboard, &mut self.cursor);
                }
                self.frame_seq += 1;
                let verdict = json!({"decision": "verify_fresh_state", "effect": "unverifiable", "recommended": Value::Null});
                self.record_action(
                    "key",
                    "success",
                    1,
                    json!({"keys": keys, "deliveryMode": delivery}),
                );
                Ok(
                    json!({ "action": "key", "keys": keys, "deliveryMode": delivery, "verdict": verdict, "frame": self.capture_frame() }),
                )
            }
            "clipboard_write" | "copy_to_clipboard" => {
                self.assert_agent_may_act(args)?;
                let text = args
                    .get("text")
                    .or_else(|| args.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                self.clipboard = text.to_string();
                self.record_action(
                    "clipboard_write",
                    "success",
                    1,
                    json!({"length": self.clipboard.len()}),
                );
                Ok(
                    json!({ "action": "clipboard_write", "length": self.clipboard.len(), "clipboard": self.clipboard }),
                )
            }
            "clipboard_read" | "get_clipboard" => {
                self.record_action(
                    "clipboard_read",
                    "success",
                    1,
                    json!({"length": self.clipboard.len()}),
                );
                Ok(
                    json!({ "action": "clipboard_read", "length": self.clipboard.len(), "clipboard": self.clipboard }),
                )
            }
            "clipboard_paste" | "paste" => {
                self.assert_agent_may_act(args)?;
                self.save_undo_checkpoint();
                let clip_text = self.clipboard.clone();
                let mut updated_id = String::new();
                if let Some(win) = self
                    .windows
                    .iter_mut()
                    .find(|w| w.id == self.active_window_id)
                {
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
                Ok(
                    json!({ "action": "clipboard_paste", "pastedLength": clip_text.len(), "elementId": updated_id, "verdict": verdict, "frame": self.capture_frame() }),
                )
            }
            "find_elements" | "ax_query" | "query_elements" => {
                let win_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.active_window_id);
                let win = self
                    .windows
                    .iter()
                    .find(|w| w.id == win_id)
                    .or_else(|| self.windows.first())
                    .ok_or_else(|| "No window available".to_string())?;
                let matched = win.query_window_elements(self.frame_seq, args);
                Ok(json!({ "windowId": win.id, "queryCount": matched.len(), "elements": matched }))
            }
            "wait" => {
                let ms = args
                    .get("duration_ms")
                    .or_else(|| args.get("ms"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100)
                    .min(5000);
                self.record_action("wait", "success", ms, json!({"waitedMs": ms}));
                Ok(json!({ "action": "wait", "waitedMs": ms, "frameSeq": self.frame_seq }))
            }
            "drag" => {
                self.assert_agent_may_act(args)?;
                self.save_undo_checkpoint();
                let (from_pt, to_pt) =
                    parse_drag_points(args, self.width, self.height, self.cursor, norm_1000);
                self.cursor = to_pt;
                self.frame_seq += 1;
                self.record_action(
                    "drag",
                    "success",
                    4,
                    json!({"from": from_pt, "to": self.cursor}),
                );
                Ok(
                    json!({ "action": "drag", "from": from_pt, "to": self.cursor, "frame": self.capture_frame() }),
                )
            }
            "scroll" => {
                self.assert_agent_may_act(args)?;
                if args.get("x").is_some() || args.get("y").is_some() {
                    let sx = parse_coordinate_value(
                        args.get("x"),
                        self.width,
                        self.cursor[0],
                        norm_1000,
                    );
                    let sy = parse_coordinate_value(
                        args.get("y"),
                        self.height,
                        self.cursor[1],
                        norm_1000,
                    );
                    self.cursor = [
                        sx.clamp(0, self.width as i32),
                        sy.clamp(0, self.height as i32),
                    ];
                }
                let (delta_x, delta_y) = parse_scroll_deltas(args);
                self.scroll_offset[0] = self.scroll_offset[0].saturating_add(delta_x);
                self.scroll_offset[1] = self.scroll_offset[1].saturating_add(delta_y);
                self.frame_seq += 1;
                self.record_action("scroll", "success", 2, json!({"delta": [delta_x, delta_y], "scrollOffset": self.scroll_offset, "cursor": self.cursor}));
                Ok(
                    json!({ "action": "scroll", "delta": [delta_x, delta_y], "scrollOffset": self.scroll_offset, "cursor": self.cursor, "frameSeq": self.frame_seq }),
                )
            }
            "window_minimize" | "minimize_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.active_window_id)
                    .to_string();
                let mut switched = false;
                for w in &mut self.windows {
                    if w.id == win_id {
                        w.focused = false;
                        w.z_order = 0;
                        switched = true;
                    }
                }
                if switched && win_id == self.active_window_id {
                    for w in &mut self.windows {
                        w.focused = false;
                    }
                    if let Some(other) = self.windows.iter_mut().find(|w| w.id != win_id) {
                        other.focused = true;
                        other.z_order += 1;
                        self.active_window_id = other.id.clone();
                        self.last_target_app = other.app.clone();
                    }
                }
                self.frame_seq += 1;
                self.record_action(
                    "window_minimize",
                    "success",
                    1,
                    json!({"windowId": win_id, "active": self.active_window_id}),
                );
                Ok(self.capture_frame())
            }
            "window_maximize" | "maximize_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.active_window_id);
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == win_id) {
                    win.bounds = [0, 0, self.width as i32, self.height as i32];
                }
                self.frame_seq += 1;
                self.record_action("window_maximize", "success", 1, json!({"windowId": win_id}));
                Ok(self.capture_frame())
            }
            "window_restore" | "restore_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.active_window_id)
                    .to_string();
                for w in &mut self.windows {
                    w.focused = false;
                }
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == win_id) {
                    win.bounds = [120, 120, 1024, 640];
                    win.focused = true;
                    self.active_window_id = win.id.clone();
                    self.last_target_app = win.app.clone();
                }
                self.frame_seq += 1;
                self.record_action("window_restore", "success", 1, json!({"windowId": win_id}));
                Ok(self.capture_frame())
            }
            "window_resize" | "resize_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.active_window_id);
                let nw = args
                    .get("width")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(800)
                    .clamp(1, i32::MAX as i64) as i32;
                let nh = args
                    .get("height")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(600)
                    .clamp(1, i32::MAX as i64) as i32;
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == win_id) {
                    win.bounds[2] = nw;
                    win.bounds[3] = nh;
                }
                self.frame_seq += 1;
                self.record_action(
                    "window_resize",
                    "success",
                    1,
                    json!({"windowId": win_id, "bounds": [nw, nh]}),
                );
                Ok(self.capture_frame())
            }
            "window_move" | "move_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.active_window_id);
                let nx = args
                    .get("x")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
                    .clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                let ny = args
                    .get("y")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
                    .clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                if let Some(win) = self.windows.iter_mut().find(|w| w.id == win_id) {
                    win.bounds[0] = nx;
                    win.bounds[1] = ny;
                }
                self.frame_seq += 1;
                self.record_action(
                    "window_move",
                    "success",
                    1,
                    json!({"windowId": win_id, "position": [nx, ny]}),
                );
                Ok(self.capture_frame())
            }
            "window_close" | "close_window" => {
                self.assert_agent_may_act(args)?;
                let win_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.active_window_id)
                    .to_string();
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
                self.frame_seq += 1;
                self.record_action(
                    "window_close",
                    "success",
                    1,
                    json!({"closedId": win_id, "active": self.active_window_id}),
                );
                Ok(self.capture_frame())
            }
            "register_window" => {
                self.assert_agent_may_act(args)?;
                self.save_undo_checkpoint();
                let id = args
                    .get("window_id")
                    .or_else(|| args.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("win-custom")
                    .to_string();
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Custom Window")
                    .to_string();
                let app = args
                    .get("app")
                    .and_then(|v| v.as_str())
                    .unwrap_or("CustomApp")
                    .to_string();
                for w in &mut self.windows {
                    w.focused = false;
                }
                let z = (self.windows.len() + 1) as u32;
                if let Some(existing) = self.windows.iter_mut().find(|w| w.id == id) {
                    existing.title = title;
                    existing.app = app.clone();
                    existing.focused = true;
                    existing.z_order = z;
                } else {
                    if self.windows.len() >= 32 {
                        self.windows
                            .retain(|w| w.focused || w.id == self.active_window_id);
                    }
                    self.windows.push(VirtualWindow {
                        id: id.clone(),
                        title,
                        app: app.clone(),
                        bounds: [120, 120, 1024, 640],
                        focused: true,
                        z_order: z,
                        url: "about:blank".into(),
                        html: "<html><body></body></html>".into(),
                        elements: Vec::new(),
                    });
                }
                self.active_window_id = id.clone();
                self.last_target_app = app;
                self.frame_seq += 1;
                self.record_action("register_window", "success", 2, json!({"windowId": id}));
                Ok(self.capture_frame())
            }
            "search_dsl" | "query" => {
                Ok(self.search_dsl(args.get("query").and_then(|v| v.as_str()).unwrap_or("")))
            }
            "audit_health" | "health" | "metrics" => Ok(self.audit_health()),
            "undo" => {
                self.assert_agent_may_act(args)?;
                self.undo()
            }
            "redo" => {
                self.assert_agent_may_act(args)?;
                self.redo()
            }
            other => Err(format!("Unsupported computer_use action '{other}'.")),
        }
    }

    pub fn execute_action(&mut self, args: &Value) -> Result<Value, String> {
        let raw_act = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("capture")
            .trim()
            .to_ascii_lowercase();
        let btn = args.get("button").and_then(|v| v.as_str()).unwrap_or("");
        let count = args
            .get("click_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);
        let action = if btn == "right" {
            "right_click".to_string()
        } else if btn == "middle" {
            "middle_click".to_string()
        } else if count == 2 {
            "double_click".to_string()
        } else if count >= 3 {
            "triple_click".to_string()
        } else {
            normalize_cua_action(&raw_act).to_string()
        };
        let norm_1000 = args.get("coord_space").and_then(|v| v.as_str()) == Some("normalized_1000");
        match action.as_str() {
            "lease_acquire" | "lease_release" | "lease_status" => {
                if action == "lease_acquire" {
                    self.assert_agent_may_act(args)?;
                    self.lease_holder = args
                        .get("holder")
                        .and_then(|v| v.as_str())
                        .unwrap_or("human")
                        .to_string();
                    self.lease_viewer_id = args
                        .get("viewer_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    self.lease_epoch += 1;
                } else if action == "lease_release" {
                    self.assert_agent_may_act(args)?;
                    self.lease_holder = "agent".into();
                    self.lease_viewer_id = None;
                    self.lease_epoch += 1;
                }
                Ok(
                    json!({ "holder": self.lease_holder, "viewerId": self.lease_viewer_id, "epoch": self.lease_epoch }),
                )
            }
            "capture" | "capture_frame" | "screenshot" | "snapshot" => {
                let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("som");
                let scale = args.get("scale").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let state_sig =
                    serde_json::to_string(&(&self.windows, &self.active_window_id, mode))
                        .unwrap_or_default();
                let cur_hash = format!("{:08x}", fnv1a_32(state_sig.as_bytes()));
                let unchanged = if !self.last_capture_hash.is_empty()
                    && self.last_capture_hash == cur_hash
                    && self.dedup_streak < 2
                {
                    self.dedup_streak += 1;
                    true
                } else {
                    self.last_capture_hash = cur_hash.clone();
                    self.dedup_streak = 0;
                    false
                };
                self.record_action(
                    "capture",
                    "success",
                    1,
                    json!({"mode": mode, "screenUnchanged": unchanged}),
                );
                Ok(self.render_capture(mode, scale, unchanged, Some(cur_hash)))
            }
            "list_windows" | "windows" => {
                let list: Vec<Value> = self.windows.iter().map(|w| json!({
                    "id": w.id, "title": w.title, "app": w.app, "bounds": w.bounds,
                    "focused": w.focused, "elementCount": w.elements.len(), "url": redact_cdp_url(&w.url)
                })).collect();
                Ok(
                    json!({ "activeWindowId": self.active_window_id, "targetApp": self.last_target_app, "windows": list }),
                )
            }
            "list_apps" | "apps" => {
                let apps: Vec<Value> = self
                    .windows
                    .iter()
                    .map(|w| json!({"app": w.app, "windowId": w.id, "focused": w.focused}))
                    .collect();
                Ok(json!({ "activeApp": self.last_target_app, "apps": apps }))
            }
            "focus_app" => {
                self.assert_agent_may_act(args)?;
                let app_name = args
                    .get("app")
                    .or_else(|| args.get("target"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'app' for focus_app".to_string())?;
                let win_id = self
                    .windows
                    .iter()
                    .find(|w| w.app.eq_ignore_ascii_case(app_name))
                    .map(|w| w.id.clone())
                    .ok_or_else(|| format!("App '{app_name}' not found"))?;
                self.execute_action(&forward_cua_context(
                    json!({"action": "focus_window", "window_id": win_id}),
                    args,
                ))
            }
            "read_window_below" => {
                let win_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.active_window_id);
                let win = self
                    .windows
                    .iter()
                    .find(|w| w.id == win_id)
                    .or_else(|| self.windows.first())
                    .ok_or_else(|| "No window available".to_string())?;
                Ok(json!({
                    "windowId": win.id, "title": win.title, "app": win.app, "bounds": win.bounds,
                    "zOrder": win.z_order, "focused": win.focused, "url": redact_cdp_url(&win.url),
                    "elementCount": win.elements.len(), "leaseHolder": self.lease_holder, "leaseEpoch": self.lease_epoch
                }))
            }
            "list_elements" | "elements" | "som" => {
                let win_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.active_window_id);
                let win = self
                    .windows
                    .iter()
                    .find(|w| w.id == win_id)
                    .ok_or_else(|| format!("Window '{win_id}' not found"))?;
                Ok(json!({ "windowId": win.id, "title": win.title, "elements": win.elements }))
            }
            "drive_preview" => self.execute_drive_preview(args),
            "annotate_preview" | "annotate" => self.execute_annotate_preview(args),
            "focus_window" | "focus" => {
                self.assert_agent_may_act(args)?;
                let target_id = args
                    .get("window_id")
                    .or_else(|| args.get("id"))
                    .or_else(|| args.get("target"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'window_id' for focus_window".to_string())?;
                let target_app = self
                    .windows
                    .iter()
                    .find(|w| w.id == target_id)
                    .map(|w| w.app.clone())
                    .ok_or_else(|| format!("Window '{target_id}' not found"))?;
                self.save_undo_checkpoint();
                for w in &mut self.windows {
                    w.focused = w.id == target_id;
                    if w.focused {
                        w.z_order += 1;
                    }
                }
                self.active_window_id = target_id.to_string();
                self.last_target_app = target_app;
                self.frame_seq += 1;
                self.record_action(
                    "focus_window",
                    "success",
                    2,
                    json!({"windowId": target_id, "app": self.last_target_app}),
                );
                Ok(self.capture_frame())
            }
            "click" | "double_click" | "triple_click" | "right_click" | "middle_click" => {
                self.assert_agent_may_act(args)?;
                let delivery = self.validate_delivery_and_target(args)?;
                let som_id = self.resolve_token_or_som(args)?;
                let target_elem = args
                    .get("element_id")
                    .or_else(|| args.get("ref"))
                    .or_else(|| args.get("target"))
                    .and_then(|v| v.as_str());
                self.save_undo_checkpoint();
                let mut resolved_id = String::new();
                let mut disabled_id = String::new();
                let raw_x =
                    parse_coordinate_value(args.get("x"), self.width, self.cursor[0], norm_1000);
                let raw_y =
                    parse_coordinate_value(args.get("y"), self.height, self.cursor[1], norm_1000);
                let mut click_xy = [raw_x, raw_y];
                let active_id = self.active_window_id.clone();
                for win in &mut self.windows {
                    let is_active = win.id == active_id;
                    if !is_active
                        && (!win.is_modal_window()
                            || !resolved_id.is_empty()
                            || !disabled_id.is_empty())
                    {
                        continue;
                    }
                    for el in &mut win.elements {
                        if resolved_id.is_empty()
                            && disabled_id.is_empty()
                            && el.matches_selector(target_elem, som_id, Some(click_xy))
                        {
                            let (pt, ok) = el.apply_click_interaction();
                            click_xy = pt;
                            if ok {
                                resolved_id = el.id.clone();
                            } else {
                                disabled_id = el.id.clone();
                            }
                        } else if is_active {
                            el.focused = false;
                        }
                    }
                }
                self.cursor = [
                    click_xy[0].clamp(0, self.width.min(i32::MAX as u32) as i32),
                    click_xy[1].clamp(0, self.height.min(i32::MAX as u32) as i32),
                ];
                self.frame_seq += 1;
                let verdict = if !disabled_id.is_empty() {
                    json!({"decision": "abort", "effect": "target_disabled", "recommended": "enable_prerequisite_control"})
                } else if !resolved_id.is_empty() {
                    json!({"decision": "done", "effect": "confirmed", "recommended": Value::Null})
                } else {
                    json!({"decision": "escalate", "effect": "suspected_noop", "recommended": if delivery == "background" { "foreground" } else { "px" }})
                };
                self.record_action(&action, "success", 3, json!({"cursor": self.cursor, "elementId": resolved_id, "disabledElementId": disabled_id, "deliveryMode": delivery, "verdict": verdict}));
                Ok(
                    json!({ "action": action, "deliveryMode": delivery, "clickedAt": self.cursor, "resolvedElementId": resolved_id, "targetDisabled": !disabled_id.is_empty(), "verdict": verdict, "frame": self.capture_frame() }),
                )
            }

            _ => self.execute_extended_action(&action, args, norm_1000),
        }
    }

}
