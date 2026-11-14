// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::IProjectRunner;
use crate::plumbing::hash_and_math::{fnv1a_32, lock_unpoison, now_ms};
use crate::plumbing::secret_redactor::SafetyPolicy;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const MAX_RUN_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
struct CustomRunTarget {
    command: String,
    cwd: String,
    preview: Option<String>,
}

#[derive(Debug)]
struct ProjectRunState {
    state: String,
    output: String,
    revision: u64,
    launch_id: String,
    child_pid: Option<u32>,
    active_target: Option<Value>,
    custom_target: Option<CustomRunTarget>,
}

impl Default for ProjectRunState {
    fn default() -> Self {
        Self {
            state: "idle".to_string(),
            output: String::new(),
            revision: 1,
            launch_id: String::new(),
            child_pid: None,
            active_target: None,
            custom_target: None,
        }
    }
}

/// Real project target discovery, bounded stdout/stderr streaming, and process-group lifecycle manager.
#[derive(Debug, Default)]
pub struct ProjectRunner {
    inner: Arc<Mutex<ProjectRunState>>,
}

impl ProjectRunner {
    pub fn create_runner() -> Self {
        Self::default()
    }

    fn build_targets(&self, workspace_root: &Path) -> Vec<Value> {
        let mut targets = Vec::new();
        if let Some(custom) = &lock_unpoison(&self.inner).custom_target {
            targets.push(make_target(
                "custom",
                "Custom Run Command",
                &custom.command,
                &custom.cwd,
                custom.preview.as_deref(),
            ));
        }
        if workspace_root.join("Cargo.toml").exists() {
            targets.push(make_target("cargo-test", "Rust Test Suite (cargo test)", "cargo test", ".", None));
            targets.push(make_target("flight-suite", "Agent Flight Verification (--flight)", "cargo run -- --flight", ".", None));
        }
        if workspace_root.join("native/Package.swift").exists() {
            targets.push(make_target("swift-native-test", "Native macOS AppKit Tests (swift test)", "swift test", "native", None));
        } else if workspace_root.join("Package.swift").exists() {
            targets.push(make_target("swift-test", "Swift Package Tests (swift test)", "swift test", ".", None));
        }
        if workspace_root.join("package.json").exists() {
            targets.push(make_target("npm-test", "Node Test Suite (npm test)", "npm test", ".", None));
        }
        if targets.is_empty() {
            targets.push(make_target("workspace-status", "Workspace Status Check", "git status --short", ".", None));
        }
        targets
    }

    fn status_json(st: &ProjectRunState) -> Value {
        let mut obj = json!({
            "state": st.state,
            "output": st.output,
            "revision": st.revision,
            "launch": st.launch_id,
            "healing": { "enabled": true }
        });
        if let Some(ref target) = st.active_target {
            obj["target"] = target.clone();
            if let Some(preview) = target.get("preview").and_then(Value::as_str) {
                obj["preview"] = json!(preview);
            }
        }
        obj
    }
}

impl IProjectRunner for ProjectRunner {
    fn discover(&self, workspace_root: &Path) -> Value {
        json!({ "targets": self.build_targets(workspace_root) })
    }

    fn start(&self, workspace_root: &Path, target_id: &str, expected_revision: Option<&str>) -> Result<Value, String> {
        let targets = self.build_targets(workspace_root);
        let target = targets
            .into_iter()
            .find(|t| t.get("id").and_then(Value::as_str) == Some(target_id))
            .ok_or_else(|| format!("PROJECT_CHANGED: Run target '{target_id}' is no longer available."))?;

        if let Some(exp) = expected_revision.filter(|s| !s.is_empty()) {
            let actual = target.get("revision").and_then(Value::as_str).unwrap_or("");
            if actual != exp {
                return Err("PROJECT_CHANGED: Target configuration changed; review and retry.".to_string());
            }
        }

        let cmd_str = target
            .get("command")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| "Run target has an empty command.".to_string())?
            .to_string();
        let rel_cwd = target.get("cwd").and_then(Value::as_str).unwrap_or(".");
        let resolved_cwd = resolve_safe_cwd(workspace_root, rel_cwd)?;

