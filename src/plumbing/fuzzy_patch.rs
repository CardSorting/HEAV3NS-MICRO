// [LAYER: PLUMBING]

pub type FuzzyHealMatch = (String, f64);
pub type PatchOperationList = Vec<PatchOperation>;

pub fn strip_quote_escapes(s: &str) -> String {
    s.replace("\\\"", "\"").replace("\\'", "'")
}

pub fn has_even_backslash_runs(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
            }
            let run_len = i - start;
            if run_len >= 2 && run_len % 2 == 0 {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

pub fn halve_even_backslash_runs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let mut count = 1usize;
            while chars.peek() == Some(&'\\') {
                chars.next();
                count += 1;
            }
            let target_count = if count >= 2 && count % 2 == 0 { count / 2 } else { count };
            for _ in 0..target_count {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Content-verified auto-correction for LLM tool-call escape drift (`escape_drift_autocorrect`).
/// Repairs spurious quote escapes (`\"`, `\'`) and doubled backslash runs (`\\`) when verified
/// against `content`, while rejecting ambiguous replacement backslashes (`AmbiguousEscapeDriftError`).
pub fn maybe_correct_escape_drift(
    old_string: &str,
    new_string: &str,
    content: &str,
) -> Result<(String, String, Option<String>), String> {
    if old_string.is_empty() || content.contains(old_string) {
        return Ok((old_string.to_string(), new_string.to_string(), None));
    }

    let old_q = strip_quote_escapes(old_string);
    if old_q != old_string && content.contains(&old_q) {
        let new_q = strip_quote_escapes(new_string);
        if has_even_backslash_runs(&old_q) {
            let old_qb = halve_even_backslash_runs(&old_q);
            if old_qb != old_q && content.contains(&old_qb) {
                if new_q.contains('\\') {
                    return Err(
                        "AmbiguousEscapeDriftError: old_string matches after quote+backslash unescape, but new_string contains backslashes that cannot be safely disambiguated"
                            .to_string(),
                    );
                }
                return Ok((old_qb, new_q, Some("quote_and_backslash_unescape".to_string())));
            }
        }
        return Ok((old_q, new_q, Some("quote_unescape".to_string())));
    }

    if old_q != old_string && has_even_backslash_runs(&old_q) {
        let old_qb = halve_even_backslash_runs(&old_q);
        if old_qb != old_q && content.contains(&old_qb) {
            let new_q = strip_quote_escapes(new_string);
            if new_q.contains('\\') {
                return Err(
                    "AmbiguousEscapeDriftError: old_string matches after quote+backslash unescape, but new_string contains backslashes that cannot be safely disambiguated"
                        .to_string(),
                );
            }
            return Ok((old_qb, new_q, Some("quote_and_backslash_unescape".to_string())));
        }
    }

    if has_even_backslash_runs(old_string) {
        let old_b = halve_even_backslash_runs(old_string);
        if old_b != old_string && content.contains(&old_b) {
            if new_string.contains('\\') {
                return Err(
                    "AmbiguousEscapeDriftError: old_string matches after backslash-run halving, but new_string contains backslashes that cannot be safely disambiguated"
                        .to_string(),
                );
            }
            return Ok((old_b, new_string.to_string(), Some("backslash_halving".to_string())));
        }
    }

    Ok((old_string.to_string(), new_string.to_string(), None))
}

/// Whitespace-tolerant and escape-drift-healing fuzzy patch locator (`ToolErrorAutoHealer::healFuzzyPatch`).
pub fn heal_fuzzy_patch(file_content: &str, target_snippet: &str) -> Option<(String, f64)> {
    if file_content.is_empty() || target_snippet.is_empty() {
        return None;
    }
    if file_content.contains(target_snippet) {
        return Some((target_snippet.to_string(), 1.0));
    }
    let unescaped_q = strip_quote_escapes(target_snippet);
    if unescaped_q != target_snippet && file_content.contains(&unescaped_q) {
        return Some((unescaped_q, 0.98));
    }
    if has_even_backslash_runs(&unescaped_q) {
        let halved = halve_even_backslash_runs(&unescaped_q);
        if halved != unescaped_q && file_content.contains(&halved) {
            return Some((halved, 0.97));
        }
    }
    let file_lines: Vec<&str> = file_content.lines().collect();
    let target_lines: Vec<&str> = target_snippet.lines().filter(|l| !l.trim().is_empty()).collect();
    if target_lines.is_empty() || file_lines.len() < target_lines.len() {
        return None;
    }

    for i in 0..=(file_lines.len() - target_lines.len()) {
        if (0..target_lines.len()).all(|j| file_lines[i + j].trim() == target_lines[j].trim()) {
            let matched = file_lines[i..i + target_lines.len()].join("\n");
            if file_content.contains(&matched) {
                return Some((matched, 0.95));
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct PatchHunkLine {
    pub prefix: char,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct PatchOperation {
    pub op_type: &'static str,
    pub file_path: String,
    pub hunks: Vec<Vec<PatchHunkLine>>,
    pub add_content: Option<String>,
}

/// Deterministic Unified Diff and V4A Multi-File Patch Parser and Applicator.
pub struct DeterministicPatchEngine;

impl DeterministicPatchEngine {
    pub fn parse(patch_text: &str) -> Vec<PatchOperation> {
        if patch_text.contains("*** Begin Patch")
            || patch_text.contains("*** Update File:")
            || patch_text.contains("*** Add File:")
        {
            Self::parse_v4a(patch_text)
        } else {
            Self::parse_unified(patch_text)
        }
    }

    pub fn parse_v4a(patch_text: &str) -> Vec<PatchOperation> {
        let mut ops = Vec::new();
        let mut cur_type: Option<&'static str> = None;
        let mut cur_path = String::new();
        let mut cur_hunks: Vec<Vec<PatchHunkLine>> = Vec::new();
        let mut cur_hunk: Vec<PatchHunkLine> = Vec::new();
        let mut add_lines: Vec<String> = Vec::new();

        let flush = |ops: &mut Vec<PatchOperation>,
                     cur_type: &mut Option<&'static str>,
                     cur_path: &mut String,
                     cur_hunks: &mut Vec<Vec<PatchHunkLine>>,
                     cur_hunk: &mut Vec<PatchHunkLine>,
                     add_lines: &mut Vec<String>| {
            if !cur_hunk.is_empty() {
                cur_hunks.push(std::mem::take(cur_hunk));
            }
            if let Some(t) = cur_type.take() {
                if !cur_path.is_empty() {
                    let add_content = if t == "add" {
                        Some(std::mem::take(add_lines).join("\n") + "\n")
                    } else {
                        None
                    };
                    ops.push(PatchOperation {
                        op_type: t,
                        file_path: std::mem::take(cur_path),
                        hunks: std::mem::take(cur_hunks),
                        add_content,
                    });
                }
            }
            cur_hunk.clear();
            cur_hunks.clear();
            add_lines.clear();
        };

        for line in patch_text.lines() {
            let trimmed = line.trim();
            if trimmed == "*** Begin Patch" || trimmed == "*** End Patch" {
                if trimmed == "*** End Patch" {
                    flush(&mut ops, &mut cur_type, &mut cur_path, &mut cur_hunks, &mut cur_hunk, &mut add_lines);
                }
                continue;
            }
            if let Some(p) = trimmed.strip_prefix("*** Update File:").or_else(|| trimmed.strip_prefix("*** File:")) {
                flush(&mut ops, &mut cur_type, &mut cur_path, &mut cur_hunks, &mut cur_hunk, &mut add_lines);
                cur_type = Some("update");
                cur_path = p.trim().to_string();
                continue;
            }
            if let Some(p) = trimmed.strip_prefix("*** Add File:") {
                flush(&mut ops, &mut cur_type, &mut cur_path, &mut cur_hunks, &mut cur_hunk, &mut add_lines);
                cur_type = Some("add");
                cur_path = p.trim().to_string();
                continue;
            }
            if let Some(p) = trimmed.strip_prefix("*** Delete File:") {
                flush(&mut ops, &mut cur_type, &mut cur_path, &mut cur_hunks, &mut cur_hunk, &mut add_lines);
                cur_type = Some("delete");
                cur_path = p.trim().to_string();
                continue;
            }
            if trimmed.starts_with("@@") {
                if !cur_hunk.is_empty() {
                    cur_hunks.push(std::mem::take(&mut cur_hunk));
                }
                continue;
            }
            match cur_type {
                Some("add") => {
                    add_lines.push(line.strip_prefix('+').unwrap_or(line).to_string());
                }
                Some("update") => {
                    if let Some(first) = line.chars().next() {
                        if matches!(first, ' ' | '-' | '+') {
                            cur_hunk.push(PatchHunkLine {
                                prefix: first,
                                content: line[first.len_utf8()..].to_string(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        flush(&mut ops, &mut cur_type, &mut cur_path, &mut cur_hunks, &mut cur_hunk, &mut add_lines);
        ops
    }

    pub fn parse_unified(diff_text: &str) -> Vec<PatchOperation> {
        let mut ops = Vec::new();
        let mut old_file = String::new();
        let mut cur_file = String::new();
        let mut cur_type: &'static str = "update";
        let mut is_old_null = false;
        let mut cur_hunks: Vec<Vec<PatchHunkLine>> = Vec::new();
        let mut cur_hunk: Vec<PatchHunkLine> = Vec::new();

        let flush = |ops: &mut Vec<PatchOperation>,
                     cur_file: &mut String,
                     cur_type: &mut &'static str,
                     cur_hunks: &mut Vec<Vec<PatchHunkLine>>,
                     cur_hunk: &mut Vec<PatchHunkLine>| {
            if !cur_hunk.is_empty() {
                cur_hunks.push(std::mem::take(cur_hunk));
            }
            if !cur_file.is_empty() && (!cur_hunks.is_empty() || *cur_type == "delete") {
                let add_content = if *cur_type == "add" {
                    let lines: Vec<String> = cur_hunks
                        .iter()
                        .flatten()
                        .filter(|l| l.prefix == '+')
                        .map(|l| l.content.clone())
                        .collect();
                    Some(lines.join("\n") + "\n")
                } else {
                    None
                };
                ops.push(PatchOperation {
                    op_type: *cur_type,
                    file_path: std::mem::take(cur_file),
                    hunks: std::mem::take(cur_hunks),
                    add_content,
                });
            }
            cur_hunk.clear();
            cur_hunks.clear();
        };

        for line in diff_text.lines() {
            if let Some(rest) = line.strip_prefix("--- ") {
                flush(&mut ops, &mut cur_file, &mut cur_type, &mut cur_hunks, &mut cur_hunk);
                let p = rest.trim();
                is_old_null = p == "/dev/null";
                old_file = p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(p).to_string();
                continue;
            }
            if let Some(rest) = line.strip_prefix("+++ ") {
                let p = rest.trim();
                let is_new_null = p == "/dev/null";
                let new_file = p.strip_prefix("b/").or_else(|| p.strip_prefix("a/")).unwrap_or(p).to_string();
                cur_file = if is_new_null { old_file.clone() } else { new_file };
                cur_type = if is_old_null {
                    "add"
                } else if is_new_null {
                    "delete"
                } else {
                    "update"
                };
                continue;
            }
            if line.starts_with("@@ ") {
                if !cur_hunk.is_empty() {
                    cur_hunks.push(std::mem::take(&mut cur_hunk));
                }
                continue;
            }
            if !cur_file.is_empty() {
                if let Some(first) = line.chars().next() {
                    if matches!(first, ' ' | '-' | '+') {
                        cur_hunk.push(PatchHunkLine {
                            prefix: first,
                            content: line[first.len_utf8()..].to_string(),
                        });
                    }
                }
            }
        }
        flush(&mut ops, &mut cur_file, &mut cur_type, &mut cur_hunks, &mut cur_hunk);
        ops
    }

    pub fn apply_hunks(original: &str, hunks: &[Vec<PatchHunkLine>]) -> Result<String, String> {
        let mut current = original.to_string();
        for (idx, hunk) in hunks.iter().enumerate() {
            let target_lines: Vec<&str> = hunk.iter().filter(|l| l.prefix != '+').map(|l| l.content.as_str()).collect();
            let replacement_lines: Vec<&str> =
                hunk.iter().filter(|l| l.prefix != '-').map(|l| l.content.as_str()).collect();
            let target_block = target_lines.join("\n");
            let replacement_block = replacement_lines.join("\n");
            if target_block.is_empty() {
                current.push_str(&replacement_block);
                continue;
            }
            if let Some((matched, _)) = heal_fuzzy_patch(&current, &target_block) {
                current = current.replacen(&matched, &replacement_block, 1);
            } else {
                return Err(format!("Hunk #{} failed to match in target file", idx + 1));
            }
        }
        Ok(current)
    }
}
