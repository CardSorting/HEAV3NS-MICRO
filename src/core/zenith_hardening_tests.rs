// [LAYER: CORE]
use crate::core::session_store::{SessionCompactor, SessionStore};
use crate::core::tool_registry::ToolRegistry;
use crate::domain::arena::ArenaAllocator;
use crate::domain::compaction::CompactionPolicy;
use crate::domain::memory::MemoryStore;
use crate::domain::pricing_and_taxonomy::{resolve_compatible_fallback_model_with_catalog, ProviderTokenUsage};
use crate::domain::tool_parser::ToolCallArgParser;
use crate::domain::types::messages::{Role, SessionMessage, ToolCall, ToolCallFunction};
use crate::domain::types::model_catalog::ModelPricingCatalog;
use crate::domain::types::posture_types::{evaluate_tool_posture, AuthorizationVerdict, ExecutionPosture, RepoConstitution};
use crate::domain::types::runtime::{CancellationToken, ExecutionAuthorityLevel};
use crate::domain::types::tool_schedule::ScheduledToolCall;
use crate::domain::types::tool_types::{ParamSchema, ParamType};
use crate::infrastructure::fs_tools::dispatch_fs_tool;
use crate::infrastructure::shell_runner::execute_shell_command_sandboxed;
use crate::infrastructure::transaction_journal::TransactionJournal;
use crate::infrastructure::workspace_cas::WorkspaceSnapshotCas;
use crate::domain::interfaces::IWorkspaceSnapshotCas;
use crate::plumbing::fuzzy_patch::heal_fuzzy_patch;
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::json_repair::repair_json_string;
use crate::plumbing::seatbelt_profile::{inspect_command_security, CommandSecurityVerdict, SeatbeltSandboxMode};
use crate::plumbing::secret_redactor::{DeterministicSecretRedactor, SafetyPolicy};
use serde_json::{json, Map};
use std::sync::{Arc, Mutex};

#[test]
fn test_sensitive_env_var_classification() {
    assert!(SafetyPolicy::is_sensitive_env_var("OPENAI_API_KEY"));
    assert!(SafetyPolicy::is_sensitive_env_var("anthropic_api_key"));
    assert!(SafetyPolicy::is_sensitive_env_var("AWS_SECRET_ACCESS_KEY"));
    assert!(SafetyPolicy::is_sensitive_env_var("GITHUB_TOKEN"));
    assert!(SafetyPolicy::is_sensitive_env_var("CUSTOM_AUTH_TOKEN"));
    assert!(!SafetyPolicy::is_sensitive_env_var("PATH"));
    assert!(!SafetyPolicy::is_sensitive_env_var("HOME"));
    assert!(!SafetyPolicy::is_sensitive_env_var("CARGO_MANIFEST_DIR"));
}

#[tokio::test]
async fn test_shell_runner_scrubs_sensitive_credential_env_vars() {
    std::env::set_var("HEAV3NS_TEST_API_KEY", "sk-secret-do-not-leak-999");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "aws-secret-do-not-leak-999");

    let cwd = std::env::current_dir().unwrap();
    let (out, _) = execute_shell_command_sandboxed(
        "printf '%s|%s|%s' \"$HEAV3NS_TEST_API_KEY\" \"$AWS_SECRET_ACCESS_KEY\" \"${PATH:+ok}\"",
        &cwd,
        5_000,
        SeatbeltSandboxMode::Unrestricted,
        None,
    )
    .await
    .unwrap();

    std::env::remove_var("HEAV3NS_TEST_API_KEY");
    std::env::remove_var("AWS_SECRET_ACCESS_KEY");

    assert_eq!(out["stdout"].as_str(), Some("||ok"));
}

