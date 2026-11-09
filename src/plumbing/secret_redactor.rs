// [LAYER: PLUMBING]
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

const PROTECTED_INSTRUCTION_FILES: &[&str] = &[
    "agents.md",
    "claude.md",
    "soul.md",
    ".cursorrules",
    ".windsurfrules",
    ".clinerules",
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

/// Stateless command, filesystem path, prompt-injection threat, and PTY query guard.
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

    /// Detects writes to protected agent instruction files (`AGENTS.md`, `CLAUDE.md`, `SOUL.md`, `.cursorrules`).
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
    /// `scope`: `"all"`, `"context"`, or `"strict"`.
    pub fn scan_for_threats(content: &str, scope: &str) -> Vec<String> {
        let mut findings = Vec::new();
        for ch in content.chars() {
            let code = ch as u32;
            if matches!(code, 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0xFEFF) {
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

    /// Detects shell-level backgrounding (`nohup`, `disown`, `setsid`, trailing `&`) and long-lived
    /// dev servers (`npm run dev`, `vite`, `uvicorn`, `docker compose up`) in foreground execution.
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

    /// Strips blocking ANSI terminal queries (`ESC[5n`, `ESC[18t`, `ESC[6n`, `ESC[?<mode>$p`) from PTY output
    /// and returns `(cleaned_output_bytes, synthesized_reply_bytes)`.
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

    /// `ADR-0015`: Detects heredoc file-write patterns (`cat << 'EOF' > file`, `tee file << EOF`)
    /// while ignoring arithmetic shifts (`$((1 << 4))`), here-strings (`<<<`), and non-file interpreter heredocs.
    pub fn detect_heredoc_file_redirection(cmd: &str) -> Option<String> {
        let trimmed = cmd.trim();
        if !trimmed.contains("<<") {
            return None;
        }
        let first_line = trimmed.lines().next().unwrap_or(trimmed);
        if first_line.contains("<<<") || first_line.contains("$((") || first_line.contains("((") {
            return None;
        }
        let has_heredoc_op = {
            static HEREDOC_RE: OnceLock<Regex> = OnceLock::new();
            let re = HEREDOC_RE.get_or_init(|| Regex::new(r"<<-?\s*['\x22]?[A-Za-z_][A-Za-z0-9_]*['\x22]?").unwrap());
            re.is_match(first_line)
        };
        if !has_heredoc_op {
            return None;
        }
        static REDIR_RE: OnceLock<Regex> = OnceLock::new();
        let redir_re = REDIR_RE.get_or_init(|| {
            Regex::new(r"(?:>>?\s*([^\s;&|]+))|(?:\|\s*tee\s+(?:-a\s+)?([^\s;&|]+))|(?:^\s*(?:cat|tee)\s+>\s*([^\s;&|<]+))").unwrap()
        });
        if let Some(caps) = redir_re.captures(first_line) {
            let target = caps
                .get(1)
                .or_else(|| caps.get(2))
                .or_else(|| caps.get(3))
                .map(|m| m.as_str())
                .unwrap_or("file");
            if !target.starts_with("/dev/null") && !target.starts_with('&') {
                return Some(format!(
                    "[ADR-0015 Advisory] Heredoc file-write to '{target}' detected. Prefer `write_file` or `replace_file_content` for atomic VFS journaling and rollback."
                ));
            }
        }
        None
    }

    /// `ADR-0016`: Detects bare terminal file-read dumps (`cat src/main.rs`, `head -n 50 file`)
    /// while allowing filter pipelines (`| grep`, `| rg`, `| jq`, `| wc`), `tail -f`, and `/proc/` or `/sys/`.
    pub fn detect_terminal_file_read(cmd: &str) -> Option<String> {
        let trimmed = cmd.trim();
        if trimmed.is_empty() || trimmed.contains("<<") || trimmed.contains('>') {
            return None;
        }
        if trimmed.contains('|') {
            let post_pipe = trimmed.split('|').skip(1).collect::<Vec<_>>().join("|");
            let allowed_filters = ["grep", "rg", "awk", "sed", "jq", "wc", "sort", "uniq", "cut", "tr", "shasum", "sha256sum", "md5"];
            if allowed_filters.iter().any(|f| post_pipe.contains(f)) {
                return None;
            }
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() < 2 {
            return None;
        }
        let prog = tokens[0];
        if !matches!(prog, "cat" | "head" | "tail" | "less" | "more" | "bat" | "nl") {
            return None;
        }
        if prog == "tail" && tokens.iter().any(|t| *t == "-f" || *t == "-F" || *t == "--follow") {
            return None;
        }
        if let Some(&target) = tokens.last() {
            if !target.starts_with('-') && !target.starts_with("/proc/") && !target.starts_with("/sys/") && !target.starts_with("/dev/") {
                return Some(format!(
                    "[ADR-0016 Advisory] Terminal file dump `{prog} {target}` detected. Prefer `view_file` or `grep_search` for line-numbered, token-governed inspection."
                ));
            }
        }
        None
    }
}
