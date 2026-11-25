// [LAYER: DOMAIN]
pub use crate::domain::types::computer_use_types::{
    IUiElement, IVirtualWindow, UiElement, VirtualWindow,
};
use crate::plumbing::hash_and_math::{fnv1a_32, now_ms};
use crate::plumbing::vision_and_dom::generate_synthetic_svg;
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
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn el(
    id: &str,
    som_id: u32,
    role: &str,
    label: &str,
    value: &str,
    bounds: [i32; 4],
    focused: bool,
) -> UiElement {
    UiElement {
        id: id.into(),
        som_id,
        role: role.into(),
        label: label.into(),
        value: value.into(),
        bounds,
        enabled: true,
        focused,
    }
}

impl VirtualDisplayState {
    pub fn new() -> Self {
        let default_html = "<html><head><title>HEAV3NS-MICRO Sovereign Workbench</title></head><body><button id=\"btn-run\" role=\"button\" aria-label=\"Run Flight Verification\">Run Flight</button><input id=\"inp-cmd\" type=\"text\" placeholder=\"Enter command or goal...\"/><a id=\"lnk-docs\" href=\"https://heav3ns.local/docs\">Architecture Docs</a><button id=\"btn-swarm\" role=\"button\" aria-label=\"Engage Legion Swarm\">Engage Swarm</button></body></html>".to_string();
        let main_win = VirtualWindow {
            id: "win-main-1".into(),
            title: "HEAV3NS-MICRO Sovereign Workbench".into(),
            app: "Heav3nsCode".into(),
            bounds: [0, 0, 1440, 900],
            focused: true,
            z_order: 1,
            url: "https://heav3ns.local/workbench".into(),
            html: default_html,
            elements: vec![
                el(
                    "btn-run",
                    1,
                    "button",
                    "Run Flight Verification",
                    "",
                    [40, 80, 200, 40],
                    false,
                ),
                el(
                    "inp-cmd",
                    2,
                    "textbox",
                    "Command Input",
                    "",
                    [260, 80, 480, 40],
                    true,
                ),
                el(
                    "lnk-docs",
                    3,
                    "link",
                    "Architecture Docs",
                    "https://heav3ns.local/docs",
                    [760, 80, 180, 40],
                    false,
                ),
                el(
                    "btn-swarm",
                    4,
                    "button",
                    "Engage Legion Swarm",
                    "",
                    [960, 80, 200, 40],
                    false,
                ),
            ],
        };
        let term_win = VirtualWindow {
            id: "win-term-2".into(), title: "Sovereign Terminal Multiplexer".into(), app: "Terminal".into(),
            bounds: [80, 480, 1280, 380], focused: false, z_order: 0, url: "about:blank".into(),
            html: "<html><head><title>Terminal</title></head><body><textarea id=\"term-stdin\" aria-label=\"Terminal Input\"></textarea></body></html>".into(),
            elements: vec![el("term-stdin", 5, "textbox", "Terminal Input", "$ cargo test --quiet", [100, 520, 1240, 320], false)],
        };
        Self {
            width: 1440,
            height: 900,
            cursor: [300, 100],
            active_window_id: "win-main-1".into(),
            frame_seq: 1,
            windows: vec![main_win, term_win],
            action_ledger: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            pending_dialog: None,
            lease_holder: "agent".into(),
            lease_viewer_id: None,
            lease_epoch: 1,
            last_capture_hash: String::new(),
            dedup_streak: 0,
            last_target_app: "Heav3nsCode".into(),
            preview_inventory: BTreeMap::new(),
            annotations: Vec::new(),
            clipboard: String::new(),
            mouse_pressed: false,
            scroll_offset: [0, 0],
        }
    }

