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

/// Truncates a string to fit within a token budget while preserving UTF-8 char boundaries.
pub fn truncate_text_to_token_budget(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(4);
    if text.len() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 16 {
        return text.chars().take(max_chars).collect();
    }
    let mut end = (max_chars - 15).min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...[truncated]", &text[..end])
}

/// Renders a deterministic unified diff between two text snapshots.
pub fn render_unified_diff(path: &str, old_text: &str, new_text: &str) -> String {
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    if old_text == new_text {
        return out;
    }
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    out.push_str(&format!("@@ -1,{} +1,{} @@\n", old_lines.len(), new_lines.len()));
    for line in &old_lines {
        out.push_str(&format!(
            "{}{line}\n",
            if new_lines.contains(line) { " " } else { "-" }
        ));
    }
    for line in &new_lines {
        if !old_lines.contains(line) {
            out.push_str(&format!("+{line}\n"));
        }
    }
    out
}

/// Maps model tool name variations across OpenAI, Anthropic, Gemini, Codex, and Cline to canonical names.
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
        "grep" | "search" | "search_files" | "ripgrep" | "rg" | "grep_search" => "grep_search",
        "find_file" | "findFiles" | "find_by_name" | "find_files_by_pattern" | "glob" | "glob_files" | "find_files" => {
            "find_files"
        }
        "stat" | "stat_file" | "fileInfo" | "path_exists" | "file_hash" | "file_info" => "file_info",
        "tree" | "directoryTree" | "get_workspace_file_tree" | "directory_tree" => "directory_tree",
        "remove_file" | "removeFile" | "deleteFile" | "rm" | "unlink" | "delete_file" => "delete_file",
        "rename_file" | "renameFile" | "moveFile" | "copy_file" | "mv" | "rename" | "move_file" => "move_file",
        "undo" | "rollback" | "rollback_turn_mutations" | "rollback_last_mutation" => "rollback_last_mutation",
        "search_codebase_symbols"
        | "get_file_outline"
        | "get_symbol_definition"
        | "get_symbol_references"
        | "search_symbols" => "search_symbols",
        "rename_symbol_across_codebase" | "search_and_replace" | "batch_replace_regex" => "batch_replace_regex",
        "workspace_diff" | "v4a_collect_working_diff" | "get_workspace_diff" => "get_workspace_diff",
        "memory_remember" | "remember" | "save_memory" => "save_memory",
        "memory_recall" | "recall" | "search_memory" => "search_memory",
        "spawn_subagent" | "execute_parallel_batch" | "engage_swarm" => "engage_swarm",
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

