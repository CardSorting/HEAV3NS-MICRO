// [LAYER: CORE]
use crate::infrastructure::shell_runner::execute_shell_command_sandboxed;
use crate::plumbing::seatbelt_profile::SeatbeltSandboxMode;
use crate::plumbing::secret_redactor::SafetyPolicy;

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
