// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::plumbing::hash_and_math::lock_unpoison;
use crate::plumbing::secret_redactor::SafetyPolicy;
use crate::plumbing::text_tokens::{encode_base64, render_unified_diff, safe_utf8_prefix};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

use crate::ui::git_remote_rpc::handle_git_remote_rpc;
use crate::ui::git_rpc::{current_git_branch, git_remote_sync_state, handle_git_rpc, is_git_repository, is_safe_rel_path, list_git_status_entries, read_git_diff_for_path};
use crate::ui::rpc_dispatch::{sha256_bytes_hex, validate_rpc_read_path};

pub fn handle_files_rpc(method: &str, params: &Value, engine: &AgentEngine) -> Value {
    let root = engine.workspace_root();
    match method {
        "workspace.branch" => {
            let head_path = root.join(".git/HEAD");
            let branch = if let Ok(head) = fs::read_to_string(head_path) {
                let trimmed = head.trim();
                if let Some(r) = trimmed.strip_prefix("ref: refs/heads/") {
                    format!("branch:{r}")
                } else if trimmed.len() >= 7 {
                    format!("detached:{}", safe_utf8_prefix(trimmed, 12))
                } else {
                    "branch:main".to_string()
                }
            } else {
                "unversioned".to_string()
            };
            json!({ "branch": branch })
        }
        "files.list" => {
            let rel = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let clean_rel = if rel == "." {
                ""
            } else {
                rel.trim_matches('/')
            };
            let query = params
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_lowercase();
            let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let candidate_dir = if clean_rel.is_empty() || clean_rel.contains("..") {
                root.to_path_buf()
            } else {
                root.join(clean_rel)
            };
            let target_dir = match (root.canonicalize(), candidate_dir.canonicalize()) {
                (Ok(cr), Ok(ct))
                    if ct.starts_with(&cr) && SafetyPolicy::check_path(&ct).is_ok() =>
                {
                    ct
                }
                _ => return json!({ "entries": [], "total": 0, "offset": offset, "more": false }),
            };
            let mut all_entries = Vec::new();
            if let Ok(rd) = fs::read_dir(&target_dir) {
                let mut raw: Vec<_> = rd.flatten().collect();
                raw.sort_by(|a, b| {
                    let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    b_dir
                        .cmp(&a_dir)
                        .then_with(|| a.file_name().cmp(&b.file_name()))
                });
                for entry in raw {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if matches!(
                        name.as_str(),
                        ".git" | "target" | ".build" | "node_modules" | ".DS_Store"
                    ) {
                        continue;
                    }
                    let ft = entry.file_type();
                    let is_dir = ft.as_ref().map(|t| t.is_dir()).unwrap_or(false);
                    let is_symlink = ft.as_ref().map(|t| t.is_symlink()).unwrap_or(false);
                    if is_symlink || SafetyPolicy::check_path(&entry.path()).is_err() {
                        continue;
                    }
                    let path_str = if clean_rel.is_empty() {
                        name.clone()
                    } else {
                        format!("{clean_rel}/{name}")
                    };
                    if !query.is_empty()
                        && !name.to_lowercase().contains(&query)
                        && !path_str.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    all_entries.push(json!({
                        "path": path_str,
                        "name": name,
                        "directory": is_dir,
                        "symlink": is_symlink,
                        "type": if is_dir { "directory" } else { "file" }
                    }));
                }
            }
            let matched = all_entries.len();
            let page_size = 100usize;
            let entries: Vec<Value> = all_entries
                .into_iter()
                .skip(offset)
                .take(page_size)
                .collect();
            let next_offset = offset + entries.len();
            json!({
                "entries": entries,
                "matched": matched,
                "more": next_offset < matched,
                "nextOffset": next_offset,
                "scanLimited": false
            })
        }
        "files.search" => {
            let query = params
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_lowercase();
            let mut matched = Vec::new();
            collect_search_files(root, root, &query, &mut matched, 0);
            matched.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| a.1.len().cmp(&b.1.len()))
                    .then_with(|| a.1.cmp(&b.1))
            });
            let total_matched = matched.len();
            let entries: Vec<Value> = matched.into_iter().take(60).map(|(_, _p, v)| v).collect();
            json!({
                "entries": entries,
                "matched": total_matched,
                "more": total_matched > 60,
                "scanLimited": false,
                "scope": "workspace"
            })
        }
        "files.read" => {
            let path = params
                .get("path")
                .or_else(|| params.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("README.md");
            let Some(full_path) = validate_rpc_read_path(root, path) else {
                return json!({ "text": "", "revision": "rev-blocked", "offset": 0, "nextOffset": 0, "size": 0, "more": false });
            };
            let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let limit = (params
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(65536) as usize)
                .clamp(1, 512 * 1024);
            let text_content = if fs::metadata(&full_path)
                .map(|m| m.len() <= 16 * 1024 * 1024)
                .unwrap_or(false)
            {
                fs::read_to_string(&full_path).unwrap_or_default()
            } else {
                String::new()
            };
            crate::ui::worker_handlers::paginate_text_response(&text_content, "rev", offset, limit)
        }
        "files.readBinary" => {
            let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let Some(full_path) = validate_rpc_read_path(root, path) else {
                return json!({ "data": "", "revision": "bin-blocked", "offset": 0, "nextOffset": 0, "size": 0, "more": false });
            };
            let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let limit = (params
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(65536) as usize)
                .clamp(1, 512 * 1024);
            let bytes = if fs::metadata(&full_path)
                .map(|m| m.len() <= 16 * 1024 * 1024)
                .unwrap_or(false)
            {
                fs::read(&full_path).unwrap_or_default()
            } else {
                Vec::new()
            };
            let total = bytes.len();
            let safe_offset = offset.min(total);
            let end = safe_offset.saturating_add(limit).min(total);
            let b64 = encode_base64(&bytes[safe_offset..end]);
            let rev = sha256_bytes_hex(&bytes);
            json!({
                "data": b64,
                "revision": rev,
                "offset": safe_offset,
                "nextOffset": end,
                "size": total,
                "more": end < total
            })
        }
        "changes.list" => {
            let is_git = is_git_repository(root);
            let branch = if is_git {
                current_git_branch(root)
            } else {
                "unversioned".to_string()
            };
            let mut entries: Vec<Value> = Vec::new();
            let mut seen_paths = std::collections::HashSet::new();
            let mut limited = false;
            if is_git {
                let (git_entries, git_limited) = list_git_status_entries(root);
                limited = git_limited;
                for item in git_entries {
                    if let Some(p) = item.get("path").and_then(|v| v.as_str()) {
                        seen_paths.insert(p.to_string());
                    }
                    entries.push(item);
                }
            }
            let staged = lock_unpoison(&engine.vfs()).export_staged();
            for f in staged {
                if seen_paths.insert(f.path.clone()) {
                    let name = Path::new(&f.path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| f.path.clone());
                    entries.push(json!({
                        "path": f.path, "name": name,
                        "status": if f.is_deleted { "deleted" } else { "modified" },
                        "staged": true, "unstaged": false, "untracked": false
                    }));
                }
            }
            if entries.is_empty() && !is_git {
                if let Some(last) = engine.latest_receipt() {
                    for fc in last.file_changes {
                        let name = Path::new(&fc.path)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| fc.path.clone());
                        entries.push(json!({
                            "path": fc.path, "name": name, "status": fc.kind,
                            "staged": false, "unstaged": true, "untracked": fc.kind == "created" || fc.kind == "added",
                            "addedLines": fc.lines_added, "removedLines": fc.lines_removed
                        }));
                    }
                }
            }
            let (has_remote, ahead, behind) = if is_git {
                git_remote_sync_state(root)
            } else {
                (false, 0u64, 0u64)
            };
            json!({
                "entries": entries,
                "isGitRepo": is_git,
                "branch": branch,
                "hasRemote": has_remote,
                "ahead": ahead,
                "behind": behind,
                "limited": limited
            })
        }
        "changes.read" => {
            let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let want_staged = params.get("staged").and_then(|v| v.as_bool());
            let staged = if want_staged != Some(false) {
                lock_unpoison(&engine.vfs()).export_staged()
            } else {
                Vec::new()
            };
            let mut diff = if let Some(f) = staged.into_iter().find(|f| f.path == path) {
                let old = validate_rpc_read_path(root, path)
                    .filter(|p| {
                        fs::metadata(p)
                            .map(|m| m.len() <= 2 * 1024 * 1024)
                            .unwrap_or(false)
                    })
                    .and_then(|p| fs::read_to_string(p).ok())
                    .unwrap_or_default();
                render_unified_diff(path, &old, if f.is_deleted { "" } else { &f.content })
            } else if is_git_repository(root) && is_safe_rel_path(path) {
                read_git_diff_for_path(root, path, want_staged == Some(true))
            } else {
                String::new()
            };
            if diff.is_empty() && want_staged != Some(true) {
                diff = engine
                    .latest_receipt()
                    .and_then(|last| last.snapshot_before.zip(last.snapshot_after))
                    .and_then(|(b, a)| engine.cas_diff_between(&b, Some(a.as_str())).ok())
                    .map(|(_, full_diff)| filter_unified_diff_for_path(&full_diff, path))
                    .unwrap_or_default();
            }
            let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let limit = (params
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(65536) as usize)
                .clamp(1, 512 * 1024);
            crate::ui::worker_handlers::paginate_text_response(&diff, "diff", offset, limit)
        }
        _ => handle_git_remote_rpc(method, params, root)
            .or_else(|| handle_git_rpc(method, params, root))
            .unwrap_or_else(|| json!({ "entries": [] })),
    }
}