    pub(crate) fn assert_agent_may_act(&self, args: &Value) -> Result<(), String> {
        let actor = args
            .get("actor")
            .and_then(|v| v.as_str())
            .unwrap_or("agent");
        if self.lease_holder == "human" && actor != "human" {
            return Err(format!(
                "human_has_control: Human viewer {:?} holds desktop lease (epoch {})",
                self.lease_viewer_id, self.lease_epoch
            ));
        }
        if let Some(exp) = args.get("expected_epoch").and_then(|v| v.as_u64()) {
            if exp != self.lease_epoch {
                return Err(format!(
                    "stale_lease_epoch: Expected epoch {} but current lease epoch is {}",
                    exp, self.lease_epoch
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn save_undo_checkpoint(&mut self) {
        if let Ok(snap) = serde_json::to_string(&(
            &self.windows,
            &self.cursor,
            &self.active_window_id,
            &self.last_target_app,
            &self.scroll_offset,
        )) {
            if self.undo_stack.len() >= 32 {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(snap);
            self.redo_stack.clear();
        }
    }

    pub fn undo(&mut self) -> Result<Value, String> {
        let prev = self
            .undo_stack
            .pop()
            .ok_or_else(|| "Undo stack is empty".to_string())?;
        if let Ok(cur) = serde_json::to_string(&(
            &self.windows,
            &self.cursor,
            &self.active_window_id,
            &self.last_target_app,
            &self.scroll_offset,
        )) {
            self.redo_stack.push(cur);
        }
        let (wins, cursor, active, target_app, scroll): (
            Vec<VirtualWindow>,
            [i32; 2],
            String,
            String,
            [i32; 2],
        ) = serde_json::from_str(&prev).map_err(|e| format!("Undo error: {e}"))?;
        self.windows = wins;
        self.cursor = cursor;
        self.active_window_id = active;
        self.last_target_app = target_app;
        self.scroll_offset = scroll;
        self.frame_seq += 1;
        self.record_action(
            "undo",
            "success",
            1,
            json!({"restoredFrame": self.frame_seq}),
        );
        Ok(self.capture_frame())
    }

    pub fn redo(&mut self) -> Result<Value, String> {
        let next = self
            .redo_stack
            .pop()
            .ok_or_else(|| "Redo stack is empty".to_string())?;
        if let Ok(cur) = serde_json::to_string(&(
            &self.windows,
            &self.cursor,
            &self.active_window_id,
            &self.last_target_app,
            &self.scroll_offset,
        )) {
            self.undo_stack.push(cur);
        }
        let (wins, cursor, active, target_app, scroll): (
            Vec<VirtualWindow>,
            [i32; 2],
            String,
            String,
            [i32; 2],
        ) = serde_json::from_str(&next).map_err(|e| format!("Redo error: {e}"))?;
        self.windows = wins;
        self.cursor = cursor;
        self.active_window_id = active;
        self.last_target_app = target_app;
        self.scroll_offset = scroll;
        self.frame_seq += 1;
        self.record_action(
            "redo",
            "success",
            1,
            json!({"restoredFrame": self.frame_seq}),
        );
        Ok(self.capture_frame())
    }

    pub(crate) fn record_action(&mut self, action: &str, status: &str, duration_ms: u64, detail: Value) {
        let seq = self
            .action_ledger
            .last()
            .and_then(|v| v.get("seq"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            + 1;
        if self.action_ledger.len() >= 512 {
            self.action_ledger.remove(0);
        }
        self.action_ledger.push(json!({
            "seq": seq, "action": action, "window": self.active_window_id, "status": status,
            "durationMs": duration_ms, "timestampMs": now_ms(), "cursor": self.cursor, "detail": detail
        }));
    }

    pub fn capture_frame(&self) -> Value {
        self.render_capture("som", 1, false, None)
    }

    pub(crate) fn render_capture(
        &self,
        mode: &str,
        scale: u32,
        unchanged: bool,
        hash_override: Option<String>,
    ) -> Value {
        let active_win = self
            .windows
            .iter()
            .find(|w| w.id == self.active_window_id)
            .or_else(|| self.windows.first());
        let title = active_win.map(|w| w.title.as_str()).unwrap_or("Display");
        let svg = generate_synthetic_svg(self.width, self.height, title);
        let frame_hash =
            hash_override.unwrap_or_else(|| format!("{:08x}", fnv1a_32(svg.as_bytes())));
        let som_elements: Vec<Value> = if mode == "vision" {
            Vec::new()
        } else {
            active_win
                .map(|w| {
                    w.elements
                        .iter()
                        .map(|el| el.to_som_json(self.frame_seq, true))
                        .collect()
                })
                .unwrap_or_default()
        };
        let max_x = active_win
            .and_then(|w| {
                w.elements
                    .iter()
                    .map(|e| e.bounds[0].saturating_add(e.bounds[2]).max(0))
                    .max()
            })
            .unwrap_or(0) as u32;
        let eff_scale = if scale > 1 {
            scale
        } else if max_x > self.width {
            2
        } else {
            1
        };
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

    pub(crate) fn resolve_token_or_som(&self, args: &Value) -> Result<Option<u32>, String> {
        if let Some(tok) = args.get("element_token").and_then(|v| v.as_str()) {
            let stripped = tok
                .strip_prefix('s')
                .ok_or_else(|| format!("Invalid element_token '{tok}'"))?;
            let (hex_seq, idx_str) = stripped
                .split_once(':')
                .ok_or_else(|| format!("Malformed element_token '{tok}'"))?;
            let idx = idx_str
                .parse::<u32>()
                .map_err(|_| format!("Invalid index in element_token '{tok}'"))?;
            let cur_hex = format!("{:x}", self.frame_seq);
            if hex_seq != cur_hex
                && args
                    .get("strict_token")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            {
                return Err(format!("stale_element_token: Token '{tok}' belongs to frame s{hex_seq}, current is s{cur_hex}"));
            }
            return Ok(Some(idx));
        }
        Ok(args
            .get("som_id")
            .or_else(|| args.get("mark"))
            .and_then(|v| v.as_u64())
            .map(|n| n as u32))
    }

    pub(crate) fn validate_delivery_and_target(&mut self, args: &Value) -> Result<String, String> {
        let delivery = args
            .get("delivery_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("background")
            .to_ascii_lowercase();
        let bring_front = args
            .get("bring_to_front")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if bring_front && delivery != "foreground" {
            return Err("bring_to_front_requires_foreground: Set delivery_mode='foreground' when bring_to_front=true".to_string());
        }
        if let Some(req_app) = args.get("app").and_then(|v| v.as_str()) {
            if !req_app.is_empty() && !req_app.eq_ignore_ascii_case(&self.last_target_app) {
                return Err(format!(
                    "input_target_mismatch: Input targeted app '{}' but active target is '{}'",
                    req_app, self.last_target_app
                ));
            }
        }
        if delivery == "foreground" {
            let active_id = self.active_window_id.clone();
            for w in &mut self.windows {
                if w.id == active_id {
                    w.focused = true;
                    w.z_order += 1;
                }
            }
        }
        Ok(delivery)
    }
}
