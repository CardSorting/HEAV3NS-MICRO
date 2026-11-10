// [LAYER: INFRASTRUCTURE]
use crate::domain::types::runtime::CancellationToken;
use crate::plumbing::seatbelt_profile::{build_macos_seatbelt_invocation, SeatbeltSandboxMode};
use crate::plumbing::secret_redactor::SafetyPolicy;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

const MAX_PIPE_CAPTURE_BYTES: usize = 4 * 1024 * 1024;

fn is_macos_seatbelt_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        if !cfg!(target_os = "macos") {
            return false;
        }
        std::process::Command::new("/usr/bin/sandbox-exec")
            .args(["-p", "(version 1)(allow default)", "--", "/usr/bin/true"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

fn kill_process_tree(pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(p) = pid {
        let _ = std::process::Command::new("/bin/kill")
            .args(["-TERM", "--", &format!("-{p}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = std::process::Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{p}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(unix))]
    let _ = pid;
}

async fn drain_pipe_bounded<R: tokio::io::AsyncRead + Unpin>(mut pipe: Option<R>, max_bytes: usize) -> Vec<u8> {
    let Some(ref mut reader) = pipe else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if out.len() < max_bytes {
                    let keep = (max_bytes - out.len()).min(n);
                    out.extend_from_slice(&chunk[..keep]);
                }
            }
        }
    }
    out
}

/// Executes a child shell command with concurrent pipe draining, process-group kill-tree cleanup,
/// cooperative cancellation, and optional macOS Seatbelt (`sandbox-exec`) confinement.
pub async fn execute_shell_command_sandboxed(
    cmd_str: &str,
    cwd: &Path,
    timeout_ms: u64,
    sandbox_mode: SeatbeltSandboxMode,
    cancel_token: Option<&CancellationToken>,
) -> Result<(Value, bool), String> {
    let home_dir = std::env::var("HOME").ok().map(PathBuf::from);
    let seatbelt = if is_macos_seatbelt_available() {
        build_macos_seatbelt_invocation(sandbox_mode, cwd, home_dir.as_deref())
    } else {
        None
    };

    let sandboxed = seatbelt.is_some();
    let mut cmd = if let Some(ref sb) = seatbelt {
        let mut c = Command::new("/usr/bin/sandbox-exec");
        c.arg("-p").arg(&sb.profile_sbpl);
        for (k, v) in &sb.parameters {
            c.arg(format!("-D{k}={v}"));
        }
        c.arg("--").arg("/bin/sh").arg("-c").arg(cmd_str);
        c
    } else {
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(cmd_str);
        c
    };

    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn shell command: {e}"))?;

    let child_pid = child.id();
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let read_fut = async {
        let (out_buf, err_buf) = tokio::join!(
            drain_pipe_bounded(stdout_pipe, MAX_PIPE_CAPTURE_BYTES),
            drain_pipe_bounded(stderr_pipe, MAX_PIPE_CAPTURE_BYTES),
        );
        let status = child.wait().await;
        (status, out_buf, err_buf)
    };

    let timeout_dur = Duration::from_millis(timeout_ms.clamp(100, 600_000));
    let outcome = if let Some(tok) = cancel_token {
        tokio::select! {
            biased;
            _ = tok.cancelled() => {
                kill_process_tree(child_pid);
                return Err("Command execution cancelled by user".to_string());
            }
            res = tokio::time::timeout(timeout_dur, read_fut) => res,
        }
    } else {
        tokio::time::timeout(timeout_dur, read_fut).await
    };

    match outcome {
        Ok((Ok(status), out_bytes, err_bytes)) => {
            let exit_code = status.code().unwrap_or(-1);
            let (cleaned_out, _pty_replies) = SafetyPolicy::strip_and_respond_pty_queries(&out_bytes, 24, 80);
            let stdout = String::from_utf8_lossy(&cleaned_out).into_owned();
            let stderr = String::from_utf8_lossy(&err_bytes).into_owned();
            let mut advisories = Vec::new();
            if let Some(adv) = SafetyPolicy::detect_heredoc_file_redirection(cmd_str) {
                advisories.push(adv);
            }
            if let Some(adv) = SafetyPolicy::detect_terminal_file_read(cmd_str) {
                advisories.push(adv);
            }
            if let Some(adv) = SafetyPolicy::detect_foreground_background_guidance(cmd_str) {
                advisories.push(adv);
            }
            let mut payload = json!({
                "exitCode": exit_code,
                "success": status.success(),
                "sandboxed": sandboxed,
                "stdout": stdout,
                "stderr": stderr
            });
            if !advisories.is_empty() {
                payload["ergonomicAdvisories"] = json!(advisories);
            }
            Ok((payload, false))
        }
        Ok((Err(e), _, _)) => {
            kill_process_tree(child_pid);
            Err(format!("Command execution failed: {e}"))
        }
        Err(_) => {
            kill_process_tree(child_pid);
            Err(format!("Command timed out after {timeout_ms}ms"))
        }
    }
}

/// Executes a child shell command without OS Seatbelt restriction (backwards-compatible entry point).
pub async fn execute_shell_command(
    cmd_str: &str,
    cwd: &Path,
    timeout_ms: u64,
    cancel_token: Option<&CancellationToken>,
) -> Result<(Value, bool), String> {
    execute_shell_command_sandboxed(cmd_str, cwd, timeout_ms, SeatbeltSandboxMode::Unrestricted, cancel_token).await
}
