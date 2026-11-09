// [LAYER: PLUMBING]
use serde_json::{json, Value};

pub trait IVisionAndDomCodec {
    fn decode_metadata(&self, bytes: &[u8]) -> Result<Value, String>;
}

pub trait IDomSnapshotter {
    fn snapshot_dom(&self, html: &str, url: &str) -> Value;
}

pub trait INavigationGuard {
    fn validate_url(&self, url: &str) -> Result<String, String>;
}

pub trait IUnicodeSanitizer {
    fn sanitize(&self, text: &str) -> String;
}

pub const MAX_IMAGE_PAYLOAD_BYTES: usize = 50 * 1024 * 1024;
pub const MAX_DOM_SNAPSHOT_BYTES: usize = 64 * 1024;

pub fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.max(1)
}

pub fn calculate_aspect_ratio(width: u32, height: u32) -> String {
    if width == 0 || height == 0 {
        return "0:0".to_string();
    }
    let divisor = gcd(width, height);
    format!("{}:{}", width / divisor, height / divisor)
}

pub fn decode_image_metadata(bytes: &[u8]) -> Result<Value, String> {
    if bytes.is_empty() {
        return Err("Empty image buffer".to_string());
    }
    if bytes.len() > MAX_IMAGE_PAYLOAD_BYTES {
        return Err(format!("Image exceeds max budget of {} bytes", MAX_IMAGE_PAYLOAD_BYTES));
    }

    // 1. PNG: 89 50 4E 47 0D 0A 1A 0A + IHDR @ 16..24
    if bytes.len() >= 24 && bytes[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        let bit_depth = if bytes.len() > 24 { bytes[24] } else { 8 };
        let color_type = if bytes.len() > 25 { bytes[25] } else { 6 };
        let has_alpha = color_type == 4 || color_type == 6;
        return Ok(build_img_meta("png", "image/png", width, height, bytes.len(), has_alpha, bit_depth, false));
    }

    // 2. GIF: GIF87a / GIF89a
    if bytes.len() >= 10 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a") {
        let width = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let height = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        let animated = bytes.windows(3).filter(|w| *w == [0x21, 0xF9, 0x04]).count() > 1;
        return Ok(build_img_meta("gif", "image/gif", width, height, bytes.len(), true, 8, animated));
    }

    // 3. BMP: 'BM' + width @ 18..22, height @ 22..26
    if bytes.len() >= 26 && &bytes[0..2] == b"BM" {
        let width = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]).unsigned_abs();
        let height = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]).unsigned_abs();
        let bpp = if bytes.len() >= 30 { u16::from_le_bytes([bytes[28], bytes[29]]) as u8 } else { 24 };
        return Ok(build_img_meta("bmp", "image/bmp", width, height, bytes.len(), bpp == 32, bpp, false));
    }

    // 4. WebP: 'RIFF'....'WEBP'
    if bytes.len() >= 30 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        let chunk = &bytes[12..16];
        let (width, height, alpha) = if chunk == b"VP8 " && bytes.len() >= 30 {
            let w = (u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3FFF) as u32;
            let h = (u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3FFF) as u32;
            (w, h, false)
        } else if chunk == b"VP8L" && bytes.len() >= 25 {
            let b0 = bytes[21] as u32;
            let b1 = bytes[22] as u32;
            let b2 = bytes[23] as u32;
            let b3 = bytes[24] as u32;
            let w = (b0 | ((b1 & 0x3F) << 8)) + 1;
            let h = (((b1 >> 6) | (b2 << 2) | ((b3 & 0x0F) << 10))) + 1;
            let has_alpha = ((b3 >> 4) & 0x01) == 1;
            (w, h, has_alpha)
        } else if chunk == b"VP8X" && bytes.len() >= 30 {
            let flags = bytes[20];
            let w = (bytes[24] as u32 | ((bytes[25] as u32) << 8) | ((bytes[26] as u32) << 16)) + 1;
            let h = (bytes[27] as u32 | ((bytes[28] as u32) << 8) | ((bytes[29] as u32) << 16)) + 1;
            (w, h, (flags & 0x10) != 0)
        } else {
            (0, 0, false)
        };
        return Ok(build_img_meta("webp", "image/webp", width, height, bytes.len(), alpha, 8, false));
    }

    // 5. JPEG: FF D8 ... scan SOF0/SOF1/SOF2 (FF C0 / FF C1 / FF C2)
    if bytes.len() >= 4 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        let mut idx = 2usize;
        let mut width = 0u32;
        let mut height = 0u32;
        let mut bit_depth = 8u8;
        while idx + 8 < bytes.len() {
            if bytes[idx] != 0xFF {
                idx += 1;
                continue;
            }
            while idx < bytes.len() && bytes[idx] == 0xFF {
                idx += 1;
            }
            if idx >= bytes.len() {
                break;
            }
            let marker = bytes[idx];
            idx += 1;
            if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
                continue;
            }
            if idx + 1 >= bytes.len() {
                break;
            }
            let seg_len = u16::from_be_bytes([bytes[idx], bytes[idx + 1]]) as usize;
            if (marker == 0xC0 || marker == 0xC1 || marker == 0xC2) && idx + 6 < bytes.len() {
                bit_depth = bytes[idx + 2];
                height = u16::from_be_bytes([bytes[idx + 3], bytes[idx + 4]]) as u32;
                width = u16::from_be_bytes([bytes[idx + 5], bytes[idx + 6]]) as u32;
                break;
            }
            if seg_len < 2 {
                break;
            }
            idx = idx.saturating_add(seg_len);
        }
        return Ok(build_img_meta("jpeg", "image/jpeg", width, height, bytes.len(), false, bit_depth, false));
    }

    // 6. SVG: XML/SVG text header
    if let Ok(text) = std::str::from_utf8(&bytes[..bytes.len().min(4096)]) {
        let trimmed = text.trim_start();
        if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && trimmed.contains("<svg")) {
            let (w, h) = parse_svg_dimensions(trimmed);
            return Ok(build_img_meta("svg", "image/svg+xml", w, h, bytes.len(), true, 8, false));
        }
    }

    Err("Unrecognized binary image header (expected PNG, JPEG, GIF, WebP, BMP, or SVG)".to_string())
}

