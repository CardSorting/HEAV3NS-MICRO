// [LAYER: PLUMBING]
use crate::plumbing::secret_redactor::{CredentialDenyScope, DeterministicSecretRedactor, SafetyPolicy};
use crate::plumbing::shell_guard::split_top_level_statements;
use std::path::Path;

/// OS Seatbelt sandbox posture distilled from Codewhale (`crates/sandbox/src/seatbelt.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatbeltSandboxMode {
    ReadOnly,
    WorkspaceWrite { allow_network: bool },
    Unrestricted,
}

/// Parameterized macOS `/usr/bin/sandbox-exec` invocation (`-p <sbpl>` + `-DKEY=VALUE` parameters)
/// preventing SBPL string injection while enforcing read-only `.git/hooks` carve-outs and last-match-wins credential blocks.
#[derive(Debug, Clone)]
pub struct SeatbeltInvocation {
    pub profile_sbpl: String,
    pub parameters: Vec<(String, String)>,
}

/// Risk and side-effect classification for shell commands under the 5-stop `ExecutionPosture` dial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSecurityVerdict {
    ReadOnlySafe,
    WorkspaceMutating { summary: String },
    RequireApproval { reason: String },
    Deny { reason: String },
}

const READONLY_CARVEOUT_SUBDIRS: &[&str] = &[
    ".git/hooks",
    ".git/config",
    ".githooks",
    ".heav3ns",
    ".codewhale",
    ".agents",
    ".claude",
];

const PURE_READONLY_COMMANDS: &[&str] = &[
    "ls", "pwd", "whoami", "uname", "date", "which", "type", "wc", "stat", "file", "du", "df",
    "true", "false", "echo", "printf", "basename", "dirname", "realpath", "id",
];

/// Builds a parameterized macOS Seatbelt SBPL profile for `/usr/bin/sandbox-exec`.
/// Uses SBPL last-match-wins ordering so credential blocks at the bottom cannot be overridden by earlier allow rules.
pub fn build_macos_seatbelt_invocation(
    mode: SeatbeltSandboxMode,
    workspace_root: &Path,
    home_dir: Option<&Path>,
) -> Option<SeatbeltInvocation> {
    if mode == SeatbeltSandboxMode::Unrestricted {
        return None;
    }

    let mut params = Vec::new();
    let canon_ws = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let ws_str = canon_ws.to_string_lossy().to_string();
    params.push(("WORKSPACE_ROOT".to_string(), ws_str.clone()));

    let mut ro_carveouts = Vec::new();
    for (idx, sub) in READONLY_CARVEOUT_SUBDIRS.iter().enumerate() {
        let key = format!("RO_CARVEOUT_{idx}");
        let joined = canon_ws.join(sub);
        let full = joined.canonicalize().unwrap_or(joined).to_string_lossy().to_string();
        params.push((key.clone(), full));
        ro_carveouts.push(key);
    }

    let mut sbpl = String::from(
        "(version 1)\n\
         (allow default)\n\
         (deny file-write*)\n\
         (allow file-write* (subpath \"/dev\"))\n",
    );
    if mode != SeatbeltSandboxMode::ReadOnly {
        sbpl.push_str(
            "(allow file-write* (subpath \"/private/tmp\"))\n\
             (allow file-write* (subpath \"/tmp\"))\n\
             (allow file-write* (subpath \"/private/var/folders\"))\n\
             (allow file-write* (subpath \"/var/folders\"))\n",
        );
    }

    if let SeatbeltSandboxMode::WorkspaceWrite { allow_network } = mode {
        sbpl.push_str("(allow file-write* (require-all (subpath (param \"WORKSPACE_ROOT\"))");
        for key in &ro_carveouts {
            sbpl.push_str(&format!(" (require-not (subpath (param \"{key}\")))"));
        }
        sbpl.push_str("))\n");
        if !allow_network {
            sbpl.push_str("(deny network-outbound)\n(allow network-outbound (local ip \"localhost:*\"))\n");
        }
    } else {
        sbpl.push_str("(deny network-outbound)\n");
    }

    // Last-match-wins Process-scope credential & SSH-agent socket blocks
    if let Some(home) = home_dir {
        let canon_home = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
        let denied_rel = [
            ".ssh",
            ".gnupg",
            ".aws",
            ".heav3ns/credentials",
            ".codewhale/credentials",
            ".codewhale/secrets",
            "Library/Keychains",
            ".npmrc",
            ".netrc",
            ".git-credentials",
            ".kube",
            ".azure",
            ".config/gcloud",
            ".docker/config.json",
            ".cargo/credentials.toml",
            ".zsh_history",
            ".bash_history",
        ];
        for (idx, rel) in denied_rel.iter().enumerate() {
            let key = format!("DENY_CRED_{idx}");
            let target = canon_home.join(rel);
            let target_str = target.canonicalize().unwrap_or(target).to_string_lossy().to_string();
            params.push((key.clone(), target_str));
            sbpl.push_str(&format!(
                "(deny file-read* (subpath (param \"{key}\")))\n\
                 (deny file-write* (subpath (param \"{key}\")))\n\
                 (deny network-outbound (remote unix-socket (subpath (param \"{key}\"))))\n"
            ));
        }
    }

    Some(SeatbeltInvocation {
        profile_sbpl: sbpl,
        parameters: params,
    })
}

