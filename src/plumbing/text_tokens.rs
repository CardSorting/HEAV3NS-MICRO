// [LAYER: PLUMBING]

pub type TokenCount = usize;
pub type UnifiedDiffText = String;

/// Estimates token count using a fast deterministic 4-bytes-per-token heuristic.
#[inline]
pub fn estimate_text_tokens(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        (text.len() + 3) / 4
    }
}

/// Returns the largest valid UTF-8 prefix of `text` whose byte length is `<= max_bytes`.
#[inline]
pub fn safe_utf8_prefix(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Returns a guaranteed non-panicking UTF-8 substring bounded by `[start_bytes..end_bytes]`,
/// snapping `start_bytes` upward (or downward if at end) and `end_bytes` downward to valid char boundaries.
#[inline]
pub fn safe_utf8_slice(text: &str, start_bytes: usize, end_bytes: usize) -> &str {
    let len = text.len();
    let mut start = start_bytes.min(len);
    while start < len && !text.is_char_boundary(start) {
        start += 1;
    }
    let mut end = end_bytes.min(len);
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end < start {
        return "";
    }
    &text[start..end]
}

/// Masks a secret or API key using Unicode scalar counts rather than raw byte offsets so non-ASCII inputs never panic.
pub fn mask_secret_preview(secret: &str) -> String {
    let char_count = secret.chars().count();
    if char_count == 0 {
        return "(empty)".to_string();
    }
    if char_count <= 8 {
        return "****".to_string();
    }
    let prefix: String = secret.chars().take(4).collect();
    let suffix: String = secret.chars().skip(char_count - 4).collect();
    format!("{prefix}...{suffix}")
}

/// Truncates a string to fit within a token budget while preserving UTF-8 char boundaries.
pub fn truncate_text_to_token_budget(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(4);
    if text.len() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 16 {
        return text.chars().take(max_chars).collect();
    }
    let prefix = safe_utf8_prefix(text, max_chars - 15);
    format!("{prefix}...[truncated]")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffOp {
    Equal(usize, usize),
    Delete(usize),
    Insert(usize),
}

fn compute_positional_diff_ops<'a>(old_lines: &[&'a str], new_lines: &[&'a str]) -> Vec<DiffOp> {
    let mut prefix = 0usize;
    while prefix < old_lines.len() && prefix < new_lines.len() && old_lines[prefix] == new_lines[prefix] {
        prefix += 1;
    }
    let mut old_end = old_lines.len();
    let mut new_end = new_lines.len();
    while old_end > prefix && new_end > prefix && old_lines[old_end - 1] == new_lines[new_end - 1] {
        old_end -= 1;
        new_end -= 1;
    }

    let mut ops = Vec::with_capacity(old_lines.len().max(new_lines.len()));
    for i in 0..prefix {
        ops.push(DiffOp::Equal(i, i));
    }

    let m = old_end - prefix;
    let n = new_end - prefix;
    if m == 0 {
        for j in prefix..new_end {
            ops.push(DiffOp::Insert(j));
        }
    } else if n == 0 {
        for i in prefix..old_end {
            ops.push(DiffOp::Delete(i));
        }
    } else if m.saturating_mul(n) <= 2_500_000 {
        let stride = n + 1;
        let mut dp = vec![0u32; (m + 1) * stride];
        for i in (0..m).rev() {
            for j in (0..n).rev() {
                dp[i * stride + j] = if old_lines[prefix + i] == new_lines[prefix + j] {
                    dp[(i + 1) * stride + (j + 1)] + 1
                } else {
                    dp[(i + 1) * stride + j].max(dp[i * stride + (j + 1)])
                };
            }
        }
        let (mut i, mut j) = (0usize, 0usize);
        while i < m && j < n {
            if old_lines[prefix + i] == new_lines[prefix + j] {
                ops.push(DiffOp::Equal(prefix + i, prefix + j));
                i += 1;
                j += 1;
            } else if dp[(i + 1) * stride + j] >= dp[i * stride + (j + 1)] {
                ops.push(DiffOp::Delete(prefix + i));
                i += 1;
            } else {
                ops.push(DiffOp::Insert(prefix + j));
                j += 1;
            }
        }
        while i < m {
            ops.push(DiffOp::Delete(prefix + i));
            i += 1;
        }
        while j < n {
            ops.push(DiffOp::Insert(prefix + j));
            j += 1;
        }
    } else {
        for i in prefix..old_end {
            ops.push(DiffOp::Delete(i));
        }
        for j in prefix..new_end {
            ops.push(DiffOp::Insert(j));
        }
    }

    let suffix_len = old_lines.len() - old_end;
    for k in 0..suffix_len {
        ops.push(DiffOp::Equal(old_end + k, new_end + k));
    }
    ops
}

/// Computes exact positional `(lines_added, lines_removed)` between two text snapshots.
pub fn count_diff_line_stats(old_text: &str, new_text: &str) -> (usize, usize) {
    if old_text == new_text {
        return (0, 0);
    }
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    let ops = compute_positional_diff_ops(&old_lines, &new_lines);
    let mut added = 0usize;
    let mut removed = 0usize;
    for op in ops {
        match op {
            DiffOp::Insert(_) => added += 1,
            DiffOp::Delete(_) => removed += 1,
            DiffOp::Equal(_, _) => {}
        }
    }
    (added, removed)
}

/// Renders a positional LCS hunked unified diff between two text snapshots (preserving duplicate and moved lines).
pub fn render_unified_diff(path: &str, old_text: &str, new_text: &str) -> String {
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    if old_text == new_text {
        return out;
    }
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    let ops = compute_positional_diff_ops(&old_lines, &new_lines);
    const CTX: usize = 3;

    let mut change_indices = Vec::new();
    for (idx, op) in ops.iter().enumerate() {
        if !matches!(op, DiffOp::Equal(_, _)) {
            change_indices.push(idx);
        }
    }
    if change_indices.is_empty() {
        return out;
    }

    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for idx in change_indices {
        let start = idx.saturating_sub(CTX);
        let end = (idx + CTX + 1).min(ops.len());
        if let Some(last) = hunks.last_mut() {
            if start <= last.1 {
                last.1 = end;
                continue;
            }
        }
        hunks.push((start, end));
    }

    for (h_start, h_end) in hunks {
        let mut old_start = None;
        let mut new_start = None;
        let mut old_count = 0usize;
        let mut new_count = 0usize;
        let mut body = String::new();

        for op in &ops[h_start..h_end] {
            match *op {
                DiffOp::Equal(oi, ni) => {
                    old_start.get_or_insert(oi + 1);
                    new_start.get_or_insert(ni + 1);
                    old_count += 1;
                    new_count += 1;
                    body.push(' ');
                    body.push_str(old_lines[oi]);
                    body.push('\n');
                }
                DiffOp::Delete(oi) => {
                    old_start.get_or_insert(oi + 1);
                    old_count += 1;
                    body.push('-');
                    body.push_str(old_lines[oi]);
                    body.push('\n');
                }
                DiffOp::Insert(ni) => {
                    new_start.get_or_insert(ni + 1);
                    new_count += 1;
                    body.push('+');
                    body.push_str(new_lines[ni]);
                    body.push('\n');
                }
            }
        }

        let os = old_start.unwrap_or(if old_count == 0 { 0 } else { 1 });
        let ns = new_start.unwrap_or(if new_count == 0 { 0 } else { 1 });
        out.push_str(&format!("@@ -{os},{old_count} +{ns},{new_count} @@\n"));
        out.push_str(&body);
    }
    out
}

/// Maps model tool name variations across OpenAI, Anthropic, Gemini, Codex, Codewhale, and Cline to canonical names.
pub fn canonical_tool_name(name: &str) -> &str {
    match name.trim() {
        "read_file" | "readFile" | "read" | "cat" | "viewFile" | "view_file" => "view_file",
        "writeFile" | "write_to_file" | "create_file" | "createFile" | "write" | "write_file" => "write_file",
        "replaceFileContent" | "replace_in_file" | "replace_content" | "edit_file" | "editFile"
        | "heal_and_apply_patch" | "replace_file_content" => "replace_file_content",
        "multi_replace" | "multiReplaceFileContent" | "multi_replace_file_content" => "multi_replace_file_content",
        "edit_file_anchored" | "anchored_edit" | "hashline_edit" => "edit_file_anchored",
        "apply_patch" | "v4a_apply_patch" | "patch_file" | "apply_unified_diff" => "apply_unified_diff",
        "runCommand" | "execute_command" | "executeCommand" | "bash" | "sh" | "terminal" | "exec" | "run_command" => {
            "run_command"
        }
        "list_files" | "listFiles" | "list_directory" | "ls" | "dir" | "list_dir" => "list_dir",
        "grep" | "search" | "search_files" | "grep_files" | "ripgrep" | "rg" | "grep_search" => "grep_search",
        "find_file" | "findFiles" | "find_by_name" | "find_files_by_pattern" | "file_search" | "glob" | "glob_files" | "find_files" => {
            "find_files"
        }
        "stat" | "stat_file" | "fileInfo" | "path_exists" | "file_hash" | "file_info" => "file_info",
        "tree" | "directoryTree" | "get_workspace_file_tree" | "directory_tree" => "directory_tree",
        "remove_file" | "removeFile" | "deleteFile" | "rm" | "unlink" | "delete_file" => "delete_file",
        "rename_file" | "renameFile" | "moveFile" | "copy_file" | "mv" | "rename" | "move_file" => "move_file",
        "undo" | "rollback" | "revert_turn" | "rollback_turn_mutations" | "rollback_last_mutation" => "rollback_last_mutation",
        "search_codebase_symbols"
        | "get_file_outline"
        | "get_symbol_definition"
        | "get_symbol_references"
        | "search_symbols" => "search_symbols",
        "rename_symbol_across_codebase" | "search_and_replace" | "batch_replace_regex" => "batch_replace_regex",
        "workspace_diff" | "v4a_collect_working_diff" | "get_workspace_diff" => "get_workspace_diff",
        "memory_remember" | "remember" | "save_memory" => "save_memory",
        "memory_recall" | "recall" | "search_memory" => "search_memory",
        "spawn_subagent" | "agent" | "execute_parallel_batch" | "legion_classify" | "swarm_classify" | "engage_swarm" => "engage_swarm",
        "execute_tools" | "batch_tools" | "run_tool_batch" | "codemode" => "execute_tools",
        "tool_search" | "discover_tools" | "find_tools" => "tool_search",
        "computer_action"
        | "computer_capture_frame"
        | "computer_click"
        | "computer_double_click"
        | "computer_right_click"
        | "computer_middle_click"
        | "computer_key"
        | "computer_wait"
        | "computer_type"
        | "computer_drag"
        | "computer_scroll"
        | "computer_set_value"
        | "computer_focus_window"
        | "computer_focus_app"
        | "computer_list_windows"
        | "computer_list_apps"
        | "computer_list_elements"
        | "computer_audit_health"
        | "computer_get_metrics"
        | "computer_search_dsl"
        | "computer_undo"
        | "computer_redo"
        | "computer_lease"
        | "drive_preview"
        | "annotate_preview"
        | "desktop_preview"
        | "read_window_below"
        | "gui_action"
        | "computer_use" => "computer_use",
        "browser_navigate"
        | "browser_snapshot"
        | "browser_click"
        | "browser_type"
        | "browser_dialog"
        | "browser_eval"
        | "cdp_navigate"
        | "cdp_snapshot"
        | "browser_use" => "browser_use",
        "vision_describe" | "vision_generate" | "inspect_image" | "decode_image" | "vision_inspect" => "vision_inspect",
        "skill_forge_custom"
        | "skill_lint_node"
        | "skill_autofix_node"
        | "skill_sync_directory"
        | "soul_forge_custom"
        | "soul_lint_profile"
        | "soul_autofix_profile"
        | "skill_tree"
        | "soul_vault"
        | "skill_forge" => "skill_forge",
        "spatial_action"
        | "spatial_ground"
        | "spatial_execute"
        | "spatial_query"
        | "spatial_plan"
        | "spatial_interact" => "spatial_interact",
        other => other,
    }
}

/// Encodes arbitrary bytes into standard RFC 4648 Base64 without external dependencies.
pub fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    if bytes.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(((bytes.len() + 2) / 3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