fn build_img_meta(fmt: &str, mime: &str, width: u32, height: u32, byte_size: usize, has_alpha: bool, bit_depth: u8, animated: bool) -> Value {
    let aspect = calculate_aspect_ratio(width, height);
    let megapixels = ((width as f64 * height as f64) / 1_000_000.0 * 1000.0).round() / 1000.0;
    // Vision token heuristic (~85 base + 170 per 512x512 tile)
    let tiles_x = ((width.max(1) + 511) / 512).max(1);
    let tiles_y = ((height.max(1) + 511) / 512).max(1);
    let estimated_vision_tokens = 85 + (tiles_x * tiles_y * 170);
    json!({
        "format": fmt,
        "mimeType": mime,
        "width": width,
        "height": height,
        "aspectRatio": aspect,
        "megapixels": megapixels,
        "byteSize": byte_size,
        "hasAlpha": has_alpha,
        "bitDepth": bit_depth,
        "animated": animated,
        "estimatedVisionTokens": estimated_vision_tokens
    })
}

fn parse_svg_dimensions(svg: &str) -> (u32, u32) {
    let extract_attr = |attr: &str| -> Option<u32> {
        let pat = format!("{}=\"", attr);
        let start = svg.find(&pat)? + pat.len();
        let rest = &svg[start..];
        let end = rest.find('"')?;
        let digits: String = rest[..end].chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse::<u32>().ok()
    };
    if let (Some(w), Some(h)) = (extract_attr("width"), extract_attr("height")) {
        return (w, h);
    }
    if let Some(vb_pos) = svg.find("viewBox=\"") {
        let rest = &svg[vb_pos + 9..];
        if let Some(end) = rest.find('"') {
            let parts: Vec<f64> = rest[..end]
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<f64>().ok())
                .collect();
            if parts.len() == 4 {
                return (parts[2].max(0.0) as u32, parts[3].max(0.0) as u32);
            }
        }
    }
    (800, 600)
}

pub fn generate_synthetic_bmp(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
    let w = width.clamp(1, 2048);
    let h = height.clamp(1, 2048);
    let row_stride = ((w * 3 + 3) / 4) * 4;
    let pixel_bytes = row_stride * h;
    let file_size = 54 + pixel_bytes;
    let mut buf = Vec::with_capacity(file_size as usize);
    buf.extend_from_slice(b"BM");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&54u32.to_le_bytes());
    buf.extend_from_slice(&40u32.to_le_bytes());
    buf.extend_from_slice(&(w as i32).to_le_bytes());
    buf.extend_from_slice(&(h as i32).to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&24u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&pixel_bytes.to_le_bytes());
    buf.extend_from_slice(&2835u32.to_le_bytes());
    buf.extend_from_slice(&2835u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    for _y in 0..h {
        for _x in 0..w {
            buf.push(b);
            buf.push(g);
            buf.push(r);
        }
        for _pad in (w * 3)..row_stride {
            buf.push(0);
        }
    }
    buf
}

