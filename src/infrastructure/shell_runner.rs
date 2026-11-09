// [LAYER: INFRASTRUCTURE]
use crate::domain::types::runtime::CancellationToken;
use crate::plumbing::secret_redactor::SafetyPolicy;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// Executes a child shell command with timeout and cooperative cancellation.
pub async fn execute_shell_command(
    cmd_str: &str,
    cwd: &Path,
    timeout_ms: u64,
    cancel_token: Option<&CancellationToken>,
) -> Result<(Value, bool), String> {
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd_str)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Failed to spawn shell command: {e}"))?;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();

    let read_fut = async {
        let (mut out_buf, mut err_buf) = (Vec::new(), Vec::new());
        if let Some(ref mut out) = stdout_pipe {
            let _ = out.read_to_end(&mut out_buf).await;
        }
        if let Some(ref mut err) = stderr_pipe {
            let _ = err.read_to_end(&mut err_buf).await;
        }
        let status = child.wait().await;
        (status, out_buf, err_buf)
    };

    let timeout_dur = Duration::from_millis(timeout_ms);
    let outcome = if let Some(tok) = cancel_token {
        tokio::select! {
            _ = tok.cancelled() => return Err("Command execution cancelled by user".to_string()),
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
                "stdout": stdout,
                "stderr": stderr
            });
            if !advisories.is_empty() {
                payload["ergonomicAdvisories"] = json!(advisories);
            }
            Ok((payload, false))
        }
        Ok((Err(e), _, _)) => Err(format!("Command execution failed: {e}")),
        Err(_) => Err(format!("Command timed out after {timeout_ms}ms")),
    }
}

