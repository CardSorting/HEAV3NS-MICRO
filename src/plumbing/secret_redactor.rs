// [LAYER: PLUMBING]
use crate::plumbing::shell_guard::ShellCommandGuard;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub type SecretRedactionResult = String;

pub trait ISecretRedactor {
    fn redact_secrets(&self, text: &str) -> String;
}

pub trait ISafetyPolicy {
    fn scan_threats(&self, content: &str, scope: &str) -> Vec<String>;
}

/// Dual-scope credential denylist tier distilled from Codewhale (`crates/sandbox/src/read_guard.rs`).
/// `Process` is blocked at both OS sandbox and model-context levels; `ModelContext` additionally blocks
/// CLI auth configs and `.env` secrets in model file/search tools while allowing legitimate CLI subprocesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialDenyScope {
    Process,
    ModelContext,
}

const ALLOWED_DOTENV_TEMPLATES: &[&str] = &[
    ".env.example",
    ".env.sample",
    ".env.template",
    ".env.defaults",
    ".env.dist",
    ".env.schema",
];

const PROTECTED_INSTRUCTION_FILES: &[&str] = &[
    "agents.md",
    "claude.md",
    "gemini.md",
    "soul.md",
    ".cursorrules",
    ".windsurfrules",
    ".clinerules",
];

/// Deterministic secret redactor and dual-scope credential path detector (`ADR-0008` / `ADR-0014`).
pub struct DeterministicSecretRedactor;

impl DeterministicSecretRedactor {
    /// Evaluates component-wise, case-folded credential path rules and returns a safe rule label
    /// (never echoing resolved symlink targets to the model context).
    pub fn classify_sensitive_path(path: &str, scope: CredentialDenyScope) -> Option<&'static str> {
        let norm = path.replace('\\', "/");
        let segments: Vec<String> = norm
            .split('/')
            .filter(|s| !s.is_empty() && *s != ".")
            .map(|s| s.to_ascii_lowercase())
            .collect();
        if segments.is_empty() {
            return None;
        }
        let file_name = segments.last().map(String::as_str).unwrap_or("");

        // Process-level high-value credential stores (blocked everywhere)
        for (i, seg) in segments.iter().enumerate() {
            let next = segments.get(i + 1).map(String::as_str);
            if seg == ".ssh" {
                return Some("SSH keys and config (~/.ssh)");
            }
            if seg == ".gnupg" {
                return Some("GPG keyring (~/.gnupg)");
            }
            if (seg == ".heav3ns" || seg == ".codewhale")
                && matches!(next, Some("credentials" | "secrets" | "config.json"))
            {
                return Some("Agent runtime credentials (~/.heav3ns)");
            }
            if seg == "library" && next == Some("keychains") {
                return Some("macOS Keychain store (~/Library/Keychains)");
            }
        }
        if matches!(file_name, ".anthropic_oauth.json" | "bws_cache.enc.json" | "id_rsa" | "id_ed25519" | "id_ecdsa") {
            return Some("Private authentication key material");
        }

        if scope == CredentialDenyScope::Process {
            return None;
        }

        // ModelContext-level secrets (blocked in model read/search/diff tools)
        if file_name == ".env"
            || (file_name.starts_with(".env.") && !ALLOWED_DOTENV_TEMPLATES.contains(&file_name))
        {
            return Some("Environment secret file (.env)");
        }
        if matches!(
            file_name,
            ".npmrc" | ".pypirc" | ".netrc" | ".git-credentials" | ".pgpass"
        ) {
            return Some("Package/registry credential file");
        }

