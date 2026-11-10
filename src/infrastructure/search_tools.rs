// [LAYER: INFRASTRUCTURE]
use crate::infrastructure::transaction_journal::TransactionJournal;
use crate::plumbing::hash_and_math::{levenshtein, now_ms};
use crate::plumbing::secret_redactor::{CredentialDenyScope, SafetyPolicy};
use crate::plumbing::text_tokens::safe_utf8_prefix;
use regex::{Regex, RegexBuilder};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub fn resolve_workspace_path(cwd: &Path, raw: &str) -> PathBuf {
    let p = Path::new(raw.trim());
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

/// Checks a path and its canonicalized symlink target against `CredentialDenyScope::ModelContext`
/// without leaking symlink destination paths on refusal.
pub fn verify_model_path_access(path: &Path) -> Result<(), String> {
    let canon = path.canonicalize().ok();
    SafetyPolicy::check_credential_path_with_target(path, canon.as_deref(), CredentialDenyScope::ModelContext)
}

/// Writes bytes atomically via a sibling temp file + `fs::rename`, preserving existing file permissions.
pub fn atomic_write_file(target: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dirs: {e}"))?;
    }
    let existing_perms = fs::metadata(target).ok().map(|m| m.permissions());
    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let tmp_path = target.with_file_name(format!(".{file_name}.heav3ns-tmp-{}-{}", std::process::id(), now_ms()));
    fs::write(&tmp_path, bytes).map_err(|e| format!("Failed to write temp file for '{}': {e}", target.display()))?;
    if let Some(perms) = existing_perms {
        let _ = fs::set_permissions(&tmp_path, perms);
    }
    if let Err(e) = fs::rename(&tmp_path, target) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Failed to commit '{}' atomically: {e}", target.display()));
    }
    Ok(())
}

/// Suggests nearby filenames in the same parent directory when a path lookup fails with ENOENT.
pub fn suggest_nearby_files(missing_path: &Path) -> Vec<String> {
    let Some(parent) = missing_path.parent() else { return Vec::new() };
    let target_name = missing_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let Ok(rd) = fs::read_dir(parent) else { return Vec::new() };
    let mut candidates: Vec<(usize, String)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if verify_model_path_access(&e.path()).is_err() {
                return None;
            }
            let dist = levenshtein(&name.to_lowercase(), &target_name);
            if dist <= 4 || name.to_lowercase().contains(&target_name) {
                Some((dist, name))
            } else {
                None
            }
        })
        .collect();
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    candidates.into_iter().take(4).map(|(_, n)| n).collect()
}

pub fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | "__pycache__" | ".venv"
    )
}

pub fn walk_find_files(
    root: &Path,
    current: &Path,
    query: &str,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<String>,
) {
    if depth > max_depth || out.len() >= 500 {
        return;
    }
    let Ok(rd) = fs::read_dir(current) else { return };
    let clean_query = query.trim_matches('*');
    for entry in rd.flatten() {
        if out.len() >= 500 {
            break;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            if !is_ignored_dir(&file_name) && verify_model_path_access(&path).is_ok() {
                walk_find_files(root, &path, query, depth + 1, max_depth, out);
            }
        } else if verify_model_path_access(&path).is_ok() {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned();
            if clean_query.is_empty()
                || file_name.to_lowercase().contains(clean_query)
                || rel.to_lowercase().contains(clean_query)
            {
                out.push(rel);
            }
        }
    }
}

pub fn build_directory_tree(
    dir: &Path,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    lines: &mut Vec<String>,
) {
    if depth >= max_depth || lines.len() >= 400 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut filtered: Vec<_> = rd
        .flatten()
        .filter(|e| !is_ignored_dir(&e.file_name().to_string_lossy()) && verify_model_path_access(&e.path()).is_ok())
        .collect();
    filtered.sort_by(|a, b| match (a.path().is_dir(), b.path().is_dir()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.file_name().cmp(&b.file_name()),
    });
    let count = filtered.len();
    for (idx, entry) in filtered.into_iter().enumerate() {
        let is_last = idx + 1 == count;
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
        let is_dir = entry.path().is_dir();
        let name = entry.file_name().to_string_lossy().into_owned();
        lines.push(format!("{prefix}{connector}{name}{}", if is_dir { "/" } else { "" }));
        if is_dir {
            build_directory_tree(&entry.path(), &child_prefix, depth + 1, max_depth, lines);
        }
    }
}

pub fn collect_source_files(target: &Path, out: &mut Vec<PathBuf>) {
    if out.len() >= 1_500 {
        return;
    }
    if target.is_file() {
        if verify_model_path_access(target).is_ok() {
            out.push(target.to_path_buf());
        }
        return;
    }
    let Ok(rd) = fs::read_dir(target) else { return };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_ignored_dir(&name) || name.starts_with('.') {
            continue;
        }
        let p = entry.path();
        if p.is_dir() {
            collect_source_files(&p, out);
        } else if p.is_file() && verify_model_path_access(&p).is_ok() {
            out.push(p);
        }
    }
}