/// Inspects a shell command for destructive patterns, Trojan Unicode, `curl`/`wget` credential upload exfiltration,
/// dynamic expansion hazards, and read-only vs mutating execution stakes (`crates/execpolicy` distillation).
pub fn inspect_command_security(cmd: &str) -> CommandSecurityVerdict {
    if cmd.len() > 65_536 {
        return CommandSecurityVerdict::Deny {
            reason: "Command exceeds 64 KiB inspection ceiling (fail-closed)".to_string(),
        };
    }
    if let Err(reason) = SafetyPolicy::check_command(cmd) {
        return CommandSecurityVerdict::Deny { reason };
    }
    let threats = SafetyPolicy::scan_for_threats(cmd, "strict");
    if !threats.is_empty() {
        return CommandSecurityVerdict::Deny {
            reason: format!("Threat scanner blocked command ({})", threats.join(", ")),
        };
    }

    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return CommandSecurityVerdict::ReadOnlySafe;
    }

    let statements = split_top_level_statements(trimmed);
    let mut all_readonly = !statements.is_empty();
    let mut first_mutating = String::new();

    for stmt in &statements {
        for stage in split_unquoted_pipes(stmt) {
            let tokens = tokenize_shell_words(&stage);
            if tokens.is_empty() {
                continue;
            }
            let (head, args) = strip_command_wrappers(&tokens);
            let Some(bin) = head else {
                continue;
            };

            // Recursive inspection for `sh -c "..."` / `bash -c "..."`
            if matches!(bin.as_str(), "sh" | "bash" | "zsh" | "dash" | "ksh") {
                let mut saw_c_flag = false;
                for pair in args.windows(2) {
                    if pair[0] == "-c" || (pair[0].starts_with('-') && pair[0].contains('c')) {
                        saw_c_flag = true;
                        match inspect_command_security(&pair[1]) {
                            d @ (CommandSecurityVerdict::Deny { .. } | CommandSecurityVerdict::RequireApproval { .. }) => {
                                return d;
                            }
                            CommandSecurityVerdict::WorkspaceMutating { summary } => {
                                all_readonly = false;
                                if first_mutating.is_empty() {
                                    first_mutating = summary;
                                }
                            }
                            CommandSecurityVerdict::ReadOnlySafe => {}
                        }
                    }
                }
                if !saw_c_flag {
                    all_readonly = false;
                    if first_mutating.is_empty() {
                        first_mutating = format!("{bin} subshell");
                    }
                }
                continue;
            }

            // Codewhale curl / wget credential & file upload guard
            if matches!(bin.as_str(), "curl" | "wget" | "scp" | "rsync" | "nc" | "ncat") {
                if let Some(verdict) = check_network_upload_exfiltration(&bin, args, stmt) {
                    return verdict;
                }
                all_readonly = false;
                if first_mutating.is_empty() {
                    first_mutating = format!("network command `{bin}`");
                }
                continue;
            }

            // Destructive rm variants (`rm -fr /`, `rm -rf $HOME`)
            if bin == "rm" {
                for a in args {
                    let clean = a.trim_matches(|c| c == '"' || c == '\'');
                    if matches!(clean, "/" | "/*" | "~" | "~/" | "$HOME" | "${HOME}") {
                        return CommandSecurityVerdict::Deny {
                            reason: format!("Safety policy refused destructive `rm` target '{clean}'"),
                        };
                    }
                }
            }

            // Git push / reset --hard / clean -fd require explicit approval outside FullAccess
            if bin == "git" {
                let sub = first_git_subcommand(args);
                if matches!(sub.as_deref(), Some("push" | "reset" | "clean")) {
                    let is_hard_or_push = sub.as_deref() == Some("push")
                        || args.iter().any(|a| a == "--hard" || a.starts_with("-f") || a == "--force");
                    if is_hard_or_push {
                        return CommandSecurityVerdict::RequireApproval {
                            reason: format!("High-impact git operation (`git {}`)", sub.unwrap_or_default()),
                        };
                    }
                }
            }

            if !is_stage_pure_readonly(&bin, args, &stage) {
                all_readonly = false;
                if first_mutating.is_empty() {
                    first_mutating = bin.clone();
                }
            }
        }
    }

    if all_readonly {
        CommandSecurityVerdict::ReadOnlySafe
    } else {
        CommandSecurityVerdict::WorkspaceMutating {
            summary: if first_mutating.is_empty() {
                "shell mutation".to_string()
            } else {
                first_mutating
            },
        }
    }
}