fn filter_unified_diff_for_path(full_diff: &str, target_path: &str) -> String {
    if target_path.is_empty() {
        return full_diff.to_string();
    }
    let header_a = format!("--- a/{target_path}");
    let mut collecting = false;
    let mut out = Vec::new();
    for line in full_diff.lines() {
        if line.starts_with("--- a/") {
            collecting = line == header_a;
        }
        if collecting {
            out.push(line);
        }
    }
    if out.is_empty() {
        String::new()
    } else {
        out.join("\n")
    }
}

fn collect_search_files(
    root: &Path,
    dir: &Path,
    query: &str,
    out: &mut Vec<(i32, String, Value)>,
    depth: usize,
) {
    if depth > 6 || out.len() >= 1000 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    let terms: Vec<&str> = query.split_whitespace().collect();
    for entry in entries {
        if out.len() >= 1000 {
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(
            name.as_str(),
            ".git" | "target" | ".build" | "node_modules" | ".DS_Store"
        ) {
            continue;
        }
        let path = entry.path();
        if entry.file_type().map(|t| t.is_symlink()).unwrap_or(true)
            || SafetyPolicy::check_path(&path).is_err()
        {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            collect_search_files(root, &path, query, out, depth + 1);
        } else if let Ok(rel) = path.strip_prefix(root) {
            let rel_str = rel.to_string_lossy().to_string();
            let rel_lower = rel_str.to_lowercase();
            let name_lower = name.to_lowercase();
            let matches = terms.is_empty() || terms.iter().all(|t| rel_lower.contains(t));
            if matches {
                let score = if query.is_empty() {
                    0
                } else if name_lower == query {
                    1000
                } else if name_lower.starts_with(query) {
                    600
                } else if name_lower.contains(query) {
                    300
                } else if rel_lower.contains(query) {
                    150
                } else {
                    50
                };
                out.push((
                    score,
                    rel_str.clone(),
                    json!({ "path": rel_str, "name": name, "type": "file" }),
                ));
            }
        }
    }
}