pub fn generate_synthetic_svg(width: u32, height: u32, title: &str) -> String {
    let safe_title: String = title
        .chars()
        .filter(|c| *c != '<' && *c != '>' && *c != '"' && *c != '&')
        .take(80)
        .collect();
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\"><rect width=\"100%\" height=\"100%\" fill=\"#0f172a\"/><text x=\"50%\" y=\"50%\" dominant-baseline=\"middle\" text-anchor=\"middle\" fill=\"#38bdf8\" font-family=\"monospace\" font-size=\"20\">{t}</text></svg>",
        w = width.max(16),
        h = height.max(16),
        t = if safe_title.is_empty() { "HEAV3NS-MICRO" } else { &safe_title }
    )
}

pub fn validate_navigation_url(url: &str) -> Result<String, String> {
    if url.chars().any(|c| (c as u32) <= 0x1F || (c as u32) == 0x7F) {
        return Err("Blocked URL containing control characters (WHATWG authority ambiguity)".to_string());
    }
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("Empty navigation URL".to_string());
    }
    let normalized_dots: String = trimmed
        .chars()
        .map(|c| if matches!(c, '\u{3002}' | '\u{FF0E}' | '\u{FF61}') { '.' } else { c })
        .collect();
    let lower = normalized_dots.to_ascii_lowercase();
    for blocked_scheme in ["javascript:", "vbscript:", "data:text/html", "chrome://", "chrome-extension://", "view-source:"] {
        if lower.starts_with(blocked_scheme) {
            return Err(format!("Blocked unsafe URL scheme: {}", blocked_scheme));
        }
    }
    for secret_prefix in ["sk-ant-", "sk-proj-", "ghp_", "gho_", "akia", "aiza", "xoxb-", "xoxp-"] {
        if lower.contains(secret_prefix) {
            return Err("Blocked URL containing embedded API key or secret token (exfiltration guard)".to_string());
        }
    }
    if lower.starts_with("file://") {
        let sensitive = ["/etc/", "/root/", "/var/run/", "/proc/", "/sys/", "/private/etc/", ".ssh", ".aws", ".env", "id_rsa"];
        if sensitive.iter().any(|s| lower.contains(s)) {
            return Err("Blocked sensitive local file:// path".to_string());
        }
        return Ok(trimmed.to_string());
    }
    if !lower.starts_with("http://") && !lower.starts_with("https://") && !lower.starts_with("about:blank") {
        return Err("URL must use http://, https://, file://, or about:blank".to_string());
    }
    if let Some(after_scheme) = lower.strip_prefix("http://").or_else(|| lower.strip_prefix("https://")) {
        let authority_end = after_scheme.find(|c| matches!(c, '/' | '?' | '#')).unwrap_or(after_scheme.len());
        let authority = &after_scheme[..authority_end];
        if authority.contains('\\') {
            return Err("Blocked URL with backslash in authority (WHATWG parser differential guard)".to_string());
        }
        if authority.contains('@') {
            return Err("Blocked URL with userinfo '@' in authority (host-spoofing guard)".to_string());
        }
        if authority.contains('%') {
            return Err("Blocked URL with percent-encoded hostname (SSRF obfuscation guard)".to_string());
        }
    }
    // Cloud metadata & SSRF protection (AWS/GCP/Alibaba/Azure IMDSv1/v2 + IPv4-mapped IPv6 + CGNAT)
    let ssrf_hosts = [
        "169.254.169.254",
        "169.254.169.253",
        "169.254.170.2",
        "metadata.google.internal",
        "metadata.goog",
        "100.100.100.200",
        "100.64.",
        "fd00:ec2::254",
        "[fd00:ec2::254]",
        "::ffff:169.254.169.254",
        "::ffff:0:169.254.169.254",
        "0:0:0:0:0:ffff:a9fe:a9fe",
        "0.0.0.0",
    ];
    for host in ssrf_hosts {
        if lower.contains(host) {
            return Err(format!("Blocked cloud metadata / SSRF endpoint: {}", host));
        }
    }
    Ok(trimmed.to_string())
}