fn check_network_upload_exfiltration(bin: &str, args: &[String], full_stmt: &str) -> Option<CommandSecurityVerdict> {
    if matches!(bin, "scp" | "nc" | "ncat") {
        return Some(CommandSecurityVerdict::RequireApproval {
            reason: format!("Outbound transfer tool `{bin}` requires explicit human approval"),
        });
    }

    let has_pipe_in = full_stmt.contains('|') && !full_stmt.trim_start().starts_with(bin);
    let mut upload_flag: Option<String> = None;
    let mut upload_value = String::new();

    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        let is_curl_upload = bin == "curl"
            && (matches!(
                arg.as_str(),
                "-d" | "-F" | "-T" | "--data" | "--data-raw" | "--data-binary" | "--data-urlencode"
                    | "--form" | "--form-string" | "--json" | "--upload-file"
            ) || arg.starts_with("-d")
                || arg.starts_with("-F")
                || arg.starts_with("-T")
                || arg.starts_with("--data")
                || arg.starts_with("--form")
                || arg.starts_with("--json=")
                || arg.starts_with("--upload-file="));

        let is_wget_upload = bin == "wget"
            && (matches!(arg.as_str(), "--post-data" | "--post-file" | "--body-data" | "--body-file")
                || arg.starts_with("--post-data=")
                || arg.starts_with("--post-file=")
                || arg.starts_with("--body-data=")
                || arg.starts_with("--body-file="));

        let is_header_file = (arg == "-H" || arg == "--header")
            && args.get(i + 1).map(|v| v.contains('@') || v.contains('$')).unwrap_or(false);

        if is_curl_upload || is_wget_upload || is_header_file {
            upload_flag = Some(arg.clone());
            if let Some((_, eq_val)) = arg.split_once('=') {
                upload_value.push_str(eq_val);
            } else if let Some(next_val) = args.get(i + 1) {
                upload_value.push_str(next_val);
                i += 1;
            }
        }
        i += 1;
    }

    if let Some(flag) = upload_flag {
        let stripped_path = upload_value
            .trim_start_matches('@')
            .trim_start_matches('<')
            .split(';')
            .next()
            .unwrap_or("");
        let clean_path = if let Some((_, rhs)) = stripped_path.split_once('=') {
            rhs.trim_start_matches('@')
        } else {
            stripped_path
        };
        if let Some(label) =
            DeterministicSecretRedactor::classify_sensitive_path(clean_path, CredentialDenyScope::ModelContext)
        {
            return Some(CommandSecurityVerdict::Deny {
                reason: format!("Blocked `{bin}` upload targeting protected credential store: {label}"),
            });
        }
        return Some(CommandSecurityVerdict::RequireApproval {
            reason: format!("`{bin}` data/file upload (`{flag}`) requires explicit user approval outside Full Access"),
        });
    }

    if has_pipe_in && (full_stmt.contains(".env") || full_stmt.contains(".ssh") || full_stmt.contains("printenv") || full_stmt.contains("env ")) {
        return Some(CommandSecurityVerdict::Deny {
            reason: format!("Blocked piping environment/credential data into `{bin}`"),
        });
    }
    None
}

