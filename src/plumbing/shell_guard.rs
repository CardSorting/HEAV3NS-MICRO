// [LAYER: PLUMBING]
use regex::Regex;
use std::sync::OnceLock;

/// Stateless shell command statement, heredoc (`ADR-0015`), and terminal file-read (`ADR-0016`) guard.
pub struct ShellCommandGuard;

#[derive(Debug, Clone)]
struct HeredocSpan {
    delimiter: String,
    index: usize,
    statement_start: usize,
}

#[derive(Debug, Default)]
struct ParsedStage {
    tokens: Vec<String>,
    command: Option<String>,
    target_files: Vec<String>,
    has_redirect_out: bool,
    has_heredoc: bool,
    has_here_string: bool,
    is_follow: bool,
    xargs_reader: Option<String>,
    find_exec_reader: Option<String>,
    subshell_advisory: Option<String>,
}

const FILE_READERS: &[&str] = &[
    "cat", "head", "tail", "more", "less", "tac", "bat", "nl", "fold", "pr", "view", "pg", "most",
];

impl ShellCommandGuard {
    pub fn detect_heredoc_file_redirection(cmd: &str) -> Option<String> {
        if !cmd.contains("<<") {
            return None;
        }
        let spans = find_top_level_heredocs(cmd);
        if spans.is_empty() {
            return None;
        }
        static BEFORE_RE: OnceLock<Regex> = OnceLock::new();
        static TEE_BEFORE_RE: OnceLock<Regex> = OnceLock::new();
        static AFTER_RE: OnceLock<Regex> = OnceLock::new();
        static PIPED_AFTER_RE: OnceLock<Regex> = OnceLock::new();

        let before_re = BEFORE_RE.get_or_init(|| {
            Regex::new(r#"(?:[0-9]*>>|[0-9]*>)\s*(?:"([^"\r\n]+)"|'([^'\r\n]+)'|([^\s'"><;&|\r\n]+))"#).unwrap()
        });
        let tee_before_re = TEE_BEFORE_RE.get_or_init(|| {
            Regex::new(r#"(?:^|[;&|\s])(?:sudo\s+|doas\s+)?tee\s+(?:-[A-Za-z]+\s+)*(?:"([^"\r\n]+)"|'([^'\r\n]+)'|([^\s'"><;&|\r\n]+))"#).unwrap()
        });
        let after_re = AFTER_RE.get_or_init(|| {
            Regex::new(r#"<<-?\s*\\?(?:"[^"\r\n]+"|'[^'\r\n]+'|[A-Za-z0-9_-]+)\s*(?:[0-9]*>>|[0-9]*>)\s*(?:"([^"\r\n]+)"|'([^'\r\n]+)'|([^\s'"><;&|\r\n]+))"#).unwrap()
        });
        let piped_after_re = PIPED_AFTER_RE.get_or_init(|| {
            Regex::new(r#"\|\s*(?:sudo\s+|doas\s+)?(?:tee\s+(?:-[A-Za-z]+\s+)*|(?:cat|sh|bash)\s*(?:[0-9]*>>|[0-9]*>)\s*)(?:"([^"\r\n]+)"|'([^'\r\n]+)'|([^\s'"><;&|\r\n]+))"#).unwrap()
        });

        let mut targets = Vec::new();
        for span in spans {
            let before = &cmd[span.statement_start..span.index];
            let line_end = cmd[span.index..].find('\n').map(|o| span.index + o).unwrap_or(cmd.len());
            let heredoc_line = &cmd[span.index..line_end];

            collect_regex_targets(before_re, before, &mut targets);
            collect_regex_targets(tee_before_re, before, &mut targets);
            collect_regex_targets(after_re, heredoc_line, &mut targets);
            collect_regex_targets(piped_after_re, heredoc_line, &mut targets);

            if line_end < cmd.len() {
                let rest = &cmd[line_end..];
                let pat = format!(
                    r#"(?m)^[ \t]*{}[ \t]*(?:[0-9]*>>|[0-9]*>)[ \t]*(?:"([^"\r\n]+)"|'([^'\r\n]+)'|([^\s'"><;&|\r\n]+))"#,
                    regex::escape(&span.delimiter)
                );
                if let Ok(closing_re) = Regex::new(&pat) {
                    collect_regex_targets(&closing_re, rest, &mut targets);
                }
            }
        }

        let first = targets.into_iter().find(|t| !is_ignored_write_target(t))?;
        Some(format!(
            "[ADR-0015 Advisory] Heredoc file-write to '{first}' detected. Prefer `write_file` or `replace_file_content` for atomic VFS journaling and rollback."
        ))
    }

    pub fn detect_terminal_file_read(cmd: &str) -> Option<String> {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return None;
        }
        for stmt in split_top_level_statements(trimmed) {
            let raw_stages = split_pipeline(&stmt);
            if raw_stages.is_empty() {
                continue;
            }
            let stages: Vec<ParsedStage> = raw_stages.iter().map(|s| parse_stage(s)).collect();
            for st in &stages {
                if st.command.as_deref() == Some("cat")
                    && st.has_redirect_out
                    && !st.has_heredoc
                    && !st.has_here_string
                    && stages.len() == 1
                    && st.tokens.iter().skip(1).all(|t| t.starts_with('-'))
                {
                    return Some(
                        "[ADR-0016 Advisory] Interactive `cat > file` without stdin will deadlock non-interactive execution. Prefer `write_file`."
                            .to_string(),
                    );
                }
            }
            let last = stages.last()?;
            if last.has_redirect_out {
                continue;
            }
            if let Some(ref xcmd) = last.xargs_reader {
                return Some(format!(
                    "[ADR-0016 Advisory] Terminal file dump `xargs {xcmd}` detected. Prefer `view_file` or `grep_search` for token-governed inspection."
                ));
            }
            if let Some(ref fcmd) = last.find_exec_reader {
                return Some(format!(
                    "[ADR-0016 Advisory] Terminal file dump `find -exec {fcmd}` detected. Prefer `view_file` or `find_files`."
                ));
            }
            if let Some(ref adv) = last.subshell_advisory {
                return Some(adv.clone());
            }
            if let Some(ref lcmd) = last.command {
                if FILE_READERS.contains(&lcmd.as_str()) && !last.is_follow {
                    if let Some(target) = last.target_files.first() {
                        return Some(format!(
                            "[ADR-0016 Advisory] Terminal file dump `{lcmd} {target}` detected. Prefer `view_file` or `grep_search` for line-numbered, token-governed inspection."
                        ));
                    }
                    if stages.len() > 1 {
                        for prev in stages[..stages.len() - 1].iter().rev() {
                            if let Some(target) = prev.target_files.first() {
                                let pcmd = prev.command.as_deref().unwrap_or(lcmd.as_str());
                                return Some(format!(
                                    "[ADR-0016 Advisory] Terminal file dump `{pcmd} {target}` piped to `{lcmd}` detected. Prefer `view_file` with `start_line`/`end_line`."
                                ));
                            }
                            let is_dumper = prev
                                .command
                                .as_deref()
                                .map(|c| FILE_READERS.contains(&c) || c == "tee")
                                .unwrap_or(false);
                            if !is_dumper {
                                break;
                            }
                        }
                    }
                }
            } else if let Some(target) = last.target_files.first() {
                return Some(format!(
                    "[ADR-0016 Advisory] Bare input redirection `< {target}` detected. Prefer `view_file`."
                ));
            }
        }
        None
    }
}

fn collect_regex_targets(re: &Regex, text: &str, out: &mut Vec<String>) {
    for caps in re.captures_iter(text) {
        if let Some(m) = caps.get(1).or_else(|| caps.get(2)).or_else(|| caps.get(3)) {
            let s = m.as_str().trim();
            if !s.is_empty() && !out.iter().any(|e| e == s) {
                out.push(s.to_string());
            }
        }
    }
}

fn is_ignored_write_target(target: &str) -> bool {
    target.starts_with("/dev/null") || target.starts_with('&') || target == "-"
}

fn is_whitelisted_read_target(target: &str) -> bool {
    let t = target.trim();
    t.is_empty()
        || t == "-"
        || t.starts_with("/dev/")
        || t.starts_with("/proc/")
        || t.starts_with("/sys/")
        || (t.starts_with("/etc/") && (t.ends_with("-release") || t == "/etc/issue" || t == "/etc/version"))
}

fn next_char_len(s: &str, idx: usize) -> usize {
    s.get(idx..).and_then(|r| r.chars().next()).map(|c| c.len_utf8()).unwrap_or(1)
}

fn find_top_level_heredocs(cmd: &str) -> Vec<HeredocSpan> {
    let bytes = cmd.as_bytes();
    let mut out = Vec::new();
    let (mut state, mut paren_depth, mut stmt_start, mut i) = (0u8, 0i32, 0usize, 0usize); // 0=none, 1=sq, 2=dq, 3=arith
    while i < bytes.len() {
        let c = bytes[i];
        let next = bytes.get(i + 1).copied().unwrap_or(0);
        match state {
            1 => {
                if c == b'\'' {
                    state = 0;
                }
            }
            2 => {
                if c == b'\\' && i + 1 < bytes.len() {
                    i += next_char_len(cmd, i + 1);
                } else if c == b'"' {
                    state = 0;
                }
            }
            3 => {
                if c == b'(' {
                    paren_depth += 1;
                } else if c == b')' {
                    paren_depth -= 1;
                    if paren_depth <= 0 {
                        state = 0;
                    }
                }
            }
            _ => {
                if c == b'\'' {
                    state = 1;
                } else if c == b'"' {
                    state = 2;
                } else if c == b'\\' && i + 1 < bytes.len() {
                    i += next_char_len(cmd, i + 1);
                } else if c == b'$' && next == b'(' && bytes.get(i + 2) == Some(&b'(') {
                    state = 3;
                    paren_depth = 2;
                    i += 2;
                } else if c == b'(' && next == b'(' {
                    state = 3;
                    paren_depth = 2;
                    i += 1;
                } else if c == b';' || c == b'\n' || c == b'\r' || (c == b'&' && next == b'&') || (c == b'|' && next == b'|') {
                    if next == b'&' || next == b'|' {
                        i += 1;
                    }
                    stmt_start = i + 1;
                } else if c == b'<' && next == b'<' && bytes.get(i + 2) != Some(&b'<') && (i == 0 || bytes[i - 1] != b'<') {
                    let mut idx = i + 2;
                    if bytes.get(idx) == Some(&b'-') {
                        idx += 1;
                    }
                    while idx < bytes.len() && (bytes[idx] == b' ' || bytes[idx] == b'\t') {
                        idx += 1;
                    }
                    if bytes.get(idx) == Some(&b'\\') {
                        idx += 1;
                    }
                    let delim = match bytes.get(idx).copied() {
                        Some(q @ (b'\'' | b'"')) => {
                            let start = idx + 1;
                            let mut end = start;
                            while end < bytes.len() && bytes[end] != q {
                                end += 1;
                            }
                            &cmd[start..end]
                        }
                        _ => {
                            let start = idx;
                            let mut end = start;
                            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'-') {
                                end += 1;
                            }
                            &cmd[start..end]
                        }
                    };
                    if !delim.is_empty() {
                        out.push(HeredocSpan {
                            delimiter: delim.to_string(),
                            index: i,
                            statement_start: stmt_start,
                        });
                    }
                }
            }
        }
        i += 1;
    }
    out
}

pub fn split_top_level_statements(cmd: &str) -> Vec<String> {
    let bytes = cmd.as_bytes();
    let mut stmts = Vec::new();
    let (mut state, mut start, mut i) = (0u8, 0usize, 0usize);
    while i < bytes.len() {
        let c = bytes[i];
        let next = bytes.get(i + 1).copied().unwrap_or(0);
        if state == 1 {
            if c == b'\'' {
                state = 0;
            }
        } else if state == 2 {
            if c == b'\\' && i + 1 < bytes.len() {
                i += next_char_len(cmd, i + 1);
            } else if c == b'"' {
                state = 0;
            }
        } else if c == b'\'' {
            state = 1;
        } else if c == b'"' {
            state = 2;
        } else if c == b'\\' && i + 1 < bytes.len() {
            i += next_char_len(cmd, i + 1);
        } else if c == b'$' && next == b'(' && bytes.get(i + 2) != Some(&b'(') {
            let (mut depth, mut j) = (1i32, i + 2);
            while j < bytes.len() && depth > 0 {
                if bytes[j] == b'\\' && j + 1 < bytes.len() {
                    j += 1 + next_char_len(cmd, j + 1);
                    continue;
                }
                if bytes[j] == b'(' {
                    depth += 1;
                } else if bytes[j] == b')' {
                    depth -= 1;
                }
                j += 1;
            }
            let end_idx = if depth == 0 { j - 1 } else { bytes.len() };
            let inner = cmd.get(i + 2..end_idx).unwrap_or("").trim();
            if !inner.is_empty() {
                stmts.extend(split_top_level_statements(inner));
            }
            i = j;
            continue;
        } else if c == b';' || c == b'\n' || c == b'\r' || c == b'(' || c == b')' || (c == b'&' && next == b'&') || (c == b'|' && next == b'|') {
            let seg = cmd[start..i].trim();
            if !seg.is_empty() {
                stmts.push(seg.to_string());
            }
            if (c == b'&' && next == b'&') || (c == b'|' && next == b'|') {
                i += 1;
            }
            start = i + 1;
        }
        i += 1;
    }
    let tail = cmd[start..].trim();
    if !tail.is_empty() {
        stmts.push(tail.to_string());
    }
    stmts
}

fn split_pipeline(stmt: &str) -> Vec<String> {
    let bytes = stmt.as_bytes();
    let (mut stages, mut state, mut start, mut i) = (Vec::new(), 0u8, 0usize, 0usize);
    while i < bytes.len() {
        let c = bytes[i];
        if state == 1 {
            if c == b'\'' {
                state = 0;
            }
        } else if state == 2 {
            if c == b'\\' && i + 1 < bytes.len() {
                i += next_char_len(stmt, i + 1);
            } else if c == b'"' {
                state = 0;
            }
        } else if c == b'\'' {
            state = 1;
        } else if c == b'"' {
            state = 2;
        } else if c == b'\\' && i + 1 < bytes.len() {
            i += next_char_len(stmt, i + 1);
        } else if c == b'|' && bytes.get(i + 1) != Some(&b'|') && (i == 0 || bytes[i - 1] != b'|') {
            let s = stmt[start..i].trim();
            if !s.is_empty() {
                stages.push(s.to_string());
            }
            start = i + 1;
        }
        i += 1;
    }
    let tail = stmt[start..].trim();
    if !tail.is_empty() {
        stages.push(tail.to_string());
    }
    stages
}

fn extract_redir_target(s: &str, start: usize) -> (String, usize) {
    let bytes = s.as_bytes();
    let mut j = start;
    while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
        j += 1;
    }
    if j >= bytes.len() {
        return (String::new(), j);
    }
    if bytes[j] == b'\'' || bytes[j] == b'"' {
        let q = bytes[j];
        j += 1;
        let beg = j;
        while j < bytes.len() && bytes[j] != q {
            if q == b'"' && bytes[j] == b'\\' && j + 1 < bytes.len() {
                j += 1 + next_char_len(s, j + 1);
                continue;
            }
            j += next_char_len(s, j);
        }
        let out = s.get(beg..j.min(bytes.len())).unwrap_or("").to_string();
        return (out, (j + 1).min(bytes.len()));
    }
    let beg = j;
    while j < bytes.len() && !matches!(bytes[j], b' ' | b'\t' | b'>' | b'<' | b';' | b'&' | b'|') {
        if bytes[j] == b'\\' && j + 1 < bytes.len() {
            j += 1 + next_char_len(s, j + 1);
            continue;
        }
        j += next_char_len(s, j);
    }
    (s.get(beg..j.min(bytes.len())).unwrap_or("").to_string(), j.min(bytes.len()))
}

fn parse_stage(stage: &str) -> ParsedStage {
    let mut ps = ParsedStage::default();
    let mut redir_in = Vec::new();
    let bytes = stage.as_bytes();
    let (mut state, mut cur, mut i) = (0u8, String::new(), 0usize);

    while i < bytes.len() {
        let c = bytes[i];
        let next = bytes.get(i + 1).copied().unwrap_or(0);
        if state == 1 {
            if c == b'\'' {
                state = 0;
                i += 1;
            } else {
                let ch = stage[i..].chars().next().unwrap_or(c as char);
                cur.push(ch);
                i += ch.len_utf8();
            }
            continue;
        }
        if state == 2 {
            if c == b'\\' && i + 1 < bytes.len() {
                let ch = stage[i + 1..].chars().next().unwrap_or(next as char);
                cur.push(ch);
                i += 1 + ch.len_utf8();
            } else if c == b'"' {
                state = 0;
                i += 1;
            } else {
                let ch = stage[i..].chars().next().unwrap_or(c as char);
                cur.push(ch);
                i += ch.len_utf8();
            }
            continue;
        }
        if c == b'\'' {
            state = 1;
            i += 1;
        } else if c == b'"' {
            state = 2;
            i += 1;
        } else if c == b'\\' && i + 1 < bytes.len() {
            let ch = stage[i + 1..].chars().next().unwrap_or(next as char);
            cur.push(ch);
            i += 1 + ch.len_utf8();
        } else if c == b'<' && next == b'<' && bytes.get(i + 2) == Some(&b'<') {
            ps.has_here_string = true;
            if !cur.is_empty() {
                ps.tokens.push(std::mem::take(&mut cur));
            }
            let (_, end) = extract_redir_target(stage, i + 3);
            i = end;
        } else if c == b'<' && next == b'<' {
            ps.has_heredoc = true;
            if !cur.is_empty() {
                ps.tokens.push(std::mem::take(&mut cur));
            }
            i += 2;
        } else if c == b'&' && next == b'>' {
            if !cur.is_empty() {
                ps.tokens.push(std::mem::take(&mut cur));
            }
            ps.has_redirect_out = true;
            let op_len = if bytes.get(i + 2) == Some(&b'>') { 3 } else { 2 };
            let (_, end) = extract_redir_target(stage, i + op_len);
            i = end;
        } else if c == b'>' {
            let is_dup = next == b'&';
            let is_append = next == b'>';
            let op_len = if is_dup || is_append { 2 } else { 1 };
            let is_err_fd = cur == "2";
            if is_err_fd || cur == "1" {
                cur.clear();
            } else if !cur.is_empty() {
                ps.tokens.push(std::mem::take(&mut cur));
            }
            if !is_err_fd && !is_dup {
                ps.has_redirect_out = true;
            }
            let (_, end) = extract_redir_target(stage, i + op_len);
            i = end;
        } else if c == b'<' {
            if cur == "0" {
                cur.clear();
            } else if !cur.is_empty() {
                ps.tokens.push(std::mem::take(&mut cur));
            }
            let (t, end) = extract_redir_target(stage, i + 1);
            if !t.is_empty() {
                redir_in.push(t);
            }
            i = end;
        } else if c.is_ascii_whitespace() {
            if !cur.is_empty() {
                ps.tokens.push(std::mem::take(&mut cur));
            }
            i += 1;
        } else {
            let ch = stage[i..].chars().next().unwrap_or(c as char);
            if !((ch == '{' || ch == '}') && cur.is_empty()) {
                cur.push(ch);
            }
            i += ch.len_utf8();
        }
    }
    if !cur.is_empty() {
        ps.tokens.push(cur);
    }
    finalize_parsed_stage(&mut ps, redir_in);
    ps
}

fn finalize_parsed_stage(ps: &mut ParsedStage, redir_in: Vec<String>) {
    let mut idx = 0usize;
    while idx < ps.tokens.len() && ps.tokens[idx].contains('=') && !ps.tokens[idx].starts_with('-') {
        idx += 1;
    }
    while idx < ps.tokens.len() {
        let base = ps.tokens[idx].rsplit('/').next().unwrap_or("").to_ascii_lowercase();
        if matches!(base.as_str(), "sudo" | "doas" | "nohup" | "time" | "exec" | "builtin") {
            idx += 1;
        } else if base == "command" {
            idx += 1;
            while idx < ps.tokens.len() && ps.tokens[idx].starts_with('-') {
                idx += 1;
            }
        } else if base == "env" {
            idx += 1;
            while idx < ps.tokens.len() {
                if matches!(ps.tokens[idx].as_str(), "-u" | "--unset") {
                    idx += 2;
                } else if ps.tokens[idx].starts_with('-') || ps.tokens[idx].contains('=') {
                    idx += 1;
                } else {
                    break;
                }
            }
        } else {
            break;
        }
    }

    let cmd = ps.tokens.get(idx).map(|t| t.rsplit('/').next().unwrap_or(t).to_ascii_lowercase());
    let args = if idx + 1 <= ps.tokens.len() { &ps.tokens[idx + 1..] } else { &[] };

    if let Some(ref c) = cmd {
        if FILE_READERS.contains(&c.as_str()) {
            if c == "tail" {
                ps.is_follow = args.iter().any(|a| a == "-f" || a == "-F" || a == "--follow" || a.starts_with("-f"));
            }
            if !ps.is_follow {
                let mut skip = false;
                for a in args {
                    if skip {
                        skip = false;
                        continue;
                    }
                    if matches!(c.as_str(), "head" | "tail") && matches!(a.as_str(), "-n" | "-c") {
                        skip = true;
                        continue;
                    }
                    if !a.starts_with('-') && !is_whitelisted_read_target(a) {
                        ps.target_files.push(a.clone());
                    }
                }
                for r in redir_in {
                    if !is_whitelisted_read_target(&r) {
                        ps.target_files.push(r);
                    }
                }
            }
        } else if c == "xargs" {
            let mut s = 0usize;
            while s < args.len() {
                if matches!(args[s].as_str(), "-I" | "-n" | "-L" | "-P" | "-s" | "-E" | "-d") {
                    s += 2;
                } else if args[s].starts_with('-') {
                    s += 1;
                } else {
                    break;
                }
            }
            if let Some(sub) = args.get(s).map(|t| t.rsplit('/').next().unwrap_or(t).to_ascii_lowercase()) {
                if FILE_READERS.contains(&sub.as_str()) {
                    ps.xargs_reader = Some(sub);
                }
            }
        } else if c == "find" {
            for w in args.windows(2) {
                if matches!(w[0].as_str(), "-exec" | "-execdir") {
                    let sub = w[1].rsplit('/').next().unwrap_or(&w[1]).to_ascii_lowercase();
                    if FILE_READERS.contains(&sub.as_str()) {
                        ps.find_exec_reader = Some(sub);
                        break;
                    }
                }
            }
        } else if matches!(c.as_str(), "sh" | "bash" | "zsh" | "dash" | "ksh") {
            for w in args.windows(2) {
                if w[0] == "-c" || (w[0].starts_with('-') && w[0].contains('c')) {
                    ps.subshell_advisory = ShellCommandGuard::detect_terminal_file_read(&w[1]);
                    break;
                }
            }
        }
    } else {
        for r in redir_in {
            if !is_whitelisted_read_target(&r) {
                ps.target_files.push(r);
            }
        }
    }
    ps.command = cmd;
}