/// Pre-screens JavaScript expressions for embedded URLs targeting cloud metadata, private networks,
/// or WHATWG-ambiguous authorities before `browser_use` `eval` executes (`expression_targets_private_url`).
pub fn expression_targets_blocked_url(expr: &str) -> Option<String> {
    let lower = expr.to_ascii_lowercase();
    for scheme in ["http://", "https://", "file://"] {
        let mut search_from = 0usize;
        while let Some(pos) = lower[search_from..].find(scheme) {
            let start = search_from + pos;
            let rest = &expr[start..];
            let end = rest
                .find(|c: char| matches!(c, '"' | '\'' | '`' | ')' | ' ' | '\n' | '\r' | '\t' | '>' | ';'))
                .unwrap_or(rest.len());
            let candidate = &rest[..end];
            if let Err(reason) = validate_navigation_url(candidate) {
                return Some(reason);
            }
            let cand_lower = candidate.to_ascii_lowercase();
            for priv_host in ["127.0.0.1", "localhost", "[::1]", "10.", "192.168.", "172.16."] {
                if cand_lower.contains(&format!("://{}", priv_host)) {
                    return Some(format!("Blocked private/loopback URL literal in JS eval: {}", candidate));
                }
            }
            search_from = start + end.max(scheme.len());
        }
    }
    None
}

pub fn redact_cdp_url(url: &str) -> String {
    let mut out = url.to_string();
    for key in ["token", "api_key", "apikey", "secret", "password", "access_token", "session_key", "auth"] {
        for sep in ["?", "&"] {
            let pat = format!("{}{}=", sep, key);
            if let Some(pos) = out.to_ascii_lowercase().find(&pat) {
                let val_start = pos + pat.len();
                let val_end = out[val_start..].find('&').map(|i| val_start + i).unwrap_or(out.len());
                out.replace_range(val_start..val_end, "[REDACTED]");
            }
        }
    }
    out
}

pub fn sanitize_trojan_unicode(text: &str) -> String {
    text.chars()
        .filter(|c| {
            let code = *c as u32;
            !matches!(
                code,
                0x200B..=0x200F // Zero-width & LRM/RLM
                | 0x202A..=0x202E // Bidi embedding/override (Trojan Source)
                | 0x2060..=0x2069 // Word joiner & directional isolates
                | 0xFEFF // BOM / zero-width no-break
            )
        })
        .collect()
}

