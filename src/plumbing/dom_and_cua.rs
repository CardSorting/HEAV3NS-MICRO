// [LAYER: PLUMBING]
pub const MAX_DOM_SNAPSHOT_BYTES: usize = 64 * 1024;

pub fn redact_cdp_url(url: &str) -> String {
    let mut out = url.to_string();
    for key in [
        "token",
        "api_key",
        "apikey",
        "secret",
        "password",
        "access_token",
        "session_key",
        "auth",
    ] {
        for sep in ["?", "&"] {
            let pat = format!("{}{}=", sep, key);
            let mut search_from = 0usize;
            while let Some(rel_pos) = out[search_from..]
                .as_bytes()
                .windows(pat.len())
                .position(|w| w.eq_ignore_ascii_case(pat.as_bytes()))
            {
                let val_start = search_from + rel_pos + pat.len();
                let val_end = out[val_start..]
                    .find(|c| c == '&' || c == '#')
                    .map(|i| val_start + i)
                    .unwrap_or(out.len());
                out.replace_range(val_start..val_end, "[REDACTED]");
                search_from = val_start + "[REDACTED]".len();
            }
        }
    }
    out
}

use serde_json::{json, Value};

pub fn snapshot_html_dom(html: &str, url: &str, max_elements: usize) -> Value {
    let bounded_html = crate::plumbing::text_tokens::safe_utf8_prefix(html, MAX_DOM_SNAPSHOT_BYTES);
    let sanitized = strip_heavy_html_blocks(bounded_html);
    let title =
        extract_tag_inner(&sanitized, "title").unwrap_or_else(|| "Untitled Document".to_string());
    let mut interactive = Vec::new();
    let mut som_id = 1usize;
    let limit = max_elements.clamp(1, 250);

    // Lightweight deterministic Set-of-Marks (SoM) scanner over HTML tags
    let bytes = sanitized.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && interactive.len() < limit {
        if bytes[i] == b'<' {
            let mut quote: Option<u8> = None;
            let rel_end = bytes[i..].iter().position(|&b| match quote {
                Some(q) => {
                    if b == q {
                        quote = None;
                    }
                    false
                }
                None if b == b'"' || b == b'\'' => {
                    quote = Some(b);
                    false
                }
                None => b == b'>',
            });
            if let Some(rel_end) = rel_end {
                let tag_slice = &sanitized[i + 1..i + rel_end];
                let trimmed = tag_slice.trim();
                if !trimmed.starts_with('/')
                    && !trimmed.starts_with('!')
                    && !trimmed.starts_with('?')
                {
                    let tag_name: String = trimmed
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                        .collect::<String>()
                        .to_ascii_uppercase();
                    let role = extract_html_attr(trimmed, "role").unwrap_or_default();
                    let is_interactive = matches!(
                        tag_name.as_str(),
                        "A" | "BUTTON"
                            | "INPUT"
                            | "SELECT"
                            | "TEXTAREA"
                            | "SUMMARY"
                            | "DETAILS"
                            | "OPTION"
                    ) || matches!(
                        role.as_str(),
                        "button"
                            | "link"
                            | "textbox"
                            | "checkbox"
                            | "menuitem"
                            | "tab"
                            | "switch"
                            | "combobox"
                    ) || extract_html_attr(trimmed, "onclick").is_some();

                    if is_interactive {
                        let elem_id = extract_html_attr(trimmed, "id").unwrap_or_default();
                        let aria_label = extract_html_attr(trimmed, "aria-label")
                            .or_else(|| extract_html_attr(trimmed, "placeholder"))
                            .or_else(|| extract_html_attr(trimmed, "name"))
                            .or_else(|| extract_html_attr(trimmed, "title"))
                            .unwrap_or_default();
                        let href = extract_html_attr(trimmed, "href").unwrap_or_default();
                        let input_type = extract_html_attr(trimmed, "type").unwrap_or_default();
                        let inner_text = extract_following_text(&sanitized[i + rel_end + 1..]);
                        let label = if !aria_label.is_empty() {
                            aria_label
                        } else if !inner_text.is_empty() {
                            inner_text
                        } else if !elem_id.is_empty() {
                            format!("#{}", elem_id)
                        } else {
                            tag_name.to_ascii_lowercase()
                        };
                        interactive.push(json!({
                            "somId": som_id,
                            "tag": tag_name,
                            "id": elem_id,
                            "role": if role.is_empty() { tag_name.to_ascii_lowercase() } else { role },
                            "label": label,
                            "href": href,
                            "inputType": input_type,
                            "selector": if !elem_id.is_empty() {
                                format!("#{}", elem_id)
                            } else {
                                format!("{}[data-som='{}']", tag_name.to_ascii_lowercase(), som_id)
                            }
                        }));
                        som_id += 1;
                    }
                }
                i += rel_end + 1;
                continue;
            }
            break;
        }
        i += 1;
    }

    let plain_text = collapse_whitespace(&strip_all_tags(&sanitized));
    let truncated_text: String = plain_text.chars().take(4096).collect();
    json!({
        "url": redact_cdp_url(url),
        "title": title,
        "interactiveCount": interactive.len(),
        "interactiveElements": interactive,
        "textDigest": truncated_text,
        "rawByteLength": html.len(),
        "compressedByteLength": truncated_text.len()
    })
}

