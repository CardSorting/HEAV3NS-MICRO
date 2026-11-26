// [LAYER: UI]
use crate::ui::git_rpc::{current_git_branch, ensure_git_identity, init_git_repository, is_git_repository, resolve_gh_binary};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

pub fn handle_git_remote_rpc(method: &str, params: &Value, root: &Path) -> Option<Value> {
    let val = match method {
        "git.githubStatus" => inspect_github_status(root),
        "git.githubSignIn" => launch_github_sign_in(),
        "git.addRemote" => add_git_remote(root, params),
        "git.connectRepo" => connect_existing_repo(root, params),
        "git.publishGitHub" => publish_github_repo(root, params),
        _ => return None,
    };
    Some(val)
}

/// Sanitizes a folder or user-entered name into a valid GitHub repository slug.
pub fn sanitize_repo_name(raw: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches(|c| c == '-' || c == '.').to_string();
    if trimmed.is_empty() {
        "heav3ns-project".to_string()
    } else {
        trimmed
    }
}

/// Normalizes friendly user input (`owner/repo`, `github.com/owner/repo`, HTTPS, or SSH)
/// into a valid Git remote URL. Rejects control chars or option-injection prefixes.
pub fn normalize_remote_input(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() || s.contains('\0') || s.contains('\n') || s.contains('\r') || s.starts_with('-') {
        return None;
    }
    if s.starts_with("https://") || s.starts_with("http://") || s.starts_with("git@") || s.starts_with("ssh://") {
        return Some(s.to_string());
    }
    let stripped = s
        .strip_prefix("github.com/")
        .or_else(|| s.strip_prefix("www.github.com/"))
        .unwrap_or(s)
        .trim_matches('/');
    let parts: Vec<&str> = stripped.split('/').collect();
    if parts.len() == 2
        && !parts[0].is_empty()
        && !parts[1].is_empty()
        && parts.iter().all(|p| {
            p.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        })
    {
        let repo = parts[1].strip_suffix(".git").unwrap_or(parts[1]);
        return Some(format!("https://github.com/{}/{}.git", parts[0], repo));
    }
    None
}

/// Converts a Git remote URL (SSH or HTTPS) into a browser-openable HTTPS web URL.
pub fn remote_url_to_web_url(url: &str) -> Option<String> {
    let s = url.trim();
    if let Some(rest) = s.strip_prefix("git@github.com:") {
        let clean = rest.strip_suffix(".git").unwrap_or(rest);
        return Some(format!("https://github.com/{clean}"));
    }
    if s.starts_with("https://") || s.starts_with("http://") {
        let clean = s.strip_suffix(".git").unwrap_or(s);
        return Some(clean.to_string());
    }
    None
}

/// Extracts a clean display label such as `owner/repo` from a Git remote URL.
pub fn remote_display_name(url: &str) -> String {
    let web = remote_url_to_web_url(url).unwrap_or_else(|| url.trim().to_string());
    if let Some(rest) = web
        .strip_prefix("https://github.com/")
        .or_else(|| web.strip_prefix("http://github.com/"))
    {
        return rest.trim_matches('/').to_string();
    }
    web
}

fn origin_remote_url(root: &Path) -> String {
    if !is_git_repository(root) {
        return String::new();
    }
    Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn inspect_github_status(root: &Path) -> Value {
    let remote_url = origin_remote_url(root);
    let has_remote = !remote_url.is_empty();
    let web_url = remote_url_to_web_url(&remote_url).unwrap_or_default();
    let remote_name = if has_remote { remote_display_name(&remote_url) } else { String::new() };
    let default_name = root
        .file_name()
        .map(|n| sanitize_repo_name(&n.to_string_lossy()))
        .unwrap_or_else(|| "heav3ns-project".to_string());

    let Some(gh) = resolve_gh_binary() else {
        return json!({
            "ok": true,
            "ghInstalled": false,
            "ghAuthenticated": false,
            "ghUser": "",
            "hasRemote": has_remote,
            "remoteUrl": remote_url,
            "webUrl": web_url,
            "remoteName": remote_name,
            "suggestedName": default_name,
            "repos": []
        });
    };

    let gh_user = Command::new(&gh)
        .args(["api", "user", "--jq", ".login"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let gh_authenticated = !gh_user.is_empty();

    let mut repos: Vec<Value> = Vec::new();
    if gh_authenticated && !has_remote {
        if let Ok(out) = Command::new(&gh)
            .args(["repo", "list", "--limit", "25", "--json", "nameWithOwner,url,isPrivate"])
            .output()
        {
            if out.status.success() {
                if let Ok(Value::Array(items)) = serde_json::from_slice::<Value>(&out.stdout) {
                    repos = items;
                }
            }
        }
    }

    json!({
        "ok": true,
        "ghInstalled": true,
        "ghAuthenticated": gh_authenticated,
        "ghUser": gh_user,
        "hasRemote": has_remote,
        "remoteUrl": remote_url,
        "webUrl": web_url,
        "remoteName": remote_name,
        "suggestedName": default_name,
        "repos": repos
    })
}

fn launch_github_sign_in() -> Value {
    if let Some(gh) = resolve_gh_binary() {
        let script_path = std::env::temp_dir().join("heav3ns-github-signin.command");
        let script = format!(
            "#!/bin/sh\necho \"Signing in to GitHub for HEAV3NS CODE...\"\n\"{gh}\" auth login --web -h github.com -p https && \"{gh}\" auth setup-git\necho \"\"\necho \"Done! You can close this window and click Check Sign-In in HEAV3NS CODE.\"\n"
        );
        if std::fs::write(&script_path, script).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755));
            }
            return json!({
                "ok": true,
                "mode": "gh-web",
                "openPath": script_path.to_string_lossy().to_string(),
                "openUrl": "https://github.com/login/device",
                "summary": "Opened GitHub sign-in. Complete authorization in your browser, then click Check Sign-In."
            });
        }
    }
    json!({
        "ok": true,
        "mode": "browser",
        "openUrl": "https://github.com/login",
        "summary": "Opened GitHub in your browser. After signing in, click Check Sign-In or paste your repo link."
    })
}