        for (i, seg) in segments.iter().enumerate() {
            let next = segments.get(i + 1).map(String::as_str);
            if seg == ".aws" {
                return Some("AWS credentials (~/.aws)");
            }
            if seg == ".kube" {
                return Some("Kubernetes kubeconfig (~/.kube)");
            }
            if seg == ".azure" {
                return Some("Azure CLI credentials (~/.azure)");
            }
            if seg == ".config" && next == Some("gcloud") {
                return Some("Google Cloud credentials (~/.config/gcloud)");
            }
            if seg == ".docker" && next == Some("config.json") {
                return Some("Docker registry auth (~/.docker/config.json)");
            }
            if seg == ".cargo" && matches!(next, Some("credentials" | "credentials.toml")) {
                return Some("Cargo registry token (~/.cargo/credentials)");
            }
        }
        None
    }

    #[inline]
    pub fn is_sensitive_path(path: &str) -> bool {
        Self::classify_sensitive_path(path, CredentialDenyScope::ModelContext).is_some()
    }

    pub fn redact(text: &str) -> String {
        if text.len() < 10 {
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
                    Regex::new(r"\b(sk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,})\b").unwrap(),
                ),
                (
                    "github_token",
                    Regex::new(r"\b((?:gh[pousr]_[A-Za-z0-9_]{30,}|github_pat_[A-Za-z0-9_]{22,}))\b").unwrap(),
                ),
                ("gitlab_token", Regex::new(r"\b(glpat-[A-Za-z0-9_-]{20,})\b").unwrap()),
                ("aws_key", Regex::new(r"\b((?:AKIA|ASIA)[0-9A-Z]{16})\b").unwrap()),
                ("google_key", Regex::new(r"\b(AIza[0-9A-Za-z_-]{30,})\b").unwrap()),
                ("hf_token", Regex::new(r"\b(hf_[A-Za-z0-9]{24,})\b").unwrap()),
                ("npm_token", Regex::new(r"\b(npm_[A-Za-z0-9]{30,})\b").unwrap()),
                ("pypi_token", Regex::new(r"\b(pypi-[A-Za-z0-9_-]{32,})\b").unwrap()),
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
        static BEARER_RE: OnceLock<Regex> = OnceLock::new();
        static URL_AUTH_RE: OnceLock<Regex> = OnceLock::new();
        static KV_SECRET_RE: OnceLock<Regex> = OnceLock::new();

        let bearer_re = BEARER_RE.get_or_init(|| {
            Regex::new(r"(?i)(\bBearer\s+)[A-Za-z0-9._~+/=-]{16,}").unwrap()
        });
        let url_auth_re = URL_AUTH_RE.get_or_init(|| {
            Regex::new(r"(https?://[^/\s:@]+:)([^@\s/]{4,})(@)").unwrap()
        });
        let kv_re = KV_SECRET_RE.get_or_init(|| {
            Regex::new(r#"(?i)(["']?(?:api[_-]?key|access[_-]?token|auth[_-]?token|client[_-]?secret|password|secret[_-]?key)["']?\s*[:=]\s*["']?)([A-Za-z0-9._~+/=-]{12,})(["']?)"#).unwrap()
        });

        let mut out = text.to_string();
        for &(label, ref re) in rules {
            if re.is_match(&out) {
                out = re.replace_all(&out, format!("[REDACTED:{label}]")).into_owned();
            }
        }
        if bearer_re.is_match(&out) {
            out = bearer_re.replace_all(&out, "${1}[REDACTED:bearer_token]").into_owned();
        }
        if url_auth_re.is_match(&out) {
            out = url_auth_re.replace_all(&out, "${1}[REDACTED:url_auth]${3}").into_owned();
        }
        if kv_re.is_match(&out) {
            out = kv_re.replace_all(&out, "${1}[REDACTED:secret_value]${3}").into_owned();
        }
        out
    }
}

impl ISecretRedactor for DeterministicSecretRedactor {
    fn redact_secrets(&self, text: &str) -> String {
        Self::redact(text)
    }
}

/// Stateless command, filesystem path, prompt-injection threat, and PTY query guard.
pub struct SafetyPolicy;

