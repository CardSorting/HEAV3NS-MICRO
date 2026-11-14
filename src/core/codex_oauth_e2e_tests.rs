// [LAYER: CORE]
use std::fs;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::domain::interfaces::ILlmProvider;
use crate::domain::types::account_types::{ChatGptAccountCredentials, ChatGptAccountStoreData};
use crate::domain::types::messages::{Role, SessionMessage};
use crate::domain::types::tick_result::ProviderRequest;
use crate::infrastructure::codex_oauth::CodexAccountStore;
use crate::infrastructure::http_provider::HttpProvider;
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::oauth_pkce::encode_base64url;

fn sample_jwt(email: &str, account_id: &str, plan: &str) -> String {
    let header = encode_base64url(br#"{"alg":"none"}"#);
    let payload = json!({
        "email": email,
        "exp": 2_000_000_000u64,
        "https://api.openai.com/auth": {
            "chatgpt_account_id": account_id,
            "chatgpt_plan_type": plan
        }
    });
    let body = encode_base64url(payload.to_string().as_bytes());
    format!("{header}.{body}.sig")
}

#[tokio::test]
async fn test_http_provider_codex_oauth_401_refresh_and_sse_response() {
    let dir = std::env::temp_dir().join(format!("heav3ns-oauth-e2e-{}", now_ms()));
    let acct_path = dir.join("accounts.json");
    let store = CodexAccountStore::at_path(&acct_path);

    let fresh_jwt = sample_jwt("pilot@example.com", "acct_live", "pro");
    let data = ChatGptAccountStoreData {
        version: 1,
        active_id: Some("acct_live".to_string()),
        auto_switch: true,
        accounts: vec![ChatGptAccountCredentials {
            id: "acct_live".to_string(),
            label: "pilot@example.com".to_string(),
            email: Some("pilot@example.com".to_string()),
            plan_type: Some("pro".to_string()),
            account_id: Some("acct_live".to_string()),
            access_token: "revoked_token".to_string(),
            refresh_token: Some("rt_init".to_string()),
            id_token: None,
            expires_at_ms: Some(now_ms() + 3_600_000),
            limited_until_ms: None,
            limit_estimated: false,
            updated_at_ms: now_ms(),
        }],
    };
    store.save_accounts_store(&data).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let fresh_for_srv = fresh_jwt.clone();

    let srv = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let first_line = req.lines().next().unwrap_or("");
            if first_line.contains("/oauth/token") {
                let body = json!({
                    "access_token": fresh_for_srv,
                    "refresh_token": "rt_new",
                    "expires_in": 3600
                })
                .to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            } else if req.contains("Bearer revoked_token") {
                let body = r#"{"error":"unauthorized"}"#;
                let resp = format!(
                    "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            } else {
                assert!(req.to_ascii_lowercase().contains("chatgpt-account-id: acct_live"));
                let sse = "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"OAuth E2E OK\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":4}}}\n\n";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{sse}",
                    sse.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            }
        }
    });

    std::env::set_var("HEAV3NS_ACCOUNTS_PATH", &acct_path);
    std::env::set_var("HEAV3NS_OPENAI_TOKEN_URL", format!("http://127.0.0.1:{port}/oauth/token"));
    std::env::set_var("HEAV3NS_CODEX_BASE_URL", format!("http://127.0.0.1:{port}/backend-api/codex"));

    let provider = HttpProvider::new("openai-codex", "https://api.openai.com/v1", None, 5_000);
    let req = ProviderRequest {
        model: "gpt-5.4".to_string(),
        messages: vec![SessionMessage::new(Role::User, "hello")],
        tools: Vec::new(),
        temperature: 0.2,
        max_tokens: 128,
        reasoning_effort: None,
    };
    let res = provider.complete(req, None).await.unwrap();
    assert_eq!(res.content.as_deref(), Some("OAuth E2E OK"));

    std::env::remove_var("HEAV3NS_ACCOUNTS_PATH");
    std::env::remove_var("HEAV3NS_OPENAI_TOKEN_URL");
    std::env::remove_var("HEAV3NS_CODEX_BASE_URL");
    srv.abort();
    let _ = fs::remove_dir_all(&dir);
}
