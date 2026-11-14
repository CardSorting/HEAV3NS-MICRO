// [LAYER: PLUMBING]
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Encodes bytes as unpadded URL-safe Base64 (RFC 4648 §5).
pub fn encode_base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) & 0x3f] as char);
        out.push(TABLE[(n >> 12) & 0x3f] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(n >> 6) & 0x3f] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[n & 0x3f] as char);
        }
    }
    out
}

/// Decodes URL-safe or standard Base64 (with or without padding).
pub fn decode_base64url(input: &str) -> Option<Vec<u8>> {
    fn val(b: u8) -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some((b - b'A') as u32),
            b'a'..=b'z' => Some((b - b'a' + 26) as u32),
            b'0'..=b'9' => Some((b - b'0' + 52) as u32),
            b'-' | b'+' => Some(62),
            b'_' | b'/' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = input
        .trim()
        .trim_end_matches('=')
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if clean.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity((clean.len() * 3) / 4);
    for chunk in clean.chunks(4) {
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        out.push(((a << 2) | (b >> 4)) as u8);
        if chunk.len() > 2 {
            let c = val(chunk[2])?;
            out.push((((b & 0x0f) << 4) | (c >> 2)) as u8);
            if chunk.len() > 3 {
                let d = val(chunk[3])?;
                out.push((((c & 0x03) << 6) | d) as u8);
            }
        }
    }
    Some(out)
}

/// Percent-encodes a string for `application/x-www-form-urlencoded` or URL query parameters.
pub fn url_encode_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Percent-decodes a URL query component (treating `+` as space).
pub fn url_decode_component(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
            out.push(bytes[i]);
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Encodes bytes as lowercase hexadecimal.
pub fn encode_hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Computes the S256 PKCE `code_challenge` from a `code_verifier`.
pub fn generate_pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    encode_base64url(&digest)
}

/// Builds the OpenAI OAuth authorization URL for `auth.openai.com`.
pub fn build_openai_authorize_url(
    auth_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &str,
    code_challenge: &str,
    state: &str,
) -> String {
    let pairs = [
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri),
        ("scope", scopes),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", "codex_cli"),
    ];
    let query: Vec<String> = pairs
        .iter()
        .map(|(k, v)| format!("{}={}", url_encode_component(k), url_encode_component(v)))
        .collect();
    format!("{auth_endpoint}?{}", query.join("&"))
}

/// Parses a URL query string (`k=v&...`) into decoded key/value pairs.
pub fn parse_query_params(query: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for pair in query.trim_start_matches('?').split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        map.insert(url_decode_component(k), url_decode_component(v));
    }
    map
}

/// Decodes the payload JSON of a JWT without verifying its signature.
pub fn parse_jwt_claims(token: &str) -> Option<Value> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let _sig = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let decoded = decode_base64url(payload)?;
    serde_json::from_slice(&decoded).ok()
}

/// Extracts the ChatGPT account/workspace ID from an OpenAI ID or access token.
pub fn extract_openai_account_id(token: &str) -> Option<String> {
    let claims = parse_jwt_claims(token)?;
    if let Some(id) = claims.get("https://api.openai.com/auth.chatgpt_account_id").and_then(Value::as_str) {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    if let Some(id) = claims
        .get("https://api.openai.com/auth")
        .and_then(|v| v.get("chatgpt_account_id"))
        .and_then(Value::as_str)
    {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    if let Some(id) = claims
        .get("chatgpt_account_id")
        .or_else(|| claims.get("account_id"))
        .and_then(Value::as_str)
    {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    claims
        .get("organizations")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(|org| org.get("id"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Extracts the user's email from an OpenAI ID or access token.
pub fn extract_openai_email(token: &str) -> Option<String> {
    let claims = parse_jwt_claims(token)?;
    claims
        .get("email")
        .and_then(Value::as_str)
        .or_else(|| {
            claims
                .get("https://api.openai.com/profile")
                .and_then(|v| v.get("email"))
                .and_then(Value::as_str)
        })
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Extracts the ChatGPT subscription plan type (`plus`, `pro`, `team`, etc.) from an OpenAI JWT.
pub fn extract_openai_plan_type(token: &str) -> Option<String> {
    let claims = parse_jwt_claims(token)?;
    claims
        .get("https://api.openai.com/auth")
        .and_then(|v| v.get("chatgpt_plan_type"))
        .and_then(Value::as_str)
        .or_else(|| claims.get("chatgpt_plan_type").and_then(Value::as_str))
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Extracts the JWT `exp` claim in milliseconds since Unix epoch.
pub fn extract_jwt_expiry_ms(token: &str) -> Option<u64> {
    let claims = parse_jwt_claims(token)?;
    let exp = claims.get("exp").and_then(Value::as_u64)?;
    (exp > 0).then(|| exp.saturating_mul(1_000))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_base64url_roundtrip_and_pkce() {
        let raw = b"hello-heav3ns-oauth-pkce";
        let enc = encode_base64url(raw);
        assert!(!enc.contains('='));
        assert_eq!(decode_base64url(&enc).as_deref(), Some(raw.as_slice()));
        let challenge = generate_pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn test_authorize_url_and_query_parsing() {
        let url = build_openai_authorize_url(
            "https://auth.openai.com/oauth/authorize",
            "app_test",
            "http://localhost:1455/auth/callback",
            "openid profile email offline_access",
            "chal123",
            "state456",
        );
        assert!(url.starts_with("https://auth.openai.com/oauth/authorize?"));
        let query = url.split_once('?').unwrap().1;
        let parsed = parse_query_params(query);
        assert_eq!(parsed.get("client_id").map(String::as_str), Some("app_test"));
        assert_eq!(parsed.get("redirect_uri").map(String::as_str), Some("http://localhost:1455/auth/callback"));
        assert_eq!(parsed.get("code_challenge").map(String::as_str), Some("chal123"));
        assert_eq!(parsed.get("state").map(String::as_str), Some("state456"));
    }

    #[test]
    fn test_jwt_claims_extraction() {
        let payload = json!({
            "email": "pilot@example.com",
            "exp": 1_900_000_000u64,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct_123",
                "chatgpt_plan_type": "pro"
            }
        });
        let header = encode_base64url(br#"{"alg":"RS256","typ":"JWT"}"#);
        let body = encode_base64url(payload.to_string().as_bytes());
        let jwt = format!("{header}.{body}.sig");
        assert_eq!(extract_openai_account_id(&jwt).as_deref(), Some("acct_123"));
        assert_eq!(extract_openai_email(&jwt).as_deref(), Some("pilot@example.com"));
        assert_eq!(extract_openai_plan_type(&jwt).as_deref(), Some("pro"));
        assert_eq!(extract_jwt_expiry_ms(&jwt), Some(1_900_000_000_000));
    }
}