impl SafetyPolicy {
    pub fn check_command(cmd: &str) -> Result<(), String> {
        Self::check_invisible_unicode(cmd)?;
        let lower = cmd.trim().to_lowercase();
        let forbidden = [
            "rm -rf /",
            "rm -rf /*",
            "rm -fr /",
            "rm -fr /*",
            "rm -rf ~",
            "rm -fr ~",
            "mkfs",
            "dd if=/dev/zero",
            ":(){:|:&};:",
            "chmod -r 777 /",
            "> /dev/sda",
        ];
        for pat in forbidden {
            if lower.contains(pat) {
                return Err(format!("Safety policy blocked destructive command pattern: '{pat}'"));
            }
        }
        Ok(())
    }

    /// Refuses invisible/Trojan Source Unicode in shell commands and surfaces an explicit octal `printf` escape hint.
    pub fn check_invisible_unicode(text: &str) -> Result<(), String> {
        for (idx, ch) in text.chars().enumerate() {
            let code = ch as u32;
            let is_mid_bom = code == 0xFEFF && idx > 0;
            if matches!(
                code,
                0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x2069
            ) || is_mid_bom
            {
                let mut buf = [0u8; 4];
                let octal: String = ch
                    .encode_utf8(&mut buf)
                    .as_bytes()
                    .iter()
                    .map(|b| format!("\\{:03o}", b))
                    .collect();
                return Err(format!(
                    "Safety policy refused hidden/Trojan Unicode U+{code:04X}. If intentional, pass explicit octal bytes via $(printf '{octal}')."
                ));
            }
        }
        Ok(())
    }

    /// Checks a filesystem path (and any canonicalized symlink target when `canonical_target` is provided)
    /// against the Dual-Scope Credential Denylist without leaking symlink destination paths.
    pub fn check_credential_path_with_target(
        path: &Path,
        canonical_target: Option<&Path>,
        scope: CredentialDenyScope,
    ) -> Result<(), String> {
        if let Some(label) = DeterministicSecretRedactor::classify_sensitive_path(&path.to_string_lossy(), scope) {
            return Err(format!("Safety policy blocked access to protected credential store: {label}"));
        }
        if let Some(canon) = canonical_target {
            if let Some(label) = DeterministicSecretRedactor::classify_sensitive_path(&canon.to_string_lossy(), scope) {
                return Err(format!("Safety policy blocked access to protected credential store: {label}"));
            }
        }
        Ok(())
    }

    pub fn check_path(path: &Path) -> Result<(), String> {
        Self::check_credential_path_with_target(path, None, CredentialDenyScope::ModelContext)
    }

    /// Detects writes to protected agent instruction files (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `SOUL.md`, `.cursorrules`).
    pub fn check_protected_instruction_write(path: &Path) -> Option<String> {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_ascii_lowercase();
        if PROTECTED_INSTRUCTION_FILES.contains(&file_name.as_str()) {
            return Some(format!(
                "[Instruction-Guard Advisory] Modifying governance file '{}'. Ensure changes preserve architectural sovereignty.",
                path.display()
            ));
        }
        None
    }

