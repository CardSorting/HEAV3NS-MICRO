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
        return Err(format!(
            "Image exceeds max budget of {} bytes",
            MAX_IMAGE_PAYLOAD_BYTES
        ));
    }

    // 1. PNG: 89 50 4E 47 0D 0A 1A 0A + IHDR @ 16..24
    if bytes.len() >= 24 && bytes[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        let bit_depth = if bytes.len() > 24 { bytes[24] } else { 8 };
        let color_type = if bytes.len() > 25 { bytes[25] } else { 6 };
        let has_alpha = color_type == 4 || color_type == 6;
        return Ok(build_img_meta(
            "png",
            "image/png",
            width,
            height,
            bytes.len(),
            has_alpha,
            bit_depth,
            false,
        ));
    }

    // 2. GIF: GIF87a / GIF89a
    if bytes.len() >= 10 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a") {
        let width = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let height = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        let animated = bytes
            .windows(3)
            .filter(|w| *w == [0x21, 0xF9, 0x04])
            .count()
            > 1;
        return Ok(build_img_meta(
            "gif",
            "image/gif",
            width,
            height,
            bytes.len(),
            true,
            8,
            animated,
        ));
    }

    // 3. BMP: 'BM' + width @ 18..22, height @ 22..26
    if bytes.len() >= 26 && &bytes[0..2] == b"BM" {
        let width = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]).unsigned_abs();
        let height =
            i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]).unsigned_abs();
        let bpp = if bytes.len() >= 30 {
            u16::from_le_bytes([bytes[28], bytes[29]]) as u8
        } else {
            24
        };
        return Ok(build_img_meta(
            "bmp",
            "image/bmp",
            width,
            height,
            bytes.len(),
            bpp == 32,
            bpp,
            false,
        ));
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
            let h = ((b1 >> 6) | (b2 << 2) | ((b3 & 0x0F) << 10)) + 1;
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
        return Ok(build_img_meta(
            "webp",
            "image/webp",
            width,
            height,
            bytes.len(),
            alpha,
            8,
            false,
        ));
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
        return Ok(build_img_meta(
            "jpeg",
            "image/jpeg",
            width,
            height,
            bytes.len(),
            false,
            bit_depth,
            false,
        ));
    }

    // 6. SVG: XML/SVG text header
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let trimmed = text.trim_start();
    if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && trimmed.contains("<svg")) {
        let (w, h) = parse_svg_dimensions(trimmed);
        return Ok(build_img_meta(
            "svg",
            "image/svg+xml",
            w,
            h,
            bytes.len(),
            true,
            8,
            false,
        ));
    }

    Err("Unrecognized binary image header (expected PNG, JPEG, GIF, WebP, BMP, or SVG)".to_string())
}

fn build_img_meta(
    fmt: &str,
    mime: &str,
    width: u32,
    height: u32,
    byte_size: usize,
    has_alpha: bool,
    bit_depth: u8,
    animated: bool,
) -> Value {
    let aspect = calculate_aspect_ratio(width, height);
    let megapixels = ((width as f64 * height as f64) / 1_000_000.0 * 1000.0).round() / 1000.0;
    // Vision token heuristic (~85 base + 170 per 512x512 tile)
    let tiles_x = ((width.max(1) as u64).saturating_add(511) / 512).max(1);
    let tiles_y = ((height.max(1) as u64).saturating_add(511) / 512).max(1);
    let estimated_vision_tokens =
        85u64.saturating_add(tiles_x.saturating_mul(tiles_y).saturating_mul(170));
    json!({
        "format": fmt, "mimeType": mime, "width": width, "height": height,
        "aspectRatio": aspect, "megapixels": megapixels, "byteSize": byte_size,
        "hasAlpha": has_alpha, "bitDepth": bit_depth, "animated": animated,
        "estimatedVisionTokens": estimated_vision_tokens
    })
}