        {
            let st = lock_unpoison(&self.inner);
            if matches!(st.state.as_str(), "running" | "stopping") {
                return Err("Project is already running. Stop it before starting another run.".to_string());
            }
        }

        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg(&cmd_str)
            .current_dir(&resolved_cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for key in SafetyPolicy::sensitive_env_vars() {
            cmd.env_remove(key);
        }
        #[cfg(unix)]
        unsafe {
            cmd.pre_exec(|| {
                libc_setpgid();
                Ok(())
            });
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to start '{cmd_str}': {e}"))?;
        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let snap = {
            let mut st = lock_unpoison(&self.inner);
            st.state = "running".to_string();
            st.output.clear();
            st.revision = st.revision.saturating_add(1);
            st.launch_id = format!("launch-{}", now_ms());
            st.child_pid = pid;
            st.active_target = Some(target);
            Self::status_json(&st)
        };

        let inner_out = Arc::clone(&self.inner);
        let out_handle = stdout.map(|s| {
            tokio::spawn(async move {
                drain_stream(s, inner_out).await;
            })
        });
        let inner_err = Arc::clone(&self.inner);
        let err_handle = stderr.map(|s| {
            tokio::spawn(async move {
                drain_stream(s, inner_err).await;
            })
        });

        let inner_wait = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let status = child.wait().await;
            if let Some(h) = out_handle {
                let _ = h.await;
            }
            if let Some(h) = err_handle {
                let _ = h.await;
            }
            let mut st = lock_unpoison(&inner_wait);
            let was_stopping = st.state == "stopping";
            st.child_pid = None;
            st.state = if was_stopping {
                "stopped".to_string()
            } else if status.as_ref().is_ok_and(|s| s.success()) {
                "completed".to_string()
            } else {
                "failed".to_string()
            };
            st.revision = st.revision.saturating_add(1);
        });

        Ok(snap)
    }

    fn stop(&self) -> Value {
        let pid = {
            let mut st = lock_unpoison(&self.inner);
            if st.state == "running" {
                st.state = "stopping".to_string();
                st.revision = st.revision.saturating_add(1);
            }
            st.child_pid
        };
        if let Some(pid) = pid {
            signal_process_group(pid, "-TERM");
            let inner = Arc::clone(&self.inner);
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let still_alive = lock_unpoison(&inner).child_pid == Some(pid);
                if still_alive {
                    signal_process_group(pid, "-KILL");
                }
            });
        }
        self.status()
    }

    fn status(&self) -> Value {
        let st = lock_unpoison(&self.inner);
        Self::status_json(&st)
    }

    fn configure(&self, workspace_root: &Path, command: &str, cwd: &str, preview: Option<&str>) -> Result<Value, String> {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return Err("Run command cannot be empty.".to_string());
        }
        let clean_cwd = if cwd.trim().is_empty() { "." } else { cwd.trim() };
        let _ = resolve_safe_cwd(workspace_root, clean_cwd)?;
        let mut st = lock_unpoison(&self.inner);
        st.custom_target = Some(CustomRunTarget {
            command: trimmed.to_string(),
            cwd: clean_cwd.to_string(),
            preview: preview.map(str::trim).filter(|s| !s.is_empty()).map(String::from),
        });
        st.revision = st.revision.saturating_add(1);
        Ok(json!({ "configured": true, "id": "custom" }))
    }
}