pub fn snapshot_html_dom(html: &str, url: &str, max_elements: usize) -> Value {
    let sanitized = strip_heavy_html_blocks(html);
    let title = extract_tag_inner(&sanitized, "title").unwrap_or_else(|| "Untitled Document".to_string());
    let mut interactive = Vec::new();
    let mut som_id = 1usize;
    let limit = max_elements.clamp(1, 250);

    // Lightweight deterministic Set-of-Marks (SoM) scanner over HTML tags
    let bytes = sanitized.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && interactive.len() < limit {
        if bytes[i] == b'<' {
            if let Some(rel_end) = sanitized[i..].find('>') {
                let tag_slice = &sanitized[i + 1..i + rel_end];
                let trimmed = tag_slice.trim();
                if !trimmed.starts_with('/') && !trimmed.starts_with('!') && !trimmed.starts_with('?') {
                    let tag_name: String = trimmed
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                        .collect::<String>()
                        .to_ascii_uppercase();
                    let role = extract_html_attr(trimmed, "role").unwrap_or_default();
                    let is_interactive = matches!(
                        tag_name.as_str(),
                        "A" | "BUTTON" | "INPUT" | "SELECT" | "TEXTAREA" | "SUMMARY" | "DETAILS" | "OPTION"
                    ) || matches!(
                        role.as_str(),
                        "button" | "link" | "textbox" | "checkbox" | "menuitem" | "tab" | "switch" | "combobox"
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
        loop {
            let lower = current.to_ascii_lowercase();
            if let Some(start) = lower.find(&open_pat) {
                if let Some(end_rel) = lower[start..].find(&close_pat) {
                    let end = start + end_rel + close_pat.len();
                    current.replace_range(start..end, " ");
                    continue;
                }
            }
            break;
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
        if let Some(pos) = lower.find(&pat) {
            let start = pos + pat.len();
            let rest = &tag_body[start..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].trim().to_string());
            }
        }
    }
    None
}

fn extract_following_text(after_open_tag: &str) -> String {
    let end = after_open_tag.find('<').unwrap_or_else(|| after_open_tag.len().min(120));
    collapse_whitespace(&after_open_tag[..end.min(120)])
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

pub fn parse_coordinate_value(val: Option<&Value>, dim: u32, default: i32, is_norm_1000: bool) -> i32 {
    let parsed: Option<f64> = match val {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if let Some(pct) = trimmed.strip_suffix('%') {
                pct.trim().parse::<f64>().ok().map(|p| (p / 100.0) * (dim as f64))
            } else {
                trimmed.parse::<f64>().ok()
            }
        }
        _ => None,
    };

    match parsed {
        Some(f) if f.is_finite() => {
            if f > 0.0 && f <= 1.0 {
                (f * (dim as f64)).round() as i32
            } else if is_norm_1000 && f > 1.0 && f <= 1000.0 {
                ((f * (dim as f64)) / 1000.0).round() as i32
            } else {
                f.round() as i32
            }
        }
        _ => default,
    }
}

pub fn parse_scroll_deltas(args: &Value) -> (i32, i32) {
    let dir = args.get("direction").and_then(|v| v.as_str()).map(|s| s.trim().to_ascii_lowercase());
    let amount = args.get("amount").or_else(|| args.get("clicks")).or_else(|| args.get("steps"))
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(2) as i32;
    let step = amount * 120;
    if let Some(d) = dir {
        match d.as_str() {
            "up" => (0, step),
            "down" => (0, -step),
            "left" => (step, 0),
            "right" => (-step, 0),
            _ => (0, -step),
        }
    } else {
        let dx = args.get("delta_x").or_else(|| args.get("dx"))
            .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
            .unwrap_or(0) as i32;
        let dy = args.get("delta_y").or_else(|| args.get("dy"))
            .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
            .unwrap_or(-240) as i32;
        (dx, dy)
    }
}

pub fn parse_drag_points(args: &Value, width: u32, height: u32, cur: [i32; 2], is_norm_1000: bool) -> ([i32; 2], [i32; 2]) {
    let (fx, fy) = if let Some(arr) = args.get("from").and_then(|v| v.as_array()) {
        if arr.len() >= 2 {
            (parse_coordinate_value(arr.get(0), width, cur[0], is_norm_1000),
             parse_coordinate_value(arr.get(1), height, cur[1], is_norm_1000))
        } else {
            (cur[0], cur[1])
        }
    } else {
        (parse_coordinate_value(args.get("from_x"), width, cur[0], is_norm_1000),
         parse_coordinate_value(args.get("from_y"), height, cur[1], is_norm_1000))
    };

    let (tx, ty) = if let Some(arr) = args.get("to").and_then(|v| v.as_array()) {
        if arr.len() >= 2 {
            (parse_coordinate_value(arr.get(0), width, fx + 100, is_norm_1000),
             parse_coordinate_value(arr.get(1), height, fy + 100, is_norm_1000))
        } else {
            (fx + 100, fy + 100)
        }
    } else {
        (parse_coordinate_value(args.get("to_x").or_else(|| args.get("x")), width, fx + 100, is_norm_1000),
         parse_coordinate_value(args.get("to_y").or_else(|| args.get("y")), height, fy + 100, is_norm_1000))
    };

    ([fx.clamp(0, width as i32), fy.clamp(0, height as i32)],
     [tx.clamp(0, width as i32), ty.clamp(0, height as i32)])
}

pub fn extract_key_sequence(args: &Value) -> String {
    if let Some(arr) = args.get("keys").and_then(|v| v.as_array()) {
        arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("+")
    } else {
        args.get("keys").or_else(|| args.get("key")).or_else(|| args.get("text")).and_then(|v| v.as_str()).unwrap_or("Return").to_string()
    }
}

pub fn is_blocked_key_combo(combo: &str) -> bool {
    let norm = combo.trim().to_ascii_lowercase().replace(' ', "");
    matches!(
        norm.as_str(),
        "cmd+shift+backspace" | "command+shift+backspace" | "cmd+option+backspace" | "cmd+alt+backspace"
        | "cmd+ctrl+q" | "command+control+q" | "cmd+shift+q" | "command+shift+q"
        | "win+l" | "super+l" | "ctrl+alt+del" | "ctrl+alt+delete" | "option+f4" | "alt+f4"
    )
}

pub fn is_blocked_type_pattern(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    ["rm -rf /", ":(){:|:&};:", "mkfs", "| bash", "| sh", "| zsh"].iter().any(|p| lower.contains(p))
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

