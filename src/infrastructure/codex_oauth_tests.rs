// [LAYER: INFRASTRUCTURE]
use crate::domain::types::runtime::CancellationToken;
use crate::plumbing::hash_and_math::now_ms;
use serde_json::json;
use std::fs;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use crate::infrastructure::codex_oauth::*;

#[cfg(test)]
mod tests {
    use crate::infrastructure::codex_oauth::*;
    use crate::infrastructure::codex_oauth_http::{parse_token_response, wait_for_oauth_callback};
    use super::*;
    use crate::plumbing::oauth_pkce::encode_base64url;

    fn sample_jwt(email: &str, account_id: &str, plan: &str) -> String {
        let header = encode_base64url(br#"{"alg":"none"}"#);
        let payload = json!({
            "email": email,
            "exp": 2_000_000_000u64,
            "https://api.openai.com/auth": { "chatgpt_account_id": account_id, "chatgpt_plan_type": plan }
        });
        format!(
            "{header}.{}.sig",
            encode_base64url(payload.to_string().as_bytes())
        )
    }

    #[test]
    fn test_account_store_lifecycle_and_rpc_projection() {
        let dir = std::env::temp_dir().join(format!("heav3ns-acct-test-{}", now_ms()));
        let store = CodexAccountStore::at_path(dir.join("accounts.json"));
        let empty = store.rpc_snapshot();
        assert_eq!(empty["saved"], json!(false));
        assert_eq!(empty["accounts"], json!([]));

        let jwt1 = sample_jwt("one@example.com", "acct_1", "pro");
        let jwt2 = sample_jwt("two@example.com", "acct_2", "plus");
        let c1 = parse_token_response(
            &json!({ "access_token": jwt1, "refresh_token": "rt_1" }),
            None,
        )
        .unwrap();
        let c2 = parse_token_response(
            &json!({ "access_token": jwt2, "refresh_token": "rt_2" }),
            None,
        )
        .unwrap();

        let mut data = store.load_accounts_store();
        assert!(upsert_account(&mut data, c1));
        assert!(upsert_account(&mut data, c2));
        store.save_accounts_store(&data).unwrap();

        let snap = store.rpc_snapshot();
        assert_eq!(snap["saved"], json!(true));
        assert_eq!(snap["accounts"].as_array().unwrap().len(), 2);
        assert_eq!(snap["account"]["id"], json!("acct_2"));
        // Tokens must never be leaked in RPC snapshots.
        assert!(!snap.to_string().contains("rt_1"));
        assert!(!snap.to_string().contains("rt_2"));

        let switched = store.switch_account("acct_1").unwrap();
        assert_eq!(switched["account"]["id"], json!("acct_1"));

        let renamed = store.rename_account("acct_1", "Work Pro").unwrap();
        assert_eq!(renamed["account"]["label"], json!("Work Pro"));

        let auto_off = store.set_auto_switch(false).unwrap();
        assert_eq!(auto_off["autoSwitch"], json!(false));

        let removed = store.remove_account("acct_1").unwrap();
        assert_eq!(removed["accounts"].as_array().unwrap().len(), 1);
        assert_eq!(removed["account"]["id"], json!("acct_2"));

        let signed_out = store.sign_out(true).unwrap();
        assert_eq!(signed_out["saved"], json!(false));
        assert_eq!(signed_out["accounts"], json!([]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_loopback_oauth_callback_receives_code_and_validates_state() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let cancel = CancellationToken::new();
        let task = tokio::spawn(async move {
            wait_for_oauth_callback(listener, "expected_state_123", &cancel).await
        });

        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        stream.write_all(b"GET /auth/callback?code=auth_code_xyz&state=expected_state_123 HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();
        let mut resp = vec![0u8; 1024];
        let n = stream.read(&mut resp).await.unwrap();
        let text = String::from_utf8_lossy(&resp[..n]);
        assert!(text.contains("200 OK") && text.contains("You&#39;re connected"));
        assert_eq!(task.await.unwrap().unwrap(), "auth_code_xyz");
    }

    static ENV_TEST_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

    #[tokio::test]
    async fn test_single_flight_refresh_deduplication_and_429_auto_switch() {
        let _env_guard = ENV_TEST_LOCK
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await;
        let dir = std::env::temp_dir().join(format!("heav3ns-rtr-test-{}", now_ms()));
        let acct_path = dir.join("accounts.json");
        let store = CodexAccountStore::at_path(&acct_path);

        let stale_jwt = sample_jwt("one@example.com", "acct_1", "pro");
        let fresh_jwt = sample_jwt("one@example.com", "acct_1", "pro");
        let backup_jwt = sample_jwt("two@example.com", "acct_2", "plus");

        let mut c1 = parse_token_response(
            &json!({ "access_token": stale_jwt, "refresh_token": "rt_1" }),
            None,
        )
        .unwrap();
        c1.expires_at_ms = Some(1_000); // expired
        c1.access_token = "stale_access_token".to_string();
        let c2 = parse_token_response(
            &json!({ "access_token": backup_jwt, "refresh_token": "rt_2" }),
            None,
        )
        .unwrap();

        let mut data = store.load_accounts_store();
        upsert_account(&mut data, c2);
        upsert_account(&mut data, c1); // acct_1 is active
        store.save_accounts_store(&data).unwrap();

        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hits_srv = std::sync::Arc::clone(&hits);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let fresh_for_srv = fresh_jwt.clone();

        let srv = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf).await;
                hits_srv.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(40)).await;
                let body = json!({
                    "access_token": fresh_for_srv,
                    "refresh_token": "rt_1_rotated",
                    "expires_in": 3600
                })
                .to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            }
        });

        std::env::set_var(
            "HEAV3NS_OPENAI_TOKEN_URL",
            format!("http://127.0.0.1:{port}/oauth/token"),
        );
        let mut tasks = Vec::new();
        for _ in 0..5 {
            let s = store.clone();
            tasks.push(tokio::spawn(async move {
                s.get_valid_active_credentials().await
            }));
        }
        for t in tasks {
            let (tok, acct) = t.await.unwrap().unwrap().unwrap();
            assert_eq!(tok, fresh_jwt);
            assert_eq!(acct.as_deref(), Some("acct_1"));
        }
        assert_eq!(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "Single-flight refresh must coalesce concurrent callers"
        );

        // Verify 429 rate-limit auto-switches to acct_2
        store.record_rate_limit(Some(60_000));
        let snap = store.rpc_snapshot();
        assert_eq!(snap["account"]["id"], json!("acct_2"));

        std::env::remove_var("HEAV3NS_OPENAI_TOKEN_URL");
        srv.abort();
        let _ = fs::remove_dir_all(&dir);
    }
}
