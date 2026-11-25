// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::{ICiGateRunner, IJoyCiGateRunner, IWorkspaceProfileLoader};
use crate::domain::profiles::{
    AgentProfile, CiGatePolicy, CiGateReceipt, DEFAULT_JOY_CLI_PATH, DEFAULT_JOY_METHODOLOGY_PATH,
};
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::text_tokens::safe_utf8_prefix;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Concrete infrastructure runner executing `HEAV3NS-JOY` (`joy-check.mjs`) as the CI gate for development tasks.
#[derive(Debug, Default, Clone, Copy)]
pub struct JoyCiGateRunner;

impl JoyCiGateRunner {
    pub fn resolve_joy_methodology_dir() -> Option<String> {
        if let Ok(env_dir) = std::env::var("JOY_HOME") {
            let trimmed = env_dir.trim();
            if !trimmed.is_empty() && Path::new(trimmed).join("bin/joy-check.mjs").is_file() {
                return Some(trimmed.to_string());
            }
        }
        if Path::new(DEFAULT_JOY_CLI_PATH).is_file() {
            return Some(DEFAULT_JOY_METHODOLOGY_PATH.to_string());
        }
        None
    }

    /// Enforces an explicit trust boundary on `joy_cli_path` to prevent untrusted workspace configs from executing arbitrary scripts.
    pub fn resolve_trusted_cli_path(
        workspace_root: &Path,
        requested: &str,
    ) -> Result<Option<PathBuf>, String> {
        let trimmed = requested.trim();
        if trimmed.is_empty() || trimmed == DEFAULT_JOY_CLI_PATH {
            let p = PathBuf::from(DEFAULT_JOY_CLI_PATH);
            return Ok(p.is_file().then_some(p));
        }
        if let Ok(env_dir) = std::env::var("JOY_HOME") {
            let env_cli = PathBuf::from(env_dir.trim()).join("bin/joy-check.mjs");
            if Path::new(trimmed) == env_cli {
                return Ok(env_cli.is_file().then_some(env_cli));
            }
        }
        if let Ok(canon_root) = workspace_root.canonicalize() {
            let allowed_ws_rel = [
                canon_root.join("bin/joy-check.mjs"),
                canon_root.join("node_modules/heav3ns-joy/bin/joy-check.mjs"),
            ];
            let req_path = Path::new(trimmed);
            let candidate = if req_path.is_absolute() {
                req_path.to_path_buf()
            } else {
                canon_root.join(req_path)
            };
            if let Ok(canon_cand) = candidate.canonicalize() {
                if canon_cand.starts_with(&canon_root)
                    && allowed_ws_rel.iter().any(|a| a == &canon_cand)
                {
                    return Ok(canon_cand.is_file().then_some(canon_cand));
                }
            }
        }
        Err(format!(
            "UNTRUSTED_CI_GATE_PATH: '{trimmed}' is outside trusted HEAV3NS-JOY locations"
        ))
    }

    fn parse_spider_counts(output: &str) -> (usize, usize) {
        let mut errors = 0usize;
        let mut warnings = 0usize;
        for token in output.split_whitespace() {
            if let Some(val) = token.strip_prefix("errors=") {
                errors = val
                    .trim_end_matches(|c: char| !c.is_ascii_digit())
                    .parse()
                    .unwrap_or(0);
            } else if let Some(val) = token.strip_prefix("warnings=") {
                warnings = val
                    .trim_end_matches(|c: char| !c.is_ascii_digit())
                    .parse()
                    .unwrap_or(0);
            }
        }
        (errors, warnings)
    }