    /// Scans skills, context files, or memory payloads for prompt injection, C2/Brainworm promptware,
    /// secret exfiltration, and invisible Unicode characters (`threat_patterns`).
    pub fn scan_for_threats(content: &str, scope: &str) -> Vec<String> {
        let mut findings = Vec::new();
        for ch in content.chars() {
            let code = ch as u32;
            if matches!(code, 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x2069 | 0xFEFF) {
                findings.push(format!("invisible unicode character U+{code:04X}"));
                break;
            }
        }
        static THREAT_RULES: OnceLock<Vec<(&'static str, &'static str, Regex)>> = OnceLock::new();
        let rules = THREAT_RULES.get_or_init(|| {
            vec![
                (
                    "prompt_injection",
                    "all",
                    Regex::new(r"(?i)ignore\s+(?:all\s+)?(?:previous|prior|above)\s+instructions").unwrap(),
                ),
                (
                    "role_hijack",
                    "all",
                    Regex::new(r"(?i)you\s+are\s+now\s+(?:a|an)\s+(?:unrestricted|jailbroken|evil|root)").unwrap(),
                ),
                (
                    "deception_hide",
                    "all",
                    Regex::new(r"(?i)do\s+not\s+(?:tell|inform|mention\s+to)\s+the\s+user").unwrap(),
                ),
                (
                    "sys_prompt_override",
                    "all",
                    Regex::new(r"(?i)system\s+prompt\s+override").unwrap(),
                ),
                (
                    "exfil_curl",
                    "all",
                    Regex::new(r"(?i)curl\s+[^\n]*\$\{?\w*(?:KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL|API)\w*\}?").unwrap(),
                ),
                (
                    "exfil_wget",
                    "all",
                    Regex::new(r"(?i)wget\s+[^\n]*\$\{?\w*(?:KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL|API)\w*\}?").unwrap(),
                ),
                (
                    "ssh_backdoor",
                    "all",
                    Regex::new(r"(?i)>>\s*~?/?\.ssh/authorized_keys").unwrap(),
                ),
                (
                    "c2_dns_txt",
                    "all",
                    Regex::new(r"(?i)(?:dig|nslookup|host)\s+[^\n]*\bTXT\b").unwrap(),
                ),
                (
                    "c2_doh_lookup",
                    "all",
                    Regex::new(r"(?i)(?:dns\.google/resolve|cloudflare-dns\.com/dns-query)").unwrap(),
                ),
                (
                    "hidden_html_comment",
                    "context",
                    Regex::new(r"(?is)<!--[^>]*?(?:ignore|override|system|secret|hidden|do\s+not\s+mention)[^>]*?-->")
                        .unwrap(),
                ),
                (
                    "strict_base64_exec",
                    "strict",
                    Regex::new(r"(?i)base64\s+(?:-d|--decode)\s*\|\s*(?:sh|bash|zsh|python)").unwrap(),
                ),
            ]
        });

        for &(id, rule_scope, ref re) in rules {
            let active = match scope {
                "strict" => true,
                "context" => rule_scope == "all" || rule_scope == "context",
                _ => rule_scope == "all",
            };
            if active && re.is_match(content) {
                findings.push(id.to_string());
            }
        }
        findings
    }