fn is_stage_pure_readonly(bin: &str, args: &[String], raw_stage: &str) -> bool {
    if raw_stage.contains('>')
        || raw_stage.contains("$(")
        || raw_stage.contains('`')
        || raw_stage.contains("<(")
        || raw_stage.contains(">(")
    {
        return false;
    }
    if PURE_READONLY_COMMANDS.contains(&bin) {
        return true;
    }
    if matches!(bin, "grep" | "rg" | "ripgrep" | "head" | "tail" | "cat" | "sort" | "uniq" | "cut" | "tr") {
        return true;
    }
    if bin == "find" {
        return !args.iter().any(|a| {
            matches!(
                a.as_str(),
                "-delete" | "-exec" | "-execdir" | "-ok" | "-okdir" | "-fprint" | "-fprintf" | "-fls" | "-fprint0"
            )
        });
    }
    if bin == "git" {
        return matches!(
            first_git_subcommand(args).as_deref(),
            Some("status" | "diff" | "log" | "show" | "branch" | "rev-parse" | "ls-files" | "blame")
        );
    }
    false
}

fn first_git_subcommand(args: &[String]) -> Option<String> {
    let mut i = 0usize;
    while i < args.len() {
        let a = &args[i];
        if matches!(a.as_str(), "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace") {
            i += 2;
        } else if a.starts_with('-') {
            i += 1;
        } else {
            return Some(a.to_ascii_lowercase());
        }
    }
    None
}

fn strip_command_wrappers(tokens: &[String]) -> (Option<String>, &[String]) {
    let mut i = 0usize;
    while i < tokens.len() && tokens[i].contains('=') && !tokens[i].starts_with('-') {
        i += 1;
    }
    while i < tokens.len() {
        let base = tokens[i].rsplit('/').next().unwrap_or(&tokens[i]).to_ascii_lowercase();
        if matches!(base.as_str(), "sudo" | "doas" | "nohup" | "time" | "exec" | "builtin" | "nice") {
            i += 1;
            while i < tokens.len() && tokens[i].starts_with('-') && base == "sudo" {
                if matches!(tokens[i].as_str(), "-u" | "-g" | "-C" | "-p") {
                    i += 2;
                } else {
                    i += 1;
                }
            }
        } else if base == "command" {
            i += 1;
            while i < tokens.len() && tokens[i].starts_with('-') {
                i += 1;
            }
        } else if base == "env" {
            i += 1;
            while i < tokens.len() {
                if matches!(tokens[i].as_str(), "-u" | "--unset" | "-C") {
                    i += 2;
                } else if tokens[i].starts_with('-') || tokens[i].contains('=') {
                    i += 1;
                } else {
                    break;
                }
            }
        } else {
            return (Some(base), &tokens[i + 1..]);
        }
    }
    (None, &[])
}

fn split_unquoted_pipes(stmt: &str) -> Vec<String> {
    let bytes = stmt.as_bytes();
    let (mut out, mut quote, mut start, mut i) = (Vec::new(), 0u8, 0usize, 0usize);
    while i < bytes.len() {
        let b = bytes[i];
        if quote == b'\'' {
            if b == b'\'' {
                quote = 0;
            }
        } else if quote == b'"' {
            if b == b'\\' {
                i += 1;
            } else if b == b'"' {
                quote = 0;
            }
        } else if b == b'\'' || b == b'"' {
            quote = b;
        } else if b == b'\\' {
            i += 1;
        } else if b == b'|' && bytes.get(i + 1) != Some(&b'|') && (i == 0 || bytes[i - 1] != b'|') {
            let seg = stmt.get(start..i).unwrap_or("").trim();
            if !seg.is_empty() {
                out.push(seg.to_string());
            }
            start = i + 1;
        }
        i += 1;
    }
    let tail = stmt.get(start..).unwrap_or("").trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

fn tokenize_shell_words(stage: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut chars = stage.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(ch) = chars.next() {
        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                } else {
                    cur.push(ch);
                }
            }
            Some('"') => {
                if ch == '\\' {
                    if let Some(nxt) = chars.next() {
                        cur.push(nxt);
                    }
                } else if ch == '"' {
                    quote = None;
                } else {
                    cur.push(ch);
                }
            }
            _ => {
                if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                } else if ch == '\\' {
                    if let Some(nxt) = chars.next() {
                        cur.push(nxt);
                    }
                } else if ch.is_whitespace() {
                    if !cur.is_empty() {
                        words.push(std::mem::take(&mut cur));
                    }
                } else {
                    cur.push(ch);
                }
            }
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}