pub fn batch_replace_regex_files(
    cwd: &Path,
    resolved: &Path,
    find_pat: &str,
    replace_with: &str,
    dry_run: bool,
    journal: &Mutex<TransactionJournal>,
) -> Result<(Value, bool), String> {
    let re = Regex::new(find_pat).map_err(|e| format!("Invalid regex '{find_pat}': {e}"))?;
    let mut files = Vec::new();
    collect_source_files(resolved, &mut files);
    let mut modified = Vec::new();
    let mut total_replacements = 0usize;
    for file in files {
        if verify_model_path_access(&file).is_err() {
            continue;
        }
        if let Ok(content) = fs::read_to_string(&file) {
            let count = re.find_iter(&content).count();
            if count > 0 {
                total_replacements += count;
                let rel = file.strip_prefix(cwd).unwrap_or(&file).to_string_lossy().into_owned();
                modified.push(json!({ "path": rel, "matches": count }));
                if !dry_run {
                    if let Ok(mut j) = journal.lock() {
                        j.record_mutation("batch_replace_regex", &file);
                    }
                    let updated = re.replace_all(&content, replace_with).into_owned();
                    let _ = atomic_write_file(&file, updated.as_bytes());
                }
            }
        }
    }
    Ok((
        json!({
            "success": true,
            "dryRun": dry_run,
            "filesMatched": modified.len(),
            "totalReplacements": total_replacements,
            "files": modified
        }),
        false,
    ))
}

pub fn extract_workspace_symbols(
    target: &Path,
    cwd: &Path,
    query: &str,
    kind_filter: Option<&str>,
    limit: usize,
) -> Vec<Value> {
    static SYM_RE: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let patterns = SYM_RE.get_or_init(|| {
        vec![
            (
                "function",
                Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?(?:async\s+)?(?:fn|function|def|func)\s+([A-Za-z_][A-Za-z0-9_]*)")
                    .unwrap(),
            ),
            (
                "struct",
                Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
            ),
            (
                "enum",
                Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?(?:export\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)")
                    .unwrap(),
            ),
            (
                "trait",
                Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
            ),
            (
                "class",
                Regex::new(r"^\s*(?:export\s+)?(?:abstract\s+)?class\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
            ),
            (
                "interface",
                Regex::new(r"^\s*(?:export\s+)?interface\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap(),
            ),
            (
                "type",
                Regex::new(r"^\s*(?:pub(?:\([^)]+\))?\s+)?(?:export\s+)?type\s+([A-Za-z_][A-Za-z0-9_]*)")
                    .unwrap(),
            ),
        ]
    });

    let mut files = Vec::new();
    collect_source_files(target, &mut files);
    files.sort();
    let mut results = Vec::new();

    for file in files {
        let Ok(content) = fs::read_to_string(&file) else { continue };
        let rel = file.strip_prefix(cwd).unwrap_or(&file).to_string_lossy().into_owned();
        for (line_idx, line) in content.lines().enumerate() {
            for &(kind, ref re) in patterns {
                if let Some(kf) = kind_filter {
                    if kf != kind {
                        continue;
                    }
                }
                if let Some(caps) = re.captures(line) {
                    let sym_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    if query.is_empty() || sym_name.to_lowercase().contains(query) {
                        results.push(json!({
                            "name": sym_name,
                            "kind": kind,
                            "file": rel,
                            "line": line_idx + 1,
                            "signature": safe_utf8_prefix(line.trim(), 240),
                        }));
                        if results.len() >= limit {
                            return results;
                        }
                    }
                    break;
                }
            }
        }
    }
    results
}

pub fn execute_grep_search(
    root: &Path,
    query: &str,
    case_insensitive: bool,
    max_results: usize,
) -> Result<Vec<Value>, String> {
    verify_model_path_access(root)?;
    let re = RegexBuilder::new(query)
        .case_insensitive(case_insensitive)
        .build()
        .or_else(|_| {
            RegexBuilder::new(&regex::escape(query))
                .case_insensitive(case_insensitive)
                .build()
        })
        .map_err(|e| format!("Invalid regex pattern: {e}"))?;

    let mut hits = Vec::new();
    if root.is_file() {
        search_file_lines(root, root.parent().unwrap_or(root), &re, max_results, &mut hits);
    } else {
        walk_grep(root, root, &re, 0, 12, max_results, &mut hits);
    }
    Ok(hits)
}

fn walk_grep(
    root: &Path,
    current: &Path,
    re: &Regex,
    depth: usize,
    max_depth: usize,
    max_results: usize,
    hits: &mut Vec<Value>,
) {
    if depth > max_depth || hits.len() >= max_results {
        return;
    }
    let Ok(rd) = fs::read_dir(current) else { return };
    for entry in rd.flatten() {
        if hits.len() >= max_results {
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            if !is_ignored_dir(&name) && verify_model_path_access(&path).is_ok() {
                walk_grep(root, &path, re, depth + 1, max_depth, max_results, hits);
            }
        } else if path.is_file() && verify_model_path_access(&path).is_ok() {
            search_file_lines(&path, root, re, max_results, hits);
        }
    }
}

fn search_file_lines(file: &Path, root: &Path, re: &Regex, max_results: usize, hits: &mut Vec<Value>) {
    if verify_model_path_access(file).is_err() {
        return;
    }
    let Ok(meta) = fs::metadata(file) else { return };
    if meta.len() > 2 * 1024 * 1024 {
        return;
    }
    let Ok(content) = fs::read_to_string(file) else { return };
    let rel = file.strip_prefix(root).unwrap_or(file).to_string_lossy().into_owned();
    for (idx, line) in content.lines().enumerate() {
        if hits.len() >= max_results {
            break;
        }
        if re.is_match(line) {
            hits.push(json!({
                "file": rel,
                "line": idx + 1,
                "content": safe_utf8_prefix(line, 300),
            }));
        }
    }
}