    pub fn detect_foreground_background_guidance(cmd: &str) -> Option<String> {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return None;
        }
        static BG_PREFIX_RE: OnceLock<Regex> = OnceLock::new();
        let bg_re = BG_PREFIX_RE.get_or_init(|| Regex::new(r"(?:^|[;&|]\s*)(?:nohup|disown|setsid)\b").unwrap());
        if bg_re.is_match(trimmed) {
            return Some(
                "[Terminal-Guard Advisory] Shell-level backgrounding (`nohup`/`disown`/`setsid`) detected in foreground execution. Child processes may be reaped when the command exits."
                    .to_string(),
            );
        }
        if trimmed.ends_with('&') && !trimmed.ends_with("&&") && !trimmed.ends_with(">&") {
            return Some(
                "[Terminal-Guard Advisory] Trailing `&` background operator detected in synchronous command."
                    .to_string(),
            );
        }
        static SERVER_RE: OnceLock<Regex> = OnceLock::new();
        let srv_re = SERVER_RE.get_or_init(|| {
            Regex::new(r"(?i)\b(?:(?:npm|pnpm|yarn|bun)\s+(?:run\s+)?(?:dev|start|serve)|npx\s+(?:vite|next\s+dev)|python3?\s+-m\s+http\.server|uvicorn\s+\S+|flask\s+run|docker(?:\s+compose|-compose)\s+up\b)").unwrap()
        });
        if srv_re.is_match(trimmed) && !trimmed.contains(" -d") && !trimmed.contains(" --detach") {
            return Some(
                "[Terminal-Guard Advisory] Long-lived server command detected in synchronous execution; ensure bounded timeout or daemon mode."
                    .to_string(),
            );
        }
        None
    }

    pub fn strip_and_respond_pty_queries(data: &[u8], rows: u16, cols: u16) -> (Vec<u8>, Vec<u8>) {
        if !data.contains(&0x1b) {
            return (data.to_vec(), Vec::new());
        }
        let mut cleaned = Vec::with_capacity(data.len());
        let mut replies = Vec::new();
        let mut i = 0usize;
        while i < data.len() {
            if data[i] == 0x1b && i + 2 < data.len() && data[i + 1] == b'[' {
                let start = i;
                let mut j = i + 2;
                let is_private = if j < data.len() && data[j] == b'?' {
                    j += 1;
                    true
                } else {
                    false
                };
                let param_start = j;
                while j < data.len() && (data[j].is_ascii_digit() || data[j] == b';') {
                    j += 1;
                }
                let params = std::str::from_utf8(&data[param_start..j]).unwrap_or("");
                if is_private && j + 1 < data.len() && data[j] == b'$' && data[j + 1] == b'p' {
                    let mode = if params.is_empty() { "0" } else { params };
                    replies.extend_from_slice(format!("\x1b[?{mode};2$y").as_bytes());
                    i = j + 2;
                    continue;
                } else if !is_private && j < data.len() {
                    let final_byte = data[j];
                    if final_byte == b'n' && params == "5" {
                        replies.extend_from_slice(b"\x1b[0n");
                        i = j + 1;
                        continue;
                    } else if final_byte == b'n' && (params == "6" || params.is_empty()) {
                        replies.extend_from_slice(b"\x1b[1;1R");
                        i = j + 1;
                        continue;
                    } else if final_byte == b't' && params == "18" {
                        replies.extend_from_slice(format!("\x1b[8;{};{}t", rows.max(1), cols.max(1)).as_bytes());
                        i = j + 1;
                        continue;
                    }
                }
                cleaned.push(data[start]);
                i = start + 1;
            } else {
                cleaned.push(data[i]);
                i += 1;
            }
        }
        (cleaned, replies)
    }

    pub fn detect_heredoc_file_redirection(cmd: &str) -> Option<String> {
        ShellCommandGuard::detect_heredoc_file_redirection(cmd)
    }

    pub fn detect_terminal_file_read(cmd: &str) -> Option<String> {
        ShellCommandGuard::detect_terminal_file_read(cmd)
    }

    /// Returns `true` when an environment variable name holds a provider API key, secret, or auth token
    /// that must be scrubbed from child shell/project subprocesses.
    pub fn is_sensitive_env_var(name: &str) -> bool {
        let upper = name.trim().to_ascii_uppercase();
        if upper.is_empty() {
            return false;
        }
        const EXACT: &[&str] = &[
            "GITHUB_TOKEN",
            "GH_TOKEN",
            "NPM_TOKEN",
            "HF_TOKEN",
            "AWS_SESSION_TOKEN",
            "AWS_ACCESS_KEY_ID",
            "OPENAI_KEY",
            "ANTHROPIC_KEY",
            "CODEX_API_KEY",
        ];
        if EXACT.contains(&upper.as_str()) {
            return true;
        }
        const SUFFIXES: &[&str] = &[
            "_API_KEY",
            "_ACCESS_KEY",
            "_SECRET",
            "_SECRET_KEY",
            "_ACCESS_TOKEN",
            "_REFRESH_TOKEN",
            "_AUTH_TOKEN",
            "_PRIVATE_KEY",
            "_CLIENT_SECRET",
            "_PASSWORD",
            "_PASSWD",
        ];
        SUFFIXES.iter().any(|suf| upper.ends_with(suf))
    }

    /// Collects all sensitive credential environment variable names currently set in the process.
    pub fn sensitive_env_vars() -> Vec<String> {
        std::env::vars_os()
            .filter_map(|(k, _)| {
                let s = k.to_string_lossy().into_owned();
                if Self::is_sensitive_env_var(&s) {
                    Some(s)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl ISafetyPolicy for SafetyPolicy {
    fn scan_threats(&self, content: &str, scope: &str) -> Vec<String> {
        Self::scan_for_threats(content, scope)
    }
}
