// [LAYER: INFRASTRUCTURE]
use crate::infrastructure::search_tools::{batch_replace_regex_files, build_directory_tree, execute_grep_search, extract_workspace_symbols, resolve_workspace_path, suggest_nearby_files, walk_find_files};
use crate::infrastructure::transaction_journal::TransactionJournal;
use crate::infrastructure::vfs_store::SkillsIngestor;
use crate::plumbing::fuzzy_patch::{heal_fuzzy_patch, DeterministicPatchEngine};
use crate::plumbing::hash_and_math::fnv1a_32;
use crate::plumbing::secret_redactor::SafetyPolicy;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::sync::Mutex;

/// Dispatches native filesystem, diff, symbol, grep, and skill tools against the workspace.
pub fn dispatch_fs_tool(
    canonical: &str,
    args: &Map<String, Value>,
    cwd: &Path,
    journal: &Mutex<TransactionJournal>,
) -> Result<(Value, bool), String> {
    match canonical {
        "view_file" => {
            let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
            let resolved = resolve_workspace_path(cwd, raw_path);
            SafetyPolicy::check_path(&resolved)?;
            let content = fs::read_to_string(&resolved).map_err(|e| {
                let nearby = suggest_nearby_files(&resolved);
                if nearby.is_empty() {
                    format!("Failed to read '{}': {e}", resolved.display())
                } else {
                    format!(
                        "Failed to read '{}': {e}. Nearby files: {:?}",
                        resolved.display(),
                        nearby
                    )
                }
            })?;
            let offset = args.get("contentOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let sliced = if offset > 0 && offset < content.len() {
                &content[offset..]
            } else {
                content.as_str()
            };
            let lines: Vec<&str> = sliced.lines().collect();
            let total_lines = lines.len();
            let start_line = args.get("startLine").and_then(|v| v.as_u64()).unwrap_or(1).max(1) as usize;
            let end_line = args
                .get("endLine")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(total_lines)
                .min(total_lines);
            let out_content = if total_lines == 0 || start_line > end_line {
                String::new()
            } else {
                lines[start_line - 1..end_line].join("\n")
            };
            Ok((
                json!({
                    "path": resolved.to_string_lossy(),
                    "content": out_content,
                    "startLine": start_line,
                    "endLine": end_line,
                    "totalLines": total_lines
                }),
                false,
            ))
        }
        "write_file" => {
            let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = resolve_workspace_path(cwd, raw_path);
            SafetyPolicy::check_path(&resolved)?;
            if let Ok(mut j) = journal.lock() {
                j.record_mutation("write_file", &resolved);
            }
            if let Some(parent) = resolved.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dirs: {e}"))?;
            }
            fs::write(&resolved, content.as_bytes())
                .map_err(|e| format!("Failed to write '{}': {e}", resolved.display()))?;
            Ok((
                json!({
                    "success": true,
                    "path": resolved.to_string_lossy(),
                    "bytesWritten": content.len()
                }),
                false,
            ))
        }
        "replace_file_content" => {
            let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
            let target = args.get("target").and_then(|v| v.as_str()).ok_or("Missing 'target'")?;
            let replacement = args.get("replacement").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = resolve_workspace_path(cwd, raw_path);
            SafetyPolicy::check_path(&resolved)?;
            let original =
                fs::read_to_string(&resolved).map_err(|e| format!("Failed to read '{}': {e}", resolved.display()))?;
            let (matched_target, confidence) = heal_fuzzy_patch(&original, target).ok_or_else(|| {
                format!("Target content not found in '{raw_path}' (even with whitespace-tolerant fuzzy matching)")
            })?;
            if let Ok(mut j) = journal.lock() {
                j.record_mutation("replace_file_content", &resolved);
            }
            let updated = original.replacen(&matched_target, replacement, 1);
            fs::write(&resolved, updated.as_bytes())
                .map_err(|e| format!("Failed to write '{}': {e}", resolved.display()))?;
            Ok((
                json!({
                    "success": true,
                    "path": resolved.to_string_lossy(),
                    "confidence": confidence,
                    "autoHealed": confidence < 1.0
                }),
                false,
            ))
        }
        "multi_replace_file_content" => {
            let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
            let chunks = args
                .get("chunks")
                .and_then(|v| v.as_array())
                .ok_or("Missing 'chunks' array")?;
            if chunks.is_empty() {
                return Err("No replacement chunks provided".to_string());
            }
            let resolved = resolve_workspace_path(cwd, raw_path);
            SafetyPolicy::check_path(&resolved)?;
            let mut current =
                fs::read_to_string(&resolved).map_err(|e| format!("Failed to read '{}': {e}", resolved.display()))?;
            let mut applied = 0usize;
            for (idx, chunk) in chunks.iter().enumerate() {
                let obj = chunk.as_object().ok_or_else(|| format!("Chunk #{idx} is not an object"))?;
                let target = obj
                    .get("target")
                    .or_else(|| obj.get("targetContent"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("Chunk #{idx} missing 'target'"))?;
                let replacement = obj
                    .get("replacement")
                    .or_else(|| obj.get("replacementContent"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let (matched, _) = heal_fuzzy_patch(&current, target)
                    .ok_or_else(|| format!("Chunk #{idx} target content not found in '{raw_path}'"))?;
                current = current.replacen(&matched, replacement, 1);
                applied += 1;
            }
            if let Ok(mut j) = journal.lock() {
                j.record_mutation("multi_replace_file_content", &resolved);
            }
            fs::write(&resolved, current.as_bytes())
                .map_err(|e| format!("Failed to write '{}': {e}", resolved.display()))?;
            Ok((
                json!({
                    "success": true,
                    "path": resolved.to_string_lossy(),
                    "replacementsApplied": applied
                }),
                false,
            ))
        }
        "edit_file_anchored" => {
            let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
            let line_num = args.get("line").and_then(|v| v.as_u64()).ok_or("Missing 'line'")? as usize;
            let expected_anchor = args
                .get("expectedAnchor")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'expectedAnchor'")?;
            let replacement = args.get("replacement").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = resolve_workspace_path(cwd, raw_path);
            SafetyPolicy::check_path(&resolved)?;
            let original =
                fs::read_to_string(&resolved).map_err(|e| format!("Failed to read '{}': {e}", resolved.display()))?;
            let mut lines: Vec<String> = original.lines().map(|s| s.to_string()).collect();
            if line_num == 0 || line_num > lines.len() {
                return Err(format!(
                    "Line {line_num} out of bounds (file has {} lines)",
                    lines.len()
                ));
            }
            let actual_hash = format!("{:08x}", fnv1a_32(lines[line_num - 1].trim().as_bytes()));
            if !actual_hash.eq_ignore_ascii_case(expected_anchor.trim()) {
                return Err(format!(
                    "Anchor mismatch at line {line_num}: expected '{expected_anchor}', got '{actual_hash}'"
                ));
            }
            if let Ok(mut j) = journal.lock() {
                j.record_mutation("edit_file_anchored", &resolved);
            }
            lines[line_num - 1] = replacement.to_string();
            let mut updated = lines.join("\n");
            if original.ends_with('\n') {
                updated.push('\n');
            }
            fs::write(&resolved, updated.as_bytes())
                .map_err(|e| format!("Failed to write '{}': {e}", resolved.display()))?;
            Ok((
                json!({ "success": true, "path": resolved.to_string_lossy(), "line": line_num }),
                false,
            ))
        }
        "apply_unified_diff" => {
            let diff_text = args.get("diff").and_then(|v| v.as_str()).ok_or("Missing 'diff'")?;
            let ops = DeterministicPatchEngine::parse(diff_text);
            if ops.is_empty() {
                return Err("No valid Unified Diff or V4A patch operations found".to_string());
            }
            let (mut modified, mut errors) = (Vec::new(), Vec::new());
            for op in &ops {
                let resolved = resolve_workspace_path(cwd, &op.file_path);
                if let Err(e) = SafetyPolicy::check_path(&resolved) {
                    errors.push(e);
                    continue;
                }
                match op.op_type {
                    "delete" => {
                        if let Ok(mut j) = journal.lock() {
                            j.record_mutation("apply_unified_diff", &resolved);
                        }
                        let _ = fs::remove_file(&resolved);
                        modified.push(op.file_path.clone());
                    }
                    "add" => {
                        let content = op.add_content.clone().unwrap_or_default();
                        if let Ok(mut j) = journal.lock() {
                            j.record_mutation("apply_unified_diff", &resolved);
                        }
                        if let Some(p) = resolved.parent() {
                            let _ = fs::create_dir_all(p);
                        }
                        match fs::write(&resolved, content.as_bytes()) {
                            Ok(_) => modified.push(op.file_path.clone()),
                            Err(e) => errors.push(format!("Failed to add '{}': {e}", op.file_path)),
                        }
                    }
                    _ => match fs::read_to_string(&resolved) {
                        Ok(orig) => match DeterministicPatchEngine::apply_hunks(&orig, &op.hunks) {
                            Ok(updated) => {
                                if let Ok(mut j) = journal.lock() {
                                    j.record_mutation("apply_unified_diff", &resolved);
                                }
                                match fs::write(&resolved, updated.as_bytes()) {
                                    Ok(_) => modified.push(op.file_path.clone()),
                                    Err(e) => errors.push(format!("Failed to write '{}': {e}", op.file_path)),
                                }
                            }
                            Err(e) => errors.push(format!("Patch mismatch in '{}': {e}", op.file_path)),
                        },
                        Err(e) => errors.push(format!("Failed to read '{}': {e}", op.file_path)),
                    },
                }
            }
            if !errors.is_empty() && modified.is_empty() {
                return Err(errors.join("; "));
            }
            Ok((
                json!({ "success": errors.is_empty(), "modifiedFiles": modified, "errors": errors }),
                false,
            ))
        }
        "delete_file" => {
            let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
            let resolved = resolve_workspace_path(cwd, raw_path);
            SafetyPolicy::check_path(&resolved)?;
            if !resolved.exists() {
                return Err(format!("File '{}' does not exist", resolved.display()));
            }
            if let Ok(mut j) = journal.lock() {
                j.record_mutation("delete_file", &resolved);
            }
            if resolved.is_dir() {
                fs::remove_dir_all(&resolved).map_err(|e| format!("Failed to remove directory: {e}"))?;
            } else {
                fs::remove_file(&resolved).map_err(|e| format!("Failed to delete file: {e}"))?;
            }
            Ok((json!({ "success": true, "path": resolved.to_string_lossy() }), false))
        }
        "move_file" => {
            let src_raw = args.get("source").and_then(|v| v.as_str()).ok_or("Missing 'source'")?;
            let dst_raw = args.get("target").and_then(|v| v.as_str()).ok_or("Missing 'target'")?;
            let src = resolve_workspace_path(cwd, src_raw);
            let dst = resolve_workspace_path(cwd, dst_raw);
            SafetyPolicy::check_path(&src)?;
            SafetyPolicy::check_path(&dst)?;
            if let Some(parent) = dst.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::rename(&src, &dst)
                .map_err(|e| format!("Failed to move '{}' -> '{}': {e}", src.display(), dst.display()))?;
            Ok((
                json!({
                    "success": true,
                    "source": src.to_string_lossy(),
                    "target": dst.to_string_lossy()
                }),
                false,
            ))
        }
        "list_dir" => {
            let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let resolved = resolve_workspace_path(cwd, raw_path);
            let rd =
                fs::read_dir(&resolved).map_err(|e| format!("Failed to list '{}': {e}", resolved.display()))?;
            let mut entries = Vec::new();
            for entry in rd.flatten() {
                let meta = entry.metadata().ok();
                entries.push(json!({
                    "name": entry.file_name().to_string_lossy(),
                    "isDir": meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    "size": meta.as_ref().map(|m| m.len()).unwrap_or(0),
                }));
            }
            entries.sort_by(|a, b| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")));
            Ok((
                json!({ "path": resolved.to_string_lossy(), "entries": entries }),
                false,
            ))
        }
        "find_files" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let max_depth = args.get("maxDepth").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let resolved = resolve_workspace_path(cwd, raw_path);
            let mut matches = Vec::new();
            walk_find_files(&resolved, &resolved, &query, 0, max_depth, &mut matches);
            matches.sort();
            Ok((
                json!({ "query": query, "count": matches.len(), "files": matches }),
                false,
            ))
        }
        "file_info" => {
            let raw_path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing 'path'")?;
            let resolved = resolve_workspace_path(cwd, raw_path);
            if !resolved.exists() {
                return Ok((json!({ "exists": false, "path": resolved.to_string_lossy() }), false));
            }
            let meta = fs::metadata(&resolved).map_err(|e| format!("Stat failed: {e}"))?;
            let mut sha256_hex = None;
            let mut lines_count = None;
            if meta.is_file() && meta.len() <= 10 * 1024 * 1024 {
                if let Ok(bytes) = fs::read(&resolved) {
                    let mut hasher = Sha256::new();
                    hasher.update(&bytes);
                    sha256_hex = Some(format!("{:x}", hasher.finalize()));
                    lines_count = Some(bytes.iter().filter(|&&b| b == b'\n').count() + 1);
                }
            }
            Ok((
                json!({
                    "exists": true,
                    "path": resolved.to_string_lossy(),
                    "isDir": meta.is_dir(),
                    "sizeBytes": meta.len(),
                    "lines": lines_count,
                    "sha256": sha256_hex
                }),
                false,
            ))
        }
        "directory_tree" => {
            let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let max_depth = args.get("maxDepth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
            let resolved = resolve_workspace_path(cwd, raw_path);
            let mut lines = vec![resolved
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| resolved.to_string_lossy().into_owned())];
            build_directory_tree(&resolved, "", 0, max_depth, &mut lines);
            Ok((json!({ "tree": lines.join("\n") }), false))
        }
        "grep_search" => {
            let query = args.get("query").and_then(|v| v.as_str()).ok_or("Missing 'query'")?;
            let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let case_insensitive = args.get("caseInsensitive").and_then(|v| v.as_bool()).unwrap_or(false);
            let max_results = args.get("maxResults").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
            let resolved = resolve_workspace_path(cwd, raw_path);
            let hits = execute_grep_search(&resolved, query, case_insensitive, max_results)?;
            Ok((
                json!({ "query": query, "count": hits.len(), "matches": hits }),
                false,
            ))
        }
        "search_symbols" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let kind_filter = args.get("kind").and_then(|v| v.as_str()).map(|s| s.to_lowercase());
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(60) as usize;
            let resolved = resolve_workspace_path(cwd, raw_path);
            let symbols = extract_workspace_symbols(&resolved, cwd, &query, kind_filter.as_deref(), limit);
            Ok((json!({ "count": symbols.len(), "symbols": symbols }), false))
        }
        "batch_replace_regex" => {
            let find_pat = args.get("target").and_then(|v| v.as_str()).ok_or("Missing 'target'")?;
            let replace_with = args.get("replacement").and_then(|v| v.as_str()).unwrap_or("");
            let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let dry_run = args.get("dryRun").and_then(|v| v.as_bool()).unwrap_or(false);
            let resolved = resolve_workspace_path(cwd, raw_path);
            batch_replace_regex_files(cwd, &resolved, find_pat, replace_with, dry_run, journal)
        }
        "list_skills" => {
            let manifests = SkillsIngestor::discover(cwd);
            let list: Vec<Value> = manifests
                .into_iter()
                .map(|m| json!({ "name": m.name, "description": m.description, "path": m.path }))
                .collect();
            Ok((json!({ "count": list.len(), "skills": list }), false))
        }
        "use_skill" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'name'")?
                .trim();
            let manifests = SkillsIngestor::discover(cwd);
            if let Some(found) = manifests.iter().find(|m| m.name.eq_ignore_ascii_case(name)) {
                Ok((
                    json!({
                        "success": true,
                        "name": found.name,
                        "location": found.path,
                        "instructions": found.instructions
                    }),
                    false,
                ))
            } else {
                let available: Vec<String> = manifests.into_iter().map(|m| m.name).collect();
                Err(format!("Skill '{name}' not found. Available skills: {available:?}"))
            }
        }
        other => Err(format!("Unsupported filesystem tool '{other}'")),
    }
}