fn strip_heavy_html_blocks(html: &str) -> String {
    let mut current = html.to_string();
    for tag in ["script", "style", "svg", "noscript"] {
        let open_pat = format!("<{}", tag);
        let close_pat = format!("</{}>", tag);
        let lower = current.to_ascii_lowercase();
        let mut out = String::with_capacity(current.len());
        let mut cursor = 0usize;
        while let Some(rel_start) = lower[cursor..].find(&open_pat) {
            let start = cursor + rel_start;
            if let Some(rel_end) = lower[start..].find(&close_pat) {
                out.push_str(&current[cursor..start]);
                out.push(' ');
                cursor = start + rel_end + close_pat.len();
            } else {
                break;
            }
        }
        if cursor > 0 {
            out.push_str(&current[cursor..]);
            current = out;
        }
    }
    current
}

fn extract_tag_inner(html: &str, tag: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let start_tag = lower.find(&open)?;
    let gt = lower[start_tag..].find('>')? + start_tag + 1;
    let end_tag = lower[gt..].find(&close)? + gt;
    Some(collapse_whitespace(&html[gt..end_tag]))
}

fn extract_html_attr(tag_body: &str, attr_name: &str) -> Option<String> {
    let lower = tag_body.to_ascii_lowercase();
    for quote in ['"', '\''] {
        let pat = format!("{}={}", attr_name.to_ascii_lowercase(), quote);
        if let Some((pos, _)) = lower
            .match_indices(&pat)
            .find(|(p, _)| *p == 0 || lower.as_bytes()[p - 1].is_ascii_whitespace())
        {
            let rest = &tag_body[pos + pat.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].trim().to_string());
            }
        }
    }
    None
}

fn extract_following_text(after_open_tag: &str) -> String {
    let end = after_open_tag
        .find('<')
        .unwrap_or_else(|| after_open_tag.len().min(120));
    collapse_whitespace(crate::plumbing::text_tokens::safe_utf8_prefix(
        after_open_tag,
        end.min(120),
    ))
}

fn strip_all_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len().min(8192));
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            out.push(' ');
        } else if ch == '>' {
            in_tag = false;
            out.push(' ');
        } else if !in_tag {
            out.push(ch);
        }
    }
    out
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn parse_coordinate_value(
    val: Option<&Value>,
    dim: u32,
    default: i32,
    is_norm_1000: bool,
) -> i32 {
    let parsed: Option<f64> = match val {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if let Some(pct) = trimmed.strip_suffix('%') {
                pct.trim()
                    .parse::<f64>()
                    .ok()
                    .map(|p| (p / 100.0) * (dim as f64))
            } else {
                trimmed.parse::<f64>().ok()
            }
        }
        _ => None,
    };

    match parsed {
        Some(f) if f.is_finite() => {
            if !is_norm_1000 && f > 0.0 && f <= 1.0 {
                (f * (dim as f64)).round() as i32
            } else if is_norm_1000 && f > 0.0 && f <= 1000.0 {
                ((f * (dim as f64)) / 1000.0).round() as i32
            } else {
                f.round() as i32
            }
        }
        _ => default,
    }
}

pub fn parse_scroll_deltas(args: &Value) -> (i32, i32) {
    let dir = args
        .get("direction")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_ascii_lowercase());
    let amount = args
        .get("amount")
        .or_else(|| args.get("clicks"))
        .or_else(|| args.get("steps"))
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .unwrap_or(2)
        .clamp(-100, 100) as i32;
    let step = amount.saturating_mul(120);
    if let Some(d) = dir {
        match d.as_str() {
            "up" => (0, step),
            "down" => (0, -step),
            "left" => (step, 0),
            "right" => (-step, 0),
            _ => (0, -step),
        }
    } else {
        let dx = args
            .get("delta_x")
            .or_else(|| args.get("dx"))
            .and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(0) as i32;
        let dy = args
            .get("delta_y")
            .or_else(|| args.get("dy"))
            .and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(-240) as i32;
        (dx, dy)
    }
}