#[test]
fn test_workspace_path_escape_and_redaction_hardening() {
    let dir = std::env::temp_dir().join(format!("heav3ns-zenith-ws-{}", now_ms()));
    std::fs::create_dir_all(&dir).unwrap();
    let journal = Mutex::new(TransactionJournal::default());

    let mut args = Map::new();
    args.insert("path".to_string(), json!("../outside_secret.txt"));
    let err = dispatch_fs_tool("view_file", &args, &dir, &journal).unwrap_err();
    assert!(err.contains("escapes workspace"), "Expected escape denial, got: {err}");

    let mut abs_args = Map::new();
    abs_args.insert("path".to_string(), json!("/etc/hosts"));
    let abs_err = dispatch_fs_tool("view_file", &abs_args, &dir, &journal).unwrap_err();
    assert!(abs_err.contains("escapes workspace"), "Expected abs escape denial, got: {abs_err}");

    assert!(SafetyPolicy::is_sensitive_env_var("SLACK_BOT_TOKEN"));
    assert!(SafetyPolicy::is_sensitive_env_var("DATABASE_URL"));
    let redacted = DeterministicSecretRedactor::redact("postgres://admin:supersecretpass@db.internal:5432/prod");
    assert!(!redacted.contains("supersecretpass"));
    assert!(redacted.contains("[REDACTED:url_auth]"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_json_repair_fuzzy_patch_arena_and_tool_parser_hardening() {
    // Valid JSON with braces inside string literal must not be corrupted
    let valid = r#"{"code": "fn main() { let x = [1;"}"#;
    assert_eq!(repair_json_string(valid), valid);

    // Fuzzy patch must match snippets containing blank lines
    let file = "fn alpha() {\n    let a = 1;\n\n    let b = 2;\n}\n";
    let snippet = "  let a = 1;\n\n  let b = 2;";
    assert!(heal_fuzzy_patch(file, snippet).is_some());

    // ArenaAllocator must reject overflow without panicking
    let mut arena = ArenaAllocator::new(64);
    assert!(arena.allocate_raw_bytes(usize::MAX).is_err());
    assert!(arena.read_str(usize::MAX - 2, 10).is_err());

    // ToolCallArgParser must reject NaN/Infinity and not false-alias cwd -> cmd
    let parser = ToolCallArgParser;
    let schema = &[
        ("cmd", ParamSchema::req(ParamType::String, "command")),
        ("limit", ParamSchema::req(ParamType::Number, "limit")),
    ];
    let mut raw_args = Map::new();
    raw_args.insert("cwd".to_string(), json!("/tmp"));
    raw_args.insert("limit".to_string(), json!("NaN"));
    let normalized = parser.normalize_aliases(raw_args, schema);
    assert!(!normalized.contains_key("cmd"), "cwd must not false-alias to cmd");
    let (_, validation) = parser.coerce_and_validate(normalized, schema);
    assert!(!validation.valid, "NaN string must fail number validation");
}

#[test]
fn test_posture_multipath_diff_and_seatbelt_readonly_hardening() {
    let law = RepoConstitution::parse_strict(
        r#"{"version": 1, "rules": [{"id": "no-secrets", "description": "protect secrets", "paths": ["secrets/*"], "action": "deny"}]}"#,
    );
    // Multi-path move_file with benign source and protected destination
    let mv_args = json!({ "source": "src/main.rs", "destination": "a/../secrets/key.txt" });
    let eval = evaluate_tool_posture(ExecutionPosture::AcceptEdits, "move_file", &mv_args, true, Some(&law), &[]);
    assert_eq!(eval.verdict, AuthorizationVerdict::Deny);

    // apply_unified_diff path extraction
    let diff_args = json!({ "diff": "--- a/.git/hooks/pre-commit\n+++ b/.git/hooks/pre-commit\n@@ -1 +1 @@\n-a\n+b\n" });
    let diff_eval = evaluate_tool_posture(ExecutionPosture::AcceptEdits, "apply_unified_diff", &diff_args, true, None, &[]);
    assert_eq!(diff_eval.verdict, AuthorizationVerdict::AskUser);
    assert!(diff_eval.session_rule_key.contains(".git/hooks/pre-commit"));

    // find -fprint must not be classified as ReadOnlySafe
    assert!(!matches!(
        inspect_command_security("find . -fprint /tmp/out.txt"),
        CommandSecurityVerdict::ReadOnlySafe
    ));
}

#[tokio::test]
async fn test_execute_batch_cancellation_emits_records_for_all_calls() {
    let mem = Arc::new(Mutex::new(MemoryStore::default()));
    let registry = ToolRegistry::new(mem);
    let cwd = std::env::current_dir().unwrap();
    let tok = CancellationToken::new();
    tok.cancel();

    let calls = vec![
        ScheduledToolCall { id: "c1".to_string(), name: "list_dir".to_string(), args: json!({}) },
        ScheduledToolCall { id: "c2".to_string(), name: "list_dir".to_string(), args: json!({}) },
    ];
    let records = registry
        .execute_batch(&calls, &cwd, ExecutionAuthorityLevel::Autonomous, Some(&tok))
        .await;
    assert_eq!(records.len(), 2, "Cancelled batch must return one record per call");
    assert!(records.iter().all(|r| !r.success));
}

#[test]
fn test_session_truncation_compaction_and_pricing_taxonomy() {
    // Truncate to budget must not leave leading orphaned Role::Tool
    let mut assistant_msg = SessionMessage::new(Role::Assistant, "x".repeat(400));
    assistant_msg.tool_calls = Some(vec![ToolCall {
        id: "t1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction { name: "view_file".to_string(), arguments: "{}".to_string() },
        thought_signature: None,
    }]);
    let msgs = vec![
        SessionMessage::new(Role::System, "sys"),
        assistant_msg,
        SessionMessage::tool_result("t1", "view_file", "ok".to_string()),
    ];
    let truncated = SessionCompactor::truncate_to_token_budget(&msgs, 40);
    assert!(truncated.iter().filter(|m| m.role != Role::System).all(|m| m.role != Role::Tool));

    // truncate_active_messages must also truncate transcript so compact_session cannot resurrect
    let mut store = SessionStore::new("s1", std::env::temp_dir(), None);
    store.add_message(SessionMessage::new(Role::System, "sys"));
    for i in 0..8 {
        store.add_message(SessionMessage::new(Role::User, format!("msg {i} {}", "z".repeat(120))));
    }
    store.truncate_active_messages(4);
    assert_eq!(store.messages().len(), store.transcript().len());
    let (_compacted, _report) = SessionCompactor::default().compact(store.messages(), &CompactionPolicy::default(), store.transcript());

    // ProviderTokenUsage::accumulate from Default preserves is_exact
    let mut acc = ProviderTokenUsage::default();
    acc.accumulate(&ProviderTokenUsage { prompt_tokens: 10, completion_tokens: 5, cached_prompt_tokens: 0, is_exact: true });
    assert!(acc.is_exact);

    // Non-OpenAI provider must not cross-route to gpt-6-luna
    let cat = ModelPricingCatalog::default();
    let fb = resolve_compatible_fallback_model_with_catalog("qwen", "qwen-max", Some("gpt-6-luna"), &cat);
    assert_ne!(fb.as_deref(), Some("gpt-6-luna"));
}

#[test]
fn test_cas_on_disk_gc_and_lock_unpoison() {
    // lock_unpoison must recover inner value after a holder panics
    let poisoned = Arc::new(Mutex::new(42usize));
    let p_clone = Arc::clone(&poisoned);
    let _ = std::thread::spawn(move || {
        let mut g = p_clone.lock().unwrap();
        *g = 99;
        panic!("intentional poison");
    })
    .join();
    assert_eq!(*crate::plumbing::hash_and_math::lock_unpoison(&poisoned), 99);

    // WorkspaceSnapshotCas must prune >40 manifests on disk and GC unreferenced blobs
    let dir = std::env::temp_dir().join(format!("heav3ns-cas-gc-{}", now_ms()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cas = WorkspaceSnapshotCas::open(&dir);
    for i in 0..45 {
        std::fs::write(dir.join("data.txt"), format!("version-{i}")).unwrap();
        cas.capture_snapshot(&format!("turn-{i}"), i as u64).unwrap();
    }
    let cas_root = cas.cas_root().to_path_buf();
    let manifest_count = std::fs::read_dir(cas_root.join("manifests")).unwrap().count();
    let blob_count = std::fs::read_dir(cas_root.join("blobs")).unwrap().count();
    assert!(manifest_count <= 40, "Expected <= 40 manifests on disk, got {manifest_count}");
    assert!(blob_count <= 40, "Expected <= 40 blobs on disk after GC, got {blob_count}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&cas_root);
}