fn make_target(id: &str, title: &str, command: &str, cwd: &str, preview: Option<&str>) -> Value {
    let rev_src = format!("{id}|{command}|{cwd}|{}", preview.unwrap_or(""));
    let revision = format!("{:08x}", fnv1a_32(rev_src.as_bytes()));
    let mut obj = json!({
        "id": id,
        "title": title,
        "name": command,
        "command": command,
        "cwd": cwd,
        "workingDirectory": cwd,
        "revision": revision,
        "kind": "command"
    });
    if let Some(p) = preview {
        obj["preview"] = json!(p);
    }
    obj
}

fn resolve_safe_cwd(workspace_root: &Path, rel_cwd: &str) -> Result<PathBuf, String> {
    let base = workspace_root
        .canonicalize()
        .map_err(|e| format!("Invalid workspace root: {e}"))?;
    let candidate = if Path::new(rel_cwd).is_absolute() {
        PathBuf::from(rel_cwd)
    } else {
        base.join(rel_cwd)
    };
    let canon = candidate
        .canonicalize()
        .map_err(|e| format!("Run directory '{rel_cwd}' does not exist: {e}"))?;
    if !canon.starts_with(&base) {
        return Err(format!("Run directory '{rel_cwd}' escapes workspace root."));
    }
    Ok(canon)
}

async fn drain_stream<R: tokio::io::AsyncRead + Unpin>(reader: R, inner: Arc<Mutex<ProjectRunState>>) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let mut st = lock_unpoison(&inner);
        st.output.push_str(&line);
        st.output.push('\n');
        if st.output.len() > MAX_RUN_OUTPUT_BYTES {
            let excess = st.output.len() - MAX_RUN_OUTPUT_BYTES;
            let mut cut = excess;
            while cut < st.output.len() && !st.output.is_char_boundary(cut) {
                cut += 1;
            }
            st.output.drain(..cut);
        }
        st.revision = st.revision.saturating_add(1);
    }
}

#[cfg(unix)]
fn libc_setpgid() {
    // Isolate child into its own process group using POSIX `setpgid(0, 0)` without extra crate deps.
    extern "C" {
        fn setpgid(pid: i32, pgid: i32) -> i32;
    }
    unsafe {
        setpgid(0, 0);
    }
}

fn signal_process_group(pid: u32, sig_flag: &str) {
    let _ = std::process::Command::new("/bin/kill")
        .arg(sig_flag)
        .arg(format!("-{pid}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_project_runner_start_output_and_completion() {
        let dir = std::env::temp_dir().join(format!("heav3ns-run-test-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let runner = ProjectRunner::create_runner();
        runner.configure(&dir, "echo zenith-run-ok", ".", None).unwrap();
        let targets = runner.discover(&dir);
        let custom = targets["targets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == "custom")
            .unwrap();
        let rev = custom["revision"].as_str().unwrap().to_string();

        let started = runner.start(&dir, "custom", Some(&rev)).unwrap();
        assert_eq!(started["state"], json!("running"));

        for _ in 0..40 {
            let st = runner.status();
            if st["state"] == json!("completed") {
                assert!(st["output"].as_str().unwrap().contains("zenith-run-ok"));
                let _ = std::fs::remove_dir_all(&dir);
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("ProjectRunner did not complete in time: {:?}", runner.status());
    }

    #[tokio::test]
    async fn test_project_runner_stop_kills_running_process_and_rejects_cwd_escape() {
        let dir = std::env::temp_dir().join(format!("heav3ns-stop-test-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let runner = ProjectRunner::create_runner();
        assert!(runner.configure(&dir, "echo nope", "../../etc", None).is_err());

        runner.configure(&dir, "sleep 30", ".", None).unwrap();
        runner.start(&dir, "custom", None).unwrap();
        let stopping = runner.stop();
        assert!(matches!(stopping["state"].as_str(), Some("stopping" | "stopped")));

        for _ in 0..40 {
            let st = runner.status();
            if st["state"] == json!("stopped") {
                let _ = std::fs::remove_dir_all(&dir);
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("ProjectRunner did not transition to stopped: {:?}", runner.status());
    }
}
