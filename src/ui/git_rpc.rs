// [LAYER: UI]
use crate::plumbing::text_tokens::render_unified_diff;
use crate::ui::rpc_dispatch::validate_rpc_read_path;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn handle_git_rpc(method: &str, params: &Value, root: &Path) -> Option<Value> {
    let val = match method {
        "git.init" => init_git_repository(root),
        "git.stage" => stage_git_paths(root, params),
        "git.unstage" => unstage_git_paths(root, params),
        "git.commit" => commit_git_changes(root, params),
        "git.discard" => discard_git_path(root, params),
        "git.generateCommitMessage" => generate_commit_message(root),
        "git.push" => push_git_changes(root),
        "git.pull" => pull_git_changes(root),
        "git.fetch" => fetch_git_changes(root),
        "git.sync" => sync_git_changes(root),
        "git.log" => list_git_commits(root),
        "git.show" => show_git_commit(root, params),
        "git.branchList" => list_git_branches(root),
        "git.checkout" => checkout_git_target(root, params),
        "git.stash" => stash_git_changes(root, false),
        "git.stashPop" => stash_git_changes(root, true),
        _ => return None,
    };
    Some(val)
}

pub fn resolve_gh_binary() -> Option<String> {
    let candidates = ["/opt/homebrew/bin/gh", "/usr/local/bin/gh", "/usr/bin/gh", "gh"];
    for candidate in candidates {
        if candidate.starts_with('/') && !Path::new(candidate).is_file() {
            continue;
        }
        if Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(candidate.to_string());
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let local_bin = format!("{home}/.local/bin/gh");
        if Path::new(&local_bin).is_file() {
            return Some(local_bin);
        }
    }
    None
}

