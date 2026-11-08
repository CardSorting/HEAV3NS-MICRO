// [LAYER: PLUMBING]

pub type FuzzyHealMatch = (String, f64);
pub type PatchOperationList = Vec<PatchOperation>;

/// Whitespace-tolerant fuzzy patch locator (`ToolErrorAutoHealer::healFuzzyPatch`).
pub fn heal_fuzzy_patch(file_content: &str, target_snippet: &str) -> Option<(String, f64)> {
    if file_content.is_empty() || target_snippet.is_empty() {
        return None;
    }
    if file_content.contains(target_snippet) {
        return Some((target_snippet.to_string(), 1.0));
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