    fn compact_summary(stdout: &str, stderr: &str, exit_code: i32) -> String {
        let combined = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else if stdout.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            format!("{}\n{}", stdout.trim(), stderr.trim())
        };
        if combined.is_empty() {
            return format!("joy-check exited with code {exit_code}");
        }
        let clipped = safe_utf8_prefix(&combined, 1_800);
        if clipped.len() < combined.len() {
            format!("{clipped}\n...[truncated]")
        } else {
            clipped.to_string()
        }
    }

    fn run_command_bounded(mut cmd: Command, timeout_ms: u64) -> Result<(i32, String, String), String> {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();

        let out_handle = std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(ref mut p) = stdout_pipe {
                let _ = p.read_to_end(&mut buf);
            }
            String::from_utf8_lossy(&buf).into_owned()
        });
        let err_handle = std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(ref mut p) = stderr_pipe {
                let _ = p.read_to_end(&mut buf);
            }
            String::from_utf8_lossy(&buf).into_owned()
        });

        let timeout = Duration::from_millis(timeout_ms.clamp(500, 120_000));
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Ok(st.code().unwrap_or(1)),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        break Err(format!(
                            "HEAV3NS-JOY CI gate timed out after {}ms",
                            timeout.as_millis()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
                Err(e) => break Err(format!("wait failed: {e}")),
            }
        };

        let stdout = out_handle.join().unwrap_or_default();
        let stderr = err_handle.join().unwrap_or_default();
        match status {
            Ok(code) => Ok((code, stdout, stderr)),
            Err(msg) => Err(msg),
        }
    }
}

impl ICiGateRunner for JoyCiGateRunner {
    fn run_ci_gate(&self, workspace_root: &Path, policy: &CiGatePolicy) -> CiGateReceipt {
        let started_ms = now_ms();
        let theme = if policy.theme.trim().is_empty() {
            "strict"
        } else {
            policy.theme.trim()
        };
        let phase = if policy.phase.trim().is_empty() {
            "ci"
        } else {
            policy.phase.trim()
        };

        let trusted_cli = match Self::resolve_trusted_cli_path(workspace_root, &policy.joy_cli_path) {
            Ok(opt) => opt,
            Err(msg) => {
                return CiGateReceipt {
                    passed: false,
                    gate: "heav3ns-joy".to_string(),
                    theme: theme.to_string(),
                    phase: phase.to_string(),
                    exit_code: 2,
                    duration_ms: now_ms().saturating_sub(started_ms),
                    blocking_count: 1,
                    advisory_count: 0,
                    summary: msg,
                };
            }
        };

        let mut cmd = if let Some(cli_path) = trusted_cli {
            let mut c = Command::new("node");
            c.arg(cli_path).arg(workspace_root);
            c
        } else {
            let mut c = Command::new("npx");
            c.arg("-y")
                .arg("heav3ns-joy@1.0.0")
                .arg("check")
                .arg(workspace_root);
            c
        };
        cmd.arg("--phase")
            .arg(phase)
            .arg("--theme")
            .arg(theme)
            .arg("--format")
            .arg("compact");

        let timeout_ms = if policy.timeout_ms == 0 { 30_000 } else { policy.timeout_ms };
        match Self::run_command_bounded(cmd, timeout_ms) {
            Ok((exit_code, stdout, stderr)) => {
                let (mut blocking_count, advisory_count) = Self::parse_spider_counts(&stdout);
                if exit_code != 0 && blocking_count == 0 {
                    blocking_count = 1;
                }
                CiGateReceipt {
                    passed: exit_code == 0,
                    gate: "heav3ns-joy".to_string(),
                    theme: theme.to_string(),
                    phase: phase.to_string(),
                    exit_code,
                    duration_ms: now_ms().saturating_sub(started_ms),
                    blocking_count,
                    advisory_count,
                    summary: Self::compact_summary(&stdout, &stderr, exit_code),
                }
            }
            Err(err) => {
                let code = if err.contains("timed out") { 124 } else { 2 };
                CiGateReceipt {
                    passed: false,
                    gate: "heav3ns-joy".to_string(),
                    theme: theme.to_string(),
                    phase: phase.to_string(),
                    exit_code: code,
                    duration_ms: now_ms().saturating_sub(started_ms),
                    blocking_count: 1,
                    advisory_count: 0,
                    summary: err,
                }
            }
        }
    }
}

impl IJoyCiGateRunner for JoyCiGateRunner {}

impl IWorkspaceProfileLoader for JoyCiGateRunner {
    fn load_workspace_profiles(&self, workspace_root: &Path) -> Vec<AgentProfile> {
        let path = workspace_root.join(".heav3ns").join("profiles.json");
        let is_symlink = std::fs::symlink_metadata(&path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(true);
        if is_symlink || !path.is_file() {
            return Vec::new();
        }
        let Ok(raw) = crate::infrastructure::search_tools::read_text_file_bounded(&path, 64 * 1024)
        else {
            return Vec::new();
        };
        AgentProfile::parse_workspace_profiles_strict(&raw).unwrap_or_default()
    }
}