pub fn parse_drag_points(
    args: &Value,
    width: u32,
    height: u32,
    cur: [i32; 2],
    is_norm_1000: bool,
) -> ([i32; 2], [i32; 2]) {
    let (fx, fy) = if let Some(arr) = args.get("from").and_then(|v| v.as_array()) {
        if arr.len() >= 2 {
            (
                parse_coordinate_value(arr.get(0), width, cur[0], is_norm_1000),
                parse_coordinate_value(arr.get(1), height, cur[1], is_norm_1000),
            )
        } else {
            (cur[0], cur[1])
        }
    } else {
        (
            parse_coordinate_value(args.get("from_x"), width, cur[0], is_norm_1000),
            parse_coordinate_value(args.get("from_y"), height, cur[1], is_norm_1000),
        )
    };

    let max_w = width.min(i32::MAX as u32) as i32;
    let max_h = height.min(i32::MAX as u32) as i32;
    let fx_clamped = fx.clamp(0, max_w);
    let fy_clamped = fy.clamp(0, max_h);
    let def_tx = fx_clamped.saturating_add(100);
    let def_ty = fy_clamped.saturating_add(100);

    let (tx, ty) = if let Some(arr) = args.get("to").and_then(|v| v.as_array()) {
        if arr.len() >= 2 {
            (
                parse_coordinate_value(arr.get(0), width, def_tx, is_norm_1000),
                parse_coordinate_value(arr.get(1), height, def_ty, is_norm_1000),
            )
        } else {
            (def_tx, def_ty)
        }
    } else {
        (
            parse_coordinate_value(
                args.get("to_x").or_else(|| args.get("x")),
                width,
                def_tx,
                is_norm_1000,
            ),
            parse_coordinate_value(
                args.get("to_y").or_else(|| args.get("y")),
                height,
                def_ty,
                is_norm_1000,
            ),
        )
    };

    (
        [fx_clamped, fy_clamped],
        [tx.clamp(0, max_w), ty.clamp(0, max_h)],
    )
}

pub fn extract_key_sequence(args: &Value) -> String {
    if let Some(arr) = args.get("keys").and_then(|v| v.as_array()) {
        arr.iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join("+")
    } else {
        args.get("keys")
            .or_else(|| args.get("key"))
            .or_else(|| args.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("Return")
            .to_string()
    }
}

pub fn is_blocked_key_combo(combo: &str) -> bool {
    let norm = combo.trim().to_ascii_lowercase().replace(' ', "");
    matches!(
        norm.as_str(),
        "cmd+shift+backspace"
            | "command+shift+backspace"
            | "cmd+option+backspace"
            | "cmd+alt+backspace"
            | "cmd+ctrl+q"
            | "command+control+q"
            | "cmd+shift+q"
            | "command+shift+q"
            | "win+l"
            | "super+l"
            | "ctrl+alt+del"
            | "ctrl+alt+delete"
            | "option+f4"
            | "alt+f4"
    )
}

pub fn is_blocked_type_pattern(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    ["rm -rf /", ":(){:|:&};:", "mkfs", "| bash", "| sh", "| zsh"]
        .iter()
        .any(|p| lower.contains(p))
}

pub fn normalize_cua_action(raw: &str) -> &str {
    match raw.trim() {
        "left_click" | "single_click" | "tap" => "click",
        "dblclick" => "double_click",
        "triple_click" | "tri_click" => "triple_click",
        "context_click" | "secondary_click" => "right_click",
        "wheel_click" => "middle_click",
        "keypress" | "press" | "shortcut" | "key_combination" => "key",
        "write" | "enter_text" | "input_text" => "type",
        "drag_and_drop" | "mouse_drag" => "drag",
        "sleep" | "pause" | "delay" => "wait",
        "hover" | "mouse_move" | "cursor_position" => "hover",
        "copy" | "copy_clipboard" => "clipboard_write",
        "paste" | "paste_clipboard" => "clipboard_paste",
        "query" | "find" | "search_elements" => "find_elements",
        "window_focus" => "focus_window",
        "window_restore" | "restore_window" | "restore" => "window_restore",
        "minimize" => "window_minimize",
        "maximize" => "window_maximize",
        "close_window" | "close" => "window_close",
        "resize_window" | "resize" => "window_resize",
        "move_window" => "window_move",
        other => other,
    }
}