pub fn add_git_remote(root: &Path, params: &Value) -> Value {
    if !is_git_repository(root) {
        let init = init_git_repository(root);
        if init["ok"] != true {
            return json!({ "ok": false, "error": "Could not prepare this folder for Git." });
        }
    }
    let raw = params.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let Some(url) = normalize_remote_input(raw) else {
        return json!({
            "ok": false,
            "error": "Enter a GitHub repository like owner/project or a valid Git link."
        });
    };
    let has_origin = !origin_remote_url(root).is_empty();
    let args = if has_origin {
        vec!["remote", "set-url", "origin", &url]
    } else {
        vec!["remote", "add", "origin", &url]
    };
    match Command::new("git").args(&args).current_dir(root).output() {
        Ok(o) if o.status.success() => json!({
            "ok": true,
            "url": url,
            "webUrl": remote_url_to_web_url(&url).unwrap_or_default(),
            "remoteName": remote_display_name(&url),
            "summary": format!("Connected to {}.", remote_display_name(&url))
        }),
        Ok(o) => json!({
            "ok": false,
            "error": String::from_utf8_lossy(&o.stderr).trim().to_string()
        }),
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

fn ensure_initial_commit(root: &Path) {
    let has_commit = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if has_commit {
        return;
    }
    ensure_git_identity(root);
    let _ = Command::new("git").args(["add", "-A"]).current_dir(root).output();
    let committed = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !committed {
        let _ = Command::new("git")
            .args(["commit", "--allow-empty", "-m", "Initial commit"])
            .current_dir(root)
            .output();
    }
}

fn connect_existing_repo(root: &Path, params: &Value) -> Value {
    let added = add_git_remote(root, params);
    if added["ok"] != true {
        return added;
    }
    if let Some(gh) = resolve_gh_binary() {
        let _ = Command::new(&gh).args(["auth", "setup-git"]).output();
    }
    ensure_initial_commit(root);
    let branch = current_git_branch(root);
    let push_ok = Command::new("git")
        .args(["push", "-u", "origin", &branch])
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let display = added["remoteName"].as_str().unwrap_or("GitHub");
    json!({
        "ok": true,
        "url": added["url"],
        "webUrl": added["webUrl"],
        "remoteName": display,
        "pushed": push_ok,
        "summary": if push_ok {
            format!("Connected and synced with {display}.")
        } else {
            format!("Connected to {display}. Click Pull or Sync to merge existing remote commits.")
        }
    })
}

fn publish_github_repo(root: &Path, params: &Value) -> Value {
    if !is_git_repository(root) {
        let init = init_git_repository(root);
        if init["ok"] != true {
            return json!({ "ok": false, "error": "Could not prepare this folder for Git." });
        }
    }
    ensure_initial_commit(root);

    let Some(gh) = resolve_gh_binary() else {
        return json!({
            "ok": false,
            "needsSignIn": true,
            "error": "Sign in to GitHub first so HEAV3NS CODE can create repositories for you."
        });
    };

    let _ = Command::new(&gh).args(["auth", "setup-git"]).output();
    let visibility = if params.get("visibility").and_then(|v| v.as_str()) == Some("public") {
        "--public"
    } else {
        "--private"
    };
    let default_name = root
        .file_name()
        .map(|n| sanitize_repo_name(&n.to_string_lossy()))
        .unwrap_or_else(|| "heav3ns-project".to_string());
    let base_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .map(sanitize_repo_name)
        .filter(|s| !s.is_empty())
        .unwrap_or(default_name);

    // Clear any broken/stale origin before gh repo create --remote=origin
    if !origin_remote_url(root).is_empty() {
        let _ = Command::new("git").args(["remote", "remove", "origin"]).current_dir(root).output();
    }

    let mut last_err = String::new();
    for attempt in 0..5u32 {
        let candidate = if attempt == 0 {
            base_name.clone()
        } else {
            format!("{base_name}-{}", attempt + 1)
        };
        let out = Command::new(&gh)
            .args([
                "repo",
                "create",
                &candidate,
                visibility,
                "--source=.",
                "--remote=origin",
                "--push",
            ])
            .current_dir(root)
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let raw_out = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let url = raw_out
                    .lines()
                    .find(|l| l.starts_with("http"))
                    .map(str::trim)
                    .unwrap_or(&raw_out)
                    .to_string();
                let origin_now = origin_remote_url(root);
                let effective = if !url.is_empty() { url } else { origin_now };
                let web = remote_url_to_web_url(&effective).unwrap_or_else(|| effective.clone());
                let display = remote_display_name(&effective);
                return json!({
                    "ok": true,
                    "url": effective,
                    "webUrl": web,
                    "remoteName": display,
                    "repoName": candidate,
                    "summary": format!("Created and published {display} on GitHub.")
                });
            }
            Ok(o) => {
                let raw_err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                let lower = raw_err.to_lowercase();
                if lower.contains("already exists") {
                    last_err = raw_err;
                    continue;
                }
                let friendly = if lower.contains("not logged into") || lower.contains("authentication") {
                    "Sign in to GitHub first using the Sign in to GitHub button.".to_string()
                } else if raw_err.is_empty() {
                    "Could not create your GitHub repository right now. Try again in a moment.".to_string()
                } else {
                    raw_err
                };
                return json!({ "ok": false, "error": friendly });
            }
            Err(_) => {
                return json!({
                    "ok": false,
                    "error": "Could not reach GitHub right now. Check your internet connection and try again."
                });
            }
        }
    }

    json!({
        "ok": false,
        "error": if last_err.is_empty() {
            format!("Could not find an available name near \"{base_name}\". Enter a different repository name.")
        } else {
            format!("A repository named \"{base_name}\" already exists on GitHub. Try a different name.")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_remote_input_accepts_friendly_formats() {
        assert_eq!(
            normalize_remote_input("CardSorting/dragon").as_deref(),
            Some("https://github.com/CardSorting/dragon.git")
        );
        assert_eq!(
            normalize_remote_input("github.com/CardSorting/dragon").as_deref(),
            Some("https://github.com/CardSorting/dragon.git")
        );
        assert_eq!(
            normalize_remote_input("https://github.com/CardSorting/dragon.git").as_deref(),
            Some("https://github.com/CardSorting/dragon.git")
        );
        assert_eq!(
            normalize_remote_input("git@github.com:CardSorting/dragon.git").as_deref(),
            Some("git@github.com:CardSorting/dragon.git")
        );
        assert!(normalize_remote_input("--upload-pack=evil").is_none());
        assert!(normalize_remote_input("   ").is_none());
    }

    #[test]
    fn test_web_url_and_display_name_helpers() {
        assert_eq!(
            remote_url_to_web_url("git@github.com:CardSorting/dragon.git").as_deref(),
            Some("https://github.com/CardSorting/dragon")
        );
        assert_eq!(
            remote_url_to_web_url("https://github.com/CardSorting/dragon.git").as_deref(),
            Some("https://github.com/CardSorting/dragon")
        );
        assert_eq!(remote_display_name("git@github.com:CardSorting/dragon.git"), "CardSorting/dragon");
        assert_eq!(sanitize_repo_name("My Cool App! "), "My-Cool-App");
    }

    #[test]
    fn test_add_git_remote_sets_and_updates_origin() {
        let dir = std::env::temp_dir().join(format!("heav3ns_remote_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let res = add_git_remote(&dir, &json!({ "url": "CardSorting/dragon" }));
        assert_eq!(res["ok"], true);
        assert_eq!(res["remoteName"], "CardSorting/dragon");
        assert_eq!(res["webUrl"], "https://github.com/CardSorting/dragon");
        let res2 = add_git_remote(&dir, &json!({ "url": "https://github.com/CardSorting/dragon-v2.git" }));
        assert_eq!(res2["ok"], true);
        assert_eq!(res2["remoteName"], "CardSorting/dragon-v2");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
