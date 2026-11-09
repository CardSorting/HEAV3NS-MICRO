// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub type ElementBounds = [i32; 4];
pub type ElementSomId = u32;

/// Contractual sovereignty interface for interactive UI elements.
pub trait IUiElement {
    fn element_id(&self) -> &str;
    fn is_focused(&self) -> bool;
}

/// Contractual sovereignty interface for virtual desktop windows.
pub trait IVirtualWindow {
    fn window_id(&self) -> &str;
    fn window_app(&self) -> &str;
}

/// Interactive UI element tracked within a Set-of-Marks or Accessibility tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiElement {
    pub id: String,
    #[serde(rename = "somId")]
    pub som_id: u32,
    pub role: String,
    pub label: String,
    pub value: String,
    pub bounds: [i32; 4], // [x, y, width, height]
    pub enabled: bool,
    pub focused: bool,
}

impl IUiElement for UiElement {
    fn element_id(&self) -> &str {
        &self.id
    }

    fn is_focused(&self) -> bool {
        self.focused
    }
}

impl UiElement {
    pub fn center_point(&self) -> [i32; 2] {
        [self.bounds[0] + self.bounds[2] / 2, self.bounds[1] + self.bounds[3] / 2]
    }

    pub fn hit_tests_point(&self, pt: [i32; 2]) -> bool {
        pt[0] >= self.bounds[0]
            && pt[0] <= self.bounds[0] + self.bounds[2]
            && pt[1] >= self.bounds[1]
            && pt[1] <= self.bounds[1] + self.bounds[3]
    }

    pub fn matches_selector(&self, target_elem: Option<&str>, som_id: Option<u32>, click_xy: Option<[i32; 2]>) -> bool {
        let by_id = target_elem
            .map(|tid| self.id == tid || self.label.eq_ignore_ascii_case(tid))
            .unwrap_or(false);
        let by_som = som_id.map(|sid| self.som_id == sid).unwrap_or(false);
        let by_xy = target_elem.is_none() && som_id.is_none() && click_xy.map(|pt| self.hit_tests_point(pt)).unwrap_or(false);
        by_id || by_som || by_xy
    }

    pub fn to_som_json(&self, frame_seq: u64, include_ref: bool) -> Value {
        if include_ref {
            json!({
                "somId": self.som_id, "id": self.id, "ref": self.id,
                "elementToken": format!("s{:x}:{}", frame_seq, self.som_id),
                "role": self.role, "label": self.label, "value": self.value, "bounds": self.bounds,
                "center": self.center_point(), "enabled": self.enabled, "focused": self.focused
            })
        } else {
            json!({
                "id": self.id, "somId": self.som_id,
                "elementToken": format!("s{:x}:{}", frame_seq, self.som_id),
                "role": self.role, "label": self.label, "value": self.value, "bounds": self.bounds,
                "center": self.center_point(), "enabled": self.enabled, "focused": self.focused
            })
        }
    }

    pub fn apply_click_interaction(&mut self) -> ([i32; 2], bool) {
        let pt = self.center_point();
        if !self.enabled {
            return (pt, false);
        }
        self.focused = true;
        let r_low = self.role.to_ascii_lowercase();
        if matches!(r_low.as_str(), "checkbox" | "switch" | "toggle") {
            self.value = if self.value == "true" || self.value == "checked" {
                "false".into()
            } else {
                "true".into()
            };
        } else if r_low == "radio" {
            self.value = "true".into();
        }
        (pt, true)
    }

    pub fn apply_text_mutation(&mut self, text: &str, overwrite: bool) -> bool {
        if !self.enabled {
            return false;
        }
        if overwrite {
            self.value = text.to_string();
        } else {
            self.value.push_str(text);
        }
        self.focused = true;
        true
    }
}

/// Virtual application window containing interactive UI elements and DOM state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VirtualWindow {
    pub id: String,
    pub title: String,
    pub app: String,
    pub bounds: [i32; 4],
    pub focused: bool,
    pub z_order: u32,
    pub url: String,
    pub html: String,
    pub elements: Vec<UiElement>,
}

impl IVirtualWindow for VirtualWindow {
    fn window_id(&self) -> &str {
        &self.id
    }

