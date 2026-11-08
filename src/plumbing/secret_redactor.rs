// [LAYER: PLUMBING]
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub type SecretRedactionResult = String;

const DENIED_PATH_SUBSTRINGS: &[&str] = &[
    ".ssh/id_rsa",
    ".ssh/id_ed25519",
    ".ssh/id_ecdsa",
    ".ssh/authorized_keys",
    ".aws/credentials",
    ".docker/config.json",
    ".kube/config",
    ".gnupg",
    ".netrc",
    ".git-credentials",
    ".anthropic_oauth.json",
    "bws_cache.enc.json",
];

/// Deterministic secret redactor and sensitive credential path detector (`ADR-047`).
pub struct DeterministicSecretRedactor;

impl DeterministicSecretRedactor {
    #[inline]
    pub fn is_sensitive_path(path: &str) -> bool {
        let norm = path.replace('\\', "/");
        DENIED_PATH_SUBSTRINGS.iter().any(|p| norm.contains(p))
    }

    pub fn redact(text: &str) -> String {
        if text.len() < 12 {
            return text.to_string();
        }
        static PATTERNS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
        let rules = PATTERNS.get_or_init(|| {
            vec![
                (
                    "pem_key",
                    Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----").unwrap(),
                ),
                ("anthropic_key", Regex::new(r"\b(sk-ant-[A-Za-z0-9_-]{20,})\b").unwrap()),
                (
                    "openai_key",
                    Regex::new(r"\b(sk-(?:proj-)?[A-Za-z0-9_-]{20,})\b").unwrap(),
                ),
                ("github_token", Regex::new(r"\b(gh[pousr]_[A-Za-z0-9_]{36,})\b").unwrap()),
                ("aws_key", Regex::new(r"\b(AKIA[0-9A-Z]{16})\b").unwrap()),
                ("google_key", Regex::new(r"\b(AIza[0-9A-Za-z_-]{30,})\b").unwrap()),
                ("hf_token", Regex::new(r"\b(hf_[A-Za-z0-9]{24,})\b").unwrap()),
                (
                    "slack_token",
                    Regex::new(r"\b(xox[baprs]-[0-9A-Za-z-]{10,64})\b").unwrap(),
                ),
                (
                    "jwt",
                    Regex::new(r"\b(eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,})\b").unwrap(),
                ),
            ]
        });
        let mut out = text.to_string();
        for &(label, ref re) in rules {
            if re.is_match(&out) {
                out = re.replace_all(&out, format!("[REDACTED:{label}]")).into_owned();
            }
        }
        out
    }
}

/// Stateless command and filesystem path safety guard.
pub struct SafetyPolicy;

impl SafetyPolicy {
    pub fn check_command(cmd: &str) -> Result<(), String> {
        let lower = cmd.trim().to_lowercase();
        let forbidden = [
            "rm -rf /",
            "rm -rf /*",
            "mkfs",
            "dd if=/dev/zero",
            ":(){:|:&};:",
            "chmod -r 777 /",
        ];
        for pat in forbidden {
            if lower.contains(pat) {
                return Err(format!("Safety policy blocked destructive command pattern: '{pat}'"));
            }
        }
        Ok(())
    }

    pub fn check_path(path: &Path) -> Result<(), String> {
        let s = path.to_string_lossy();
        if DeterministicSecretRedactor::is_sensitive_path(&s) {
            return Err(format!("Safety policy blocked access to sensitive credential path: '{s}'"));
        }
        Ok(())
    }
}