fn parse_svg_dimensions(svg: &str) -> (u32, u32) {
    let extract_attr = |attr: &str| -> Option<u32> {
        let pat = format!("{}=\"", attr);
        let pos = svg
            .match_indices(&pat)
            .find(|(p, _)| *p == 0 || svg.as_bytes()[p - 1].is_ascii_whitespace())?
            .0;
        let rest = &svg[pos + pat.len()..];
        let end = rest.find('"')?;
        let digits: String = rest[..end]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
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
    if url
        .chars()
        .any(|c| (c as u32) <= 0x1F || (c as u32) == 0x7F)
    {
        return Err(
            "Blocked URL containing control characters (WHATWG authority ambiguity)".to_string(),
        );
    }
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("Empty navigation URL".to_string());
    }
    let normalized_dots: String = trimmed
        .chars()
        .map(|c| {
            if matches!(c, '\u{3002}' | '\u{FF0E}' | '\u{FF61}') {
                '.'
            } else {
                c
            }
        })
        .collect();
    let lower = normalized_dots.to_ascii_lowercase();
    for blocked_scheme in [
        "javascript:",
        "vbscript:",
        "data:text/html",
        "chrome://",
        "chrome-extension://",
        "view-source:",
    ] {
        if lower.starts_with(blocked_scheme) {
            return Err(format!("Blocked unsafe URL scheme: {}", blocked_scheme));
        }
    }
    for secret_prefix in [
        "sk-ant-", "sk-proj-", "ghp_", "gho_", "akia", "aiza", "xoxb-", "xoxp-",
    ] {
        if lower.contains(secret_prefix) {
            return Err(
                "Blocked URL containing embedded API key or secret token (exfiltration guard)"
                    .to_string(),
            );
        }
    }
    if lower.starts_with("file://") {
        let decoded = percent_decode_ascii(&lower).replace('\\', "/");
        if decoded.contains("/../") || decoded.ends_with("/..") {
            return Err("Blocked path traversal in file:// URL".to_string());
        }
        let path_part = decoded
            .strip_prefix("file://localhost")
            .or_else(|| decoded.strip_prefix("file://"))
            .unwrap_or(&decoded);
        let sensitive = [
            "/etc/",
            "/root/",
            "/var/run/",
            "/proc/",
            "/sys/",
            "/private/etc/",
        ];
        let cred_hit =
            crate::plumbing::secret_redactor::DeterministicSecretRedactor::classify_sensitive_path(
                path_part,
                crate::plumbing::secret_redactor::CredentialDenyScope::ModelContext,
            )
            .is_some();
        if path_part == "/etc"
            || path_part == "/private/etc"
            || sensitive.iter().any(|s| decoded.contains(s))
            || cred_hit
        {
            return Err("Blocked sensitive local file:// path".to_string());
        }
        return Ok(trimmed.to_string());
    }
    if !lower.starts_with("http://")
        && !lower.starts_with("https://")
        && !lower.starts_with("about:blank")
    {
        return Err("URL must use http://, https://, file://, or about:blank".to_string());
    }
    if let Some(after_scheme) = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))
    {
        let authority_end = after_scheme
            .find(|c| matches!(c, '/' | '?' | '#'))
            .unwrap_or(after_scheme.len());
        let authority = &after_scheme[..authority_end];
        if authority.contains('\\') {
            return Err(
                "Blocked URL with backslash in authority (WHATWG parser differential guard)"
                    .to_string(),
            );
        }
        if authority.contains('@') {
            return Err(
                "Blocked URL with userinfo '@' in authority (host-spoofing guard)".to_string(),
            );
        }
        if authority.contains('%') {
            return Err(
                "Blocked URL with percent-encoded hostname (SSRF obfuscation guard)".to_string(),
            );
        }
        let host = extract_authority_host(authority);
        if is_blocked_ssrf_host(host) {
            return Err(format!(
                "Blocked cloud metadata / private / loopback SSRF endpoint: {}",
                host
            ));
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

fn percent_decode_ascii(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex_str) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                    out.push(val);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn extract_authority_host(authority: &str) -> &str {
    if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest)
    } else {
        authority.split(':').next().unwrap_or(authority)
    }
}