pub fn ensure_git_identity(root: &Path) {
    let has_name = Command::new("git")
        .args(["config", "user.name"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false);
    let has_email = Command::new("git")
        .args(["config", "user.email"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false);
    if has_name && has_email {
        return;
    }
    let mut name = String::new();
    let mut email = String::new();
    if let Some(gh) = resolve_gh_binary() {
        if let Ok(out) = Command::new(&gh)
            .args(["api", "user", "--jq", r#"[(.name // .login // ""), (.email // ""), (.login // "")] | @tsv"#])
            .output()
        {
            if out.status.success() {
                let line = String::from_utf8_lossy(&out.stdout);
                let cols: Vec<&str> = line.trim().split('\t').collect();
                if let Some(n) = cols.first().filter(|s| !s.is_empty()) {
                    name = (*n).to_string();
                }
                if let Some(e) = cols.get(1).filter(|s| !s.is_empty()) {
                    email = (*e).to_string();
                } else if let Some(login) = cols.get(2).filter(|s| !s.is_empty()) {
                    email = format!("{login}@users.noreply.github.com");
                }
            }
        }
    }
    if name.is_empty() {
        name = std::env::var("USER").unwrap_or_else(|_| "HEAV3NS User".to_string());
    }
    if email.is_empty() {
        let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        email = format!("{user}@users.noreply.github.com");
    }
    if !has_name {
        let _ = Command::new("git").args(["config", "user.name", &name]).current_dir(root).output();
    }
    if !has_email {
        let _ = Command::new("git").args(["config", "user.email", &email]).current_dir(root).output();
    }
}

pub fn is_safe_rel_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\0')
        && !Path::new(path)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
}

pub fn is_git_repository(root: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

pub fn current_git_branch(root: &Path) -> String {
    if let Ok(out) = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(root)
        .output()
    {
        if out.status.success() {
            let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !b.is_empty() {
                return b;
            }
        }
    }
    if let Ok(out) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
    {
        if out.status.success() {
            let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !b.is_empty() {
                return b;
            }
        }
    }
    "main".to_string()
}

pub fn git_remote_sync_state(root: &Path) -> (bool, u64, u64) {
    let has_remote = Command::new("git")
        .args(["remote"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false);
    if !has_remote {
        return (false, 0, 0);
    }
    let counts = Command::new("git")
        .args(["rev-list", "--left-right", "--count", "@{u}...HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let mut parts = counts.split_whitespace();
    let behind = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let ahead = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    (true, ahead, behind)
}

pub fn list_git_status_entries(root: &Path) -> (Vec<Value>, bool) {
    let Ok(out) = Command::new("git")
        .args(["status", "--porcelain=v1", "-uall"])
        .current_dir(root)
        .output()
    else {
        return (Vec::new(), false);
    };
    if !out.status.success() {
        return (Vec::new(), false);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut entries = Vec::new();
    let mut limited = false;
    for raw_line in text.lines() {
        if raw_line.len() < 4 {
            continue;
        }
        if entries.len() >= 500 {
            limited = true;
            break;
        }
        let bytes = raw_line.as_bytes();
        let x = bytes[0] as char;
        let y = bytes[1] as char;
        let raw_path = raw_line[3..].trim();
        let path = raw_path
            .rsplit_once(" -> ")
            .map(|(_, dst)| dst)
            .unwrap_or(raw_path)
            .trim_matches('"')
            .to_string();
        let name = Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        let untracked = x == '?' && y == '?';
        let staged = !untracked && x != ' ';
        let unstaged = untracked || y != ' ';
        let status_code = if untracked {
            "untracked"
        } else if x == 'D' || y == 'D' {
            "deleted"
        } else if x == 'A' || y == 'A' {
            "added"
        } else if x == 'R' || y == 'R' {
            "renamed"
        } else {
            "modified"
        };
        entries.push(json!({
            "path": path,
            "name": name,
            "status": status_code,
            "staged": staged,
            "unstaged": unstaged,
            "untracked": untracked
        }));
    }
    (entries, limited)
}

pub fn read_git_diff_for_path(root: &Path, path: &str, staged: bool) -> String {
    let args: Vec<&str> = if staged {
        vec!["diff", "--cached", "--", path]
    } else {
        vec!["diff", "--", path]
    };
    if let Ok(out) = Command::new("git").args(&args).current_dir(root).output() {
        if out.status.success() {
            let diff = String::from_utf8_lossy(&out.stdout).to_string();
            if !diff.trim().is_empty() {
                return diff;
            }
        }
    }
    if !staged {
        if let Some(full) = validate_rpc_read_path(root, path) {
            if fs::metadata(&full).map(|m| m.len() <= 512 * 1024).unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&full) {
                    return render_unified_diff(path, "", &content);
                }
            }
        }
    }
    String::new()
}

pub fn init_git_repository(root: &Path) -> Value {
    if is_git_repository(root) {
        return json!({ "ok": true, "isGitRepo": true, "branch": current_git_branch(root) });
    }
    let ok = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        || Command::new("git")
            .args(["init"])
            .current_dir(root)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    json!({
        "ok": ok,
        "isGitRepo": ok && is_git_repository(root),
        "branch": if ok { current_git_branch(root) } else { "unversioned".to_string() }
    })
}

pub fn stage_git_paths(root: &Path, params: &Value) -> Value {
    if !is_git_repository(root) {
        return json!({ "ok": false, "error": "Not a Git repository." });
    }
    let all = params.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
    let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let out = if all {
        Command::new("git").args(["add", "-A"]).current_dir(root).output()
    } else if is_safe_rel_path(path) {
        Command::new("git").args(["add", "--", path]).current_dir(root).output()
    } else {
        return json!({ "ok": false, "error": "Invalid relative path." });
    };
    git_status_json(out)
}

fn unstage_git_paths(root: &Path, params: &Value) -> Value {
    if !is_git_repository(root) {
        return json!({ "ok": false, "error": "Not a Git repository." });
    }
    let all = params.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
    let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let out = if all {
        Command::new("git").args(["reset", "HEAD"]).current_dir(root).output()
    } else if is_safe_rel_path(path) {
        Command::new("git").args(["reset", "HEAD", "--", path]).current_dir(root).output()
    } else {
        return json!({ "ok": false, "error": "Invalid relative path." });
    };
    git_status_json(out)
}

pub fn commit_git_changes(root: &Path, params: &Value) -> Value {
    if !is_git_repository(root) {
        return json!({ "ok": false, "error": "Not a Git repository." });
    }
    let msg = params
        .get("message")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    if msg.is_empty() {
        return json!({ "ok": false, "error": "Commit message cannot be empty." });
    }
    if params.get("autoStage").and_then(|v| v.as_bool()).unwrap_or(false) {
        let has_staged = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(root)
            .status()
            .map(|s| !s.success())
            .unwrap_or(false);
        if !has_staged {
            let _ = Command::new("git").args(["add", "-A"]).current_dir(root).output();
        }
    }
    ensure_git_identity(root);
    let amend = params.get("amend").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut args = vec!["commit"];
    if amend {
        args.push("--amend");
    }
    args.push("-m");
    args.push(msg);
    let commit_res = match Command::new("git").args(&args).current_dir(root).output() {
        Ok(o) if o.status.success() => {
            json!({ "ok": true, "summary": String::from_utf8_lossy(&o.stdout).trim().to_string() })
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
            let out_s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            return json!({ "ok": false, "error": if err.is_empty() { out_s } else { err } });
        }
        Err(e) => return json!({ "ok": false, "error": e.to_string() }),
    };
    if params.get("push").and_then(|v| v.as_bool()).unwrap_or(false) {
        let push_res = push_git_changes(root);
        if push_res["ok"] != true {
            return json!({
                "ok": false,
                "committed": true,
                "error": push_res["error"].as_str().unwrap_or("Committed locally, but git push failed.")
            });
        }
        return json!({
            "ok": true,
            "pushed": true,
            "summary": push_res["summary"].as_str().unwrap_or("Committed and pushed to remote.")
        });
    }
    commit_res
}

fn discard_git_path(root: &Path, params: &Value) -> Value {
    if !is_git_repository(root) {
        return json!({ "ok": false, "error": "Not a Git repository." });
    }
    let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if !is_safe_rel_path(path) {
        return json!({ "ok": false, "error": "Invalid relative path." });
    }
    git_status_json(Command::new("git").args(["checkout", "--", path]).current_dir(root).output())
}

pub fn generate_commit_message(root: &Path) -> Value {
    if !is_git_repository(root) {
        return json!({ "ok": false, "error": "Not a Git repository." });
    }
    let (entries, _) = list_git_status_entries(root);
    if entries.is_empty() {
        return json!({ "ok": false, "error": "No changes to summarize." });
    }
    let staged_only: Vec<&Value> = entries
        .iter()
        .filter(|e| e.get("staged").and_then(|v| v.as_bool()).unwrap_or(false))
        .collect();
    let target = if staged_only.is_empty() {
        entries.iter().collect::<Vec<&Value>>()
    } else {
        staged_only
    };
    let all_untracked_or_added = target.iter().all(|e| {
        let s = e.get("status").and_then(|v| v.as_str()).unwrap_or("");
        s == "untracked" || s == "added"
    });
    let all_docs = target.iter().all(|e| {
        let p = e.get("path").and_then(|v| v.as_str()).unwrap_or("");
        p.ends_with(".md") || p.ends_with(".txt") || p.starts_with("docs/")
    });
    let verb_prefix = if all_docs { "docs" } else if all_untracked_or_added { "feat" } else { "chore" };
    let names: Vec<&str> = target.iter().filter_map(|e| e.get("name").and_then(|v| v.as_str())).collect();
    let first = names.first().copied().unwrap_or("workspace");
    let message = if names.len() == 1 {
        let st = target[0].get("status").and_then(|v| v.as_str()).unwrap_or("update");
        let action = match st { "added" | "untracked" => "add", "deleted" => "remove", _ => "update" };
        format!("{verb_prefix}: {action} {first}")
    } else {
        format!("{verb_prefix}: update {first} and {} more file(s)", names.len() - 1)
    };
    json!({ "ok": true, "message": message })
}

fn push_git_changes(root: &Path) -> Value {
    if !is_git_repository(root) {
        return json!({ "ok": false, "error": "Not a Git repository." });
    }
    let (has_remote, _, _) = git_remote_sync_state(root);
    if !has_remote {
        return json!({ "ok": false, "error": "No Git remote configured. Add an origin remote first." });
    }
    match Command::new("git").args(["push"]).current_dir(root).output() {
        Ok(o) if o.status.success() => {
            let out_s = String::from_utf8_lossy(&o.stderr).trim().to_string();
            json!({ "ok": true, "summary": if out_s.is_empty() { "Pushed to remote.".to_string() } else { out_s } })
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if err.contains("has no upstream branch") || err.contains("set-upstream") {
                let branch = current_git_branch(root);
                match Command::new("git").args(["push", "-u", "origin", &branch]).current_dir(root).output() {
                    Ok(u) if u.status.success() => json!({ "ok": true, "summary": format!("Pushed and set upstream origin/{branch}.") }),
                    Ok(u) => json!({ "ok": false, "error": String::from_utf8_lossy(&u.stderr).trim().to_string() }),
                    Err(e) => json!({ "ok": false, "error": e.to_string() }),
                }
            } else {
                json!({ "ok": false, "error": err })
            }
        }
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

fn pull_git_changes(root: &Path) -> Value {
    if !is_git_repository(root) {
        return json!({ "ok": false, "error": "Not a Git repository." });
    }
    git_status_json(Command::new("git").args(["pull", "--rebase"]).current_dir(root).output())
}

fn fetch_git_changes(root: &Path) -> Value {
    if !is_git_repository(root) {
        return json!({ "ok": false, "error": "Not a Git repository." });
    }
    git_status_json(Command::new("git").args(["fetch", "--all", "--prune"]).current_dir(root).output())
}

fn sync_git_changes(root: &Path) -> Value {
    let pull = pull_git_changes(root);
    if pull["ok"] != true {
        return pull;
    }
    push_git_changes(root)
}

fn list_git_branches(root: &Path) -> Value {
    if !is_git_repository(root) {
        return json!({ "branches": [], "current": "unversioned" });
    }
    let current = current_git_branch(root);
    let branches: Vec<String> = Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    json!({ "branches": branches, "current": current })
}

fn checkout_git_target(root: &Path, params: &Value) -> Value {
    if !is_git_repository(root) {
        return json!({ "ok": false, "error": "Not a Git repository." });
    }
    let target = params.get("target").and_then(|v| v.as_str()).map(str::trim).unwrap_or("");
    if target.is_empty() || target.contains('\0') || target.starts_with('-') {
        return json!({ "ok": false, "error": "Invalid branch or commit target." });
    }
    let create = params.get("create").and_then(|v| v.as_bool()).unwrap_or(false);
    let out = if create {
        Command::new("git").args(["checkout", "-b", target]).current_dir(root).output()
    } else {
        Command::new("git").args(["checkout", target]).current_dir(root).output()
    };
    git_status_json(out)
}

fn stash_git_changes(root: &Path, pop: bool) -> Value {
    if !is_git_repository(root) {
        return json!({ "ok": false, "error": "Not a Git repository." });
    }
    let args = if pop { vec!["stash", "pop"] } else { vec!["stash", "push", "-u"] };
    git_status_json(Command::new("git").args(&args).current_dir(root).output())
}

fn list_git_commits(root: &Path) -> Value {
    if !is_git_repository(root) {
        return json!({ "commits": [], "isGitRepo": false });
    }
    let Ok(out) = Command::new("git")
        .args(["log", "-n", "40", "--format=%H%x1f%h%x1f%s%x1f%an%x1f%cr%x1f%D"])
        .current_dir(root)
        .output()
    else {
        return json!({ "commits": [], "isGitRepo": true });
    };
    if !out.status.success() {
        return json!({ "commits": [], "isGitRepo": true });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let commits: Vec<Value> = text
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let parts: Vec<&str> = line.split('\x1f').collect();
            if parts.len() < 5 {
                return None;
            }
            let refs: Vec<String> = parts
                .get(5)
                .copied()
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.strip_prefix("HEAD -> ").unwrap_or(s).to_string())
                .collect();
            Some(json!({
                "hash": parts[0],
                "shortHash": parts[1],
                "subject": parts[2],
                "author": parts[3],
                "relativeTime": parts[4],
                "refs": refs,
                "isHead": idx == 0
            }))
        })
        .collect();
    json!({ "commits": commits, "isGitRepo": true })
}

fn show_git_commit(root: &Path, params: &Value) -> Value {
    let hash = params.get("hash").and_then(|v| v.as_str()).map(str::trim).unwrap_or("");
    if !is_git_repository(root)
        || hash.is_empty()
        || !hash.chars().all(|c| c.is_ascii_hexdigit())
    {
        return json!({ "text": "", "revision": "git-show", "offset": 0, "nextOffset": 0, "size": 0, "more": false });
    }
    let diff = Command::new("git")
        .args(["show", "--stat", "--patch", hash])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let limit = (params.get("limit").and_then(|v| v.as_u64()).unwrap_or(65536) as usize).clamp(1, 512 * 1024);
    crate::ui::worker_handlers::paginate_text_response(&diff, &format!("commit-{hash}"), offset, limit)
}

fn git_status_json(out: std::io::Result<std::process::Output>) -> Value {
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            json!({ "ok": true, "summary": s })
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
            let out_s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            json!({ "ok": false, "error": if err.is_empty() { out_s } else { err } })
        }
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_git_source_control_workflow() {
        let dir = std::env::temp_dir().join(format!("heav3ns_git_full_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(!is_git_repository(&dir));

        let init = init_git_repository(&dir);
        assert_eq!(init["ok"], true);
        assert!(is_git_repository(&dir));

        Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(&dir).output().unwrap();
        Command::new("git").args(["config", "user.name", "Test"]).current_dir(&dir).output().unwrap();

        fs::write(dir.join("game.js"), "console.log('hi');\n").unwrap();
        let (entries, _) = list_git_status_entries(&dir);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["untracked"], true);

        let gen = generate_commit_message(&dir);
        assert_eq!(gen["ok"], true);
        assert!(gen["message"].as_str().unwrap_or("").contains("game.js"));

        assert_eq!(commit_git_changes(&dir, &json!({ "message": "feat: add game", "autoStage": true }))["ok"], true);

        let log = list_git_commits(&dir);
        let commits = log["commits"].as_array().unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0]["subject"], "feat: add game");
        assert_eq!(commits[0]["isHead"], true);

        let hash = commits[0]["hash"].as_str().unwrap();
        let shown = show_git_commit(&dir, &json!({ "hash": hash }));
        assert!(shown["text"].as_str().unwrap_or("").contains("feat: add game"));

        let branches = list_git_branches(&dir);
        assert!(!branches["branches"].as_array().unwrap().is_empty());

        let _ = fs::remove_dir_all(&dir);
    }
}