    fn window_app(&self) -> &str {
        &self.app
    }
}

impl VirtualWindow {
    pub fn is_modal_window(&self) -> bool {
        let tl = self.title.to_ascii_lowercase();
        tl.contains("dialog") || tl.contains("alert") || tl.contains("confirm") || tl.contains("modal")
    }

    pub fn query_window_elements(&self, frame_seq: u64, args: &Value) -> Vec<Value> {
        let role_filter = args.get("role").and_then(|v| v.as_str());
        let label_filter = args.get("label_contains").or_else(|| args.get("text")).and_then(|v| v.as_str()).map(|s| s.to_ascii_lowercase());
        let value_filter = args.get("value_contains").and_then(|v| v.as_str()).map(|s| s.to_ascii_lowercase());
        let elem_id_filter = args.get("element_id").or_else(|| args.get("id")).and_then(|v| v.as_str());
        let focused_only = args.get("focused_only").and_then(|v| v.as_bool()).unwrap_or(false);
        let enabled_only = args.get("enabled_only").and_then(|v| v.as_bool()).unwrap_or(false);
        self.elements
            .iter()
            .filter(|e| {
                role_filter.map(|r| e.role.eq_ignore_ascii_case(r)).unwrap_or(true)
                    && elem_id_filter.map(|eid| e.id == eid || e.label.eq_ignore_ascii_case(eid)).unwrap_or(true)
                    && label_filter.as_ref().map(|l| e.label.to_ascii_lowercase().contains(l)).unwrap_or(true)
                    && value_filter.as_ref().map(|vl| e.value.to_ascii_lowercase().contains(vl)).unwrap_or(true)
                    && (!focused_only || e.focused)
                    && (!enabled_only || e.enabled)
            })
            .map(|e| e.to_som_json(frame_seq, false))
            .collect()
    }

    pub fn apply_window_key(&mut self, k_norm: &str, clipboard: &mut String, cursor: &mut [i32; 2]) -> bool {
        match k_norm {
            "cmd+c" | "ctrl+c" => {
                if let Some(el) = self.elements.iter().find(|e| e.focused) {
                    *clipboard = el.value.clone();
                }
                true
            }
            "cmd+v" | "ctrl+v" => {
                if let Some(el) = self.elements.iter_mut().find(|e| e.focused && e.enabled) {
                    el.value.push_str(clipboard);
                }
                true
            }
            "tab" | "shift+tab" => {
                let cur = self.elements.iter().position(|e| e.focused);
                for e in &mut self.elements {
                    e.focused = false;
                }
                if !self.elements.is_empty() {
                    let len = self.elements.len();
                    let next_idx = if k_norm == "tab" {
                        cur.map(|i| (i + 1) % len).unwrap_or(0)
                    } else {
                        cur.map(|i| (i + len - 1) % len).unwrap_or(len - 1)
                    };
                    self.elements[next_idx].focused = true;
                    *cursor = self.elements[next_idx].center_point();
                }
                true
            }
            "backspace" | "delete" => {
                if let Some(el) = self.elements.iter_mut().find(|e| e.focused && e.enabled) {
                    el.value.pop();
                }
                true
            }
            "cmd+backspace" | "ctrl+backspace" | "clear" => {
                if let Some(el) = self.elements.iter_mut().find(|e| e.focused && e.enabled) {
                    el.value.clear();
                }
                true
            }
            "space" => {
                if let Some(el) = self.elements.iter_mut().find(|e| e.focused && e.enabled) {
                    let r_low = el.role.to_ascii_lowercase();
                    if matches!(r_low.as_str(), "checkbox" | "switch" | "toggle") {
                        el.value = if el.value == "true" || el.value == "checked" { "false".into() } else { "true".into() };
                    } else if matches!(r_low.as_str(), "textbox" | "input" | "searchbox" | "textarea") {
                        el.value.push(' ');
                    }
                }
                true
            }
            "return" | "enter" => {
                if let Some(el) = self.elements.iter().find(|e| e.focused && (e.role == "button" || e.role == "link")) {
                    *cursor = el.center_point();
                }
                true
            }
            _ => false,
        }
    }
}