fn parse_nonstandard_octet(part: &str) -> Option<u64> {
    if part.is_empty() {
        return None;
    }
    if let Some(hex) = part.strip_prefix("0x").or_else(|| part.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    if part.len() > 1 && part.starts_with('0') {
        return u64::from_str_radix(part, 8).ok();
    }
    part.parse::<u64>().ok()
}

fn parse_nonstandard_ipv4(host: &str) -> Option<std::net::Ipv4Addr> {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }
    let mut vals = Vec::with_capacity(parts.len());
    for p in &parts {
        vals.push(parse_nonstandard_octet(p)?);
    }
    for &v in &vals[..vals.len() - 1] {
        if v > 255 {
            return None;
        }
    }
    let last = vals[vals.len() - 1];
    let addr = match vals.len() {
        1 => u32::try_from(last).ok()?,
        2 => {
            if last > 0x00FF_FFFF {
                return None;
            }
            ((vals[0] as u32) << 24) | (last as u32)
        }
        3 => {
            if last > 0xFFFF {
                return None;
            }
            ((vals[0] as u32) << 24) | ((vals[1] as u32) << 16) | (last as u32)
        }
        4 => {
            if last > 255 {
                return None;
            }
            ((vals[0] as u32) << 24)
                | ((vals[1] as u32) << 16)
                | ((vals[2] as u32) << 8)
                | (last as u32)
        }
        _ => return None,
    };
    Some(std::net::Ipv4Addr::from(addr))
}

fn is_blocked_ipv4(v4: std::net::Ipv4Addr) -> bool {
    let o = v4.octets();
    v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local()
        || v4.is_unspecified()
        || o[0] == 0
        || v4.is_broadcast()
        || v4.is_multicast()
        || (o[0] == 100 && (64..=127).contains(&o[1]))
}

fn is_blocked_ssrf_host(host: &str) -> bool {
    let host = host.trim_end_matches('.');
    if host.is_empty()
        || matches!(
            host,
            "localhost" | "metadata.google.internal" | "metadata.goog"
        )
        || host.ends_with(".localhost")
        || host.ends_with(".internal")
    {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => is_blocked_ipv4(v4),
            std::net::IpAddr::V6(v6) => {
                if let Some(mapped) = v6.to_ipv4_mapped().or_else(|| v6.to_ipv4()) {
                    return is_blocked_ipv4(mapped);
                }
                let seg = v6.segments();
                if matches!(
                    seg[0..6],
                    [0, 0, 0, 0, 0xffff, 0] | [0x0064, 0xff9b, 0, 0, 0, 0]
                ) {
                    return is_blocked_ipv4(std::net::Ipv4Addr::from(
                        ((seg[6] as u32) << 16) | (seg[7] as u32),
                    ));
                }
                v6.is_loopback()
                    || v6.is_unspecified()
                    || (seg[0] & 0xfe00 == 0xfc00)
                    || (seg[0] & 0xffc0 == 0xfe80)
            }
        };
    }
    parse_nonstandard_ipv4(host).is_some_and(is_blocked_ipv4)
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
                .find(|c: char| {
                    matches!(
                        c,
                        '"' | '\'' | '`' | ')' | ' ' | '\n' | '\r' | '\t' | '>' | ';'
                    )
                })
                .unwrap_or(rest.len());
            let candidate = &rest[..end];
            if let Err(reason) = validate_navigation_url(candidate) {
                return Some(reason);
            }
            let cand_lower = candidate.to_ascii_lowercase();
            for priv_host in [
                "127.0.0.1",
                "localhost",
                "[::1]",
                "10.",
                "192.168.",
                "172.16.",
            ] {
                if cand_lower.contains(&format!("://{}", priv_host)) {
                    return Some(format!(
                        "Blocked private/loopback URL literal in JS eval: {}",
                        candidate
                    ));
                }
            }
            search_from = start + end.max(scheme.len());
        }
    }
    None
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


pub use crate::plumbing::dom_and_cua::*;
