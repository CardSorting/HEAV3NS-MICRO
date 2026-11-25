// [LAYER: UI]
use crate::domain::types::progress::{
    EngineProgressEvent, EngineProgressPhase, EngineProgressStatus,
};
use crate::domain::types::provider_registry::{
    lookup_provider_descriptor, retired_provider_remediation,
};
use crate::plumbing::hash_and_math::levenshtein;
use std::env;
use std::io::{self, Write};
use std::path::Path;

/// Check whether ANSI styling should be active based on environment.
pub fn is_color_enabled() -> bool {
    if env::var("NO_COLOR").is_ok() {
        return false;
    }
    if let Ok(term) = env::var("TERM") {
        if term.eq_ignore_ascii_case("dumb") {
            return false;
        }
    }
    true
}

/// Applies ANSI styling codes when colors are enabled.
pub fn style(text: &str, code: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn bold(text: &str) -> String {
    style(text, "1")
}

pub fn green(text: &str) -> String {
    style(text, "32")
}

pub fn yellow(text: &str) -> String {
    style(text, "33")
}

pub fn red(text: &str) -> String {
    style(text, "31")
}

pub fn cyan(text: &str) -> String {
    style(text, "36")
}

pub fn dim(text: &str) -> String {
    style(text, "2")
}

/// Returns `true` when `provider` is a zero-key local inference route (`ollama`, `vllm`, `sglang`, `lmstudio`, `apple-playground`, `custom`, `scripted`).
pub fn is_zero_key_local_provider(provider: &str) -> bool {
    provider.eq_ignore_ascii_case("scripted")
        || lookup_provider_descriptor(provider).is_some_and(|d| !d.requires_api_key && !d.retired)
}

/// Prints a welcome banner and runtime status pill.
pub fn print_welcome_banner(cwd: &Path, provider: &str, model: &str, has_key: bool) {
    let key_status = if retired_provider_remediation(provider).is_some() {
        red("[RETIRED PROVIDER — SWITCH TO GEMINI]")
    } else if is_zero_key_local_provider(provider) {
        green("[LOCAL AI — ZERO KEY NEEDED]")
    } else if has_key {
        green("[AUTHENTICATED]")
    } else {
        yellow("[SETUP REQUIRED — NO API KEY DETECTED]")
    };

    println!();
    println!(
        "{}",
        cyan("╔══════════════════════════════════════════════════════════════════════════════╗")
    );
    println!(
        "{}  {}  {}",
        cyan("║"),
        bold("HEAV3NS-MICRO v0.1.0 — Sovereign Agentic Coding Runtime"),
        cyan("║")
    );
    println!(
        "{}  {} {}",
        cyan("║"),
        dim("16-MiB Arena · 31 Tools · 5-Stop Posture · CAS Receipts · KV-Pinned"),
        cyan("║")
    );
    println!(
        "{}",
        cyan("╠══════════════════════════════════════════════════════════════════════════════╣")
    );
    println!(
        "{}  {} {} {} {}",
        cyan("║"),
        bold("Workspace:"),
        dim(&cwd.display().to_string()),
        cyan("│"),
        key_status
    );
    println!(
        "{}  {} {} ({})",
        cyan("║"),
        bold("Active:   "),
        cyan(provider),
        dim(model)
    );
    println!(
        "{}",
        cyan("╚══════════════════════════════════════════════════════════════════════════════╝")
    );
}

/// Displays an onboarding quickstart notice for users without API keys configured.
pub fn print_onboarding_notice(provider: &str, model: &str) {
    let desc = lookup_provider_descriptor(provider).filter(|d| !d.retired);
    let primary_env = desc
        .and_then(|d| d.env_vars.first().copied())
        .unwrap_or("OPENAI_API_KEY");
    let slot = desc.map(|d| d.secret_slot).unwrap_or(provider);
    let portal = desc
        .map(|d| d.credential_url)
        .unwrap_or("https://platform.openai.com/api-keys");
    println!();
    println!(
        "{}",
        yellow("┌──────────────────────────────────────────────────────────────────────────────┐")
    );
    println!(
        "{}  {}  {}",
        yellow("│"),
        bold("QUICKSTART & ONBOARDING GUIDE"),
        yellow("│")
    );
    println!(
        "{}",
        yellow("├──────────────────────────────────────────────────────────────────────────────┤")
    );
    println!(
        "{}  No API key is configured for provider '{}' (model '{}').",
        yellow("│"),
        cyan(provider),
        dim(model)
    );
    println!(
        "{}  Here are your fastest options to get started immediately:",
        yellow("│")
    );
    println!("{}", yellow("│"));
    println!(
        "{}  {} {}",
        yellow("│"),
        bold("1. Configure Active Provider Credential:"),
        yellow("│")
    );
    println!(
        "{}     Export: {}export {primary_env}=\"<YOUR_KEY>\"",
        yellow("│"),
        dim("")
    );
    println!(
        "{}     Vault:  {}",
        yellow("│"),
        cyan(&format!(
            "heav3ns-micro config set key.{slot} \"<YOUR_KEY>\""
        ))
    );
    println!("{}     Portal: {}", yellow("│"), dim(portal));
    println!("{}", yellow("│"));
    println!(
        "{}  {} {}",
        yellow("│"),
        bold("2. Instant Offline Demo (Zero Setup / Zero Keys):"),
        yellow("│")
    );
    println!(
        "{}     Run {} or type {} inside this session.",
        yellow("│"),
        cyan("heav3ns-micro demo"),
        cyan("/demo")
    );
    println!("{}", yellow("│"));
    println!(
        "{}  {} {}",
        yellow("│"),
        bold("3. Free Local AI (Ollama / vLLM / SGLang / LM Studio):"),
        yellow("│")
    );
    println!(
        "{}     Run: {}",
        yellow("│"),
        cyan("heav3ns-micro --provider ollama -m qwen2.5-coder:14b")
    );
    println!("{}", yellow("│"));
    println!(
        "{}  {} {}",
        yellow("│"),
        bold("4. Interactive Setup Wizard (65 Providers):"),
        yellow("│")
    );
    println!(
        "{}     Run: {}  (or switch live via {})",
        yellow("│"),
        cyan("heav3ns-micro init"),
        cyan("/provider list")
    );
    println!(
        "{}  {} Type {} to diagnose or {} for all commands.",
        yellow("│"),
        bold("Tip:"),
        cyan("/doctor"),
        cyan("/help")
    );
    println!(
        "{}",
        yellow("└──────────────────────────────────────────────────────────────────────────────┘")
    );
    println!();
}

/// Formats a provider-aware authentication & remediation card when an LLM provider rejects credentials.
pub fn format_auth_error_card(raw_err: &str, provider: &str, model: &str) -> String {
    let mut out = String::new();
    out.push('\n');
    out.push_str(&yellow(
        "┌──────────────────────────────────────────────────────────────────────────────┐\n",
    ));
    if let Some(retired_msg) = retired_provider_remediation(provider) {
        out.push_str(&format!(
            "{}  {}  {}\n",
            yellow("│"),
            bold("RETIRED PROVIDER SELECTOR — ACTION REQUIRED"),
            yellow("│")
        ));
        out.push_str(&yellow(
            "├──────────────────────────────────────────────────────────────────────────────┤\n",
        ));
        out.push_str(&format!("{}  {}\n", yellow("│"), red(retired_msg)));
        out.push_str(&format!(
            "{}    1. Switch in session: {}\n",
            yellow("│"),
            cyan("/provider gemini")
        ));
        out.push_str(&format!(
            "{}    2. Export Gemini key: {}\n",
            yellow("│"),
            cyan("export GEMINI_API_KEY=\"AIza...\"")
        ));
        out.push_str(&yellow(
            "└──────────────────────────────────────────────────────────────────────────────┘\n",
        ));
        return out;
    }

    let desc = lookup_provider_descriptor(provider);
    let disp = desc.map(|d| d.display_name).unwrap_or(provider);
    let env_list = desc
        .filter(|d| !d.env_vars.is_empty())
        .map(|d| d.env_vars.join(" / "))
        .unwrap_or_else(|| "CUSTOM_LLM_API_KEY".to_string());
    let primary_env = desc
        .and_then(|d| d.env_vars.first().copied())
        .unwrap_or("CUSTOM_LLM_API_KEY");
    let slot = desc.map(|d| d.secret_slot).unwrap_or(provider);
    let portal = desc
        .map(|d| d.credential_url)
        .unwrap_or("https://openrouter.ai/keys");

    out.push_str(&format!(
        "{}  {}  {}\n",
        yellow("│"),
        bold("AUTHENTICATION REQUIRED — LLM PROVIDER ERROR"),
        yellow("│")
    ));
    out.push_str(&yellow(
        "├──────────────────────────────────────────────────────────────────────────────┤\n",
    ));
    out.push_str(&format!(
        "{}  Provider '{}' ({}) with model '{}' rejected the request.\n",
        yellow("│"),
        cyan(provider),
        dim(disp),
        dim(model)
    ));
    let summary = if raw_err.contains("401")
        || raw_err.contains("API key")
        || raw_err.contains("Authorization")
    {
        "Missing or invalid API key."
    } else {
        "Request failed to authenticate with the provider."
    };
    out.push_str(&format!(
        "{}  Reason: {} (expected env: {})\n",
        yellow("│"),
        red(summary),
        cyan(&env_list)
    ));
    out.push_str(&format!(
        "{}  Credential portal: {}\n",
        yellow("│"),
        dim(portal)
    ));
    out.push_str(&yellow("│\n"));
    out.push_str(&format!(
        "{}  {} Choose one of the following solutions:\n",
        yellow("│"),
        bold("HOW TO RESOLVE:")
    ));
    out.push_str(&format!(
        "{}    1. Export provider key:    {}\n",
        yellow("│"),
        cyan(&format!("export {primary_env}=\"<YOUR_KEY>\""))
    ));
    out.push_str(&format!(
        "{}    2. Save to local vault:    {}\n",
        yellow("│"),
        cyan(&format!(
            "heav3ns-micro config set key.{slot} \"<YOUR_KEY>\""
        ))
    ));
    out.push_str(&format!(
        "{}    3. Switch live in session: {}\n",
        yellow("│"),
        cyan("/provider list  (or /provider ollama)")
    ));
    out.push_str(&format!(
        "{}    4. Run setup wizard:       {}\n",
        yellow("│"),
        cyan("heav3ns-micro init")
    ));
    out.push_str(&format!(
        "{}    5. Test runtime offline:   {}\n",
        yellow("│"),
        cyan("heav3ns-micro demo")
    ));
    out.push_str(&yellow(
        "└──────────────────────────────────────────────────────────────────────────────┘\n",
    ));
    out
}

/// Runs a diagnostic health check of the CLI environment, keys, git workspace, and runtime.
pub fn create_cli_progress_listener() -> std::sync::Arc<dyn Fn(EngineProgressEvent) + Send + Sync> {
    std::sync::Arc::new(|evt| {
        let elapsed = evt.elapsed_ms.unwrap_or(0);
        let time_str = if elapsed > 100 {
            format!(" ({}ms)", elapsed)
        } else {
            String::new()
        };

        match evt.phase {
            EngineProgressPhase::Connecting if evt.status == EngineProgressStatus::Started => {
                println!("  {} Connecting to model{}...", cyan("⠋"), dim(&time_str));
                let _ = io::stdout().flush();
            }
            EngineProgressPhase::Thinking => {
                println!(
                    "  {} Model reasoning & planning{}...",
                    cyan("⠋"),
                    dim(&time_str)
                );
                let _ = io::stdout().flush();
            }
            EngineProgressPhase::Tool if evt.status == EngineProgressStatus::Started => {
                for item in evt.message.split("; ") {
                    if let Some((tool, target)) = item.split_once(": ") {
                        println!("  {} {}: {}", yellow("▶"), bold(tool), cyan(target));
                    } else {
                        println!("  {} Calling: {}", yellow("▶"), cyan(item));
                    }
                }
                let _ = io::stdout().flush();
            }
            EngineProgressPhase::Tool if evt.status == EngineProgressStatus::Completed => {
                for item in evt.message.split("; ") {
                    if item.contains("(ok") {
                        println!("  {} {}", green("✔"), item);
                    } else {
                        println!("  {} {}", red("✖"), item);
                    }
                }
                let _ = io::stdout().flush();
            }
            EngineProgressPhase::Planning => {
                println!("  {} {}", dim("•"), dim(&evt.message));
                let _ = io::stdout().flush();
            }
            EngineProgressPhase::Failed => {
                println!("  {} {}", red("✖"), red(&evt.message));
                let _ = io::stdout().flush();
            }
            _ => {}
        }
    })
}

/// Formats a clean turn telemetry summary footer for interactive feedback.
pub fn format_turn_summary(model: &str, duration_ms: u64, tool_count: usize) -> String {
    let dur_sec = (duration_ms as f64) / 1000.0;
    format!(
        "\n{}\n",
        dim(&format!(
            "─── ✦ Turn Completed · Model: {} · Tools: {} · Duration: {:.2}s ───",
            model, tool_count, dur_sec
        ))
    )
}

/// Renders syntax-colored unified diff output (+ green, - red, @@ cyan, header bold).
pub fn render_color_diff(raw_diff: &str) -> String {
    let text = if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw_diff) {
        if let Some(ud) = val.get("unifiedDiff").and_then(|v| v.as_str()) {
            if ud.is_empty() {
                return format!("  {} No pending uncommitted changes in VFS.\n", green("✔"));
            }
            let files_summary = if let Some(arr) = val.get("files").and_then(|v| v.as_array()) {
                let names: Vec<_> = arr.iter().filter_map(|v| v.as_str()).collect();
                format!(
                    "Pending changes across {} file(s): {}\n\n",
                    names.len(),
                    names.join(", ")
                )
            } else {
                String::new()
            };
            format!("{}{}", files_summary, ud)
        } else {
            raw_diff.to_string()
        }
    } else {
        raw_diff.to_string()
    };

    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            out.push_str(&green(line));
        } else if line.starts_with('-') && !line.starts_with("---") {
            out.push_str(&red(line));
        } else if line.starts_with("@@") {
            out.push_str(&cyan(line));
        } else if line.starts_with("diff") || line.starts_with("---") || line.starts_with("+++") {
            out.push_str(&bold(line));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Suggests the closest valid slash command for a mistyped input.
pub fn suggest_command(input: &str, candidates: &[&str]) -> Option<String> {
    let target = input.trim();
    let mut best_match = None;
    let mut min_dist = usize::MAX;

    for &cand in candidates {
        let dist = levenshtein(target, cand);
        if dist < min_dist && dist <= 3 {
            min_dist = dist;
            best_match = Some(cand.to_string());
        }
    }

    best_match
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggest_command_fuzzy_matches() {
        let commands = [
            "/help", "/stats", "/vfs", "/diff", "/commit", "/doctor", "/memory",
        ];
        assert_eq!(
            suggest_command("/stat", &commands),
            Some("/stats".to_string())
        );
        assert_eq!(
            suggest_command("/hlp", &commands),
            Some("/help".to_string())
        );
        assert_eq!(
            suggest_command("/doctro", &commands),
            Some("/doctor".to_string())
        );
        assert_eq!(
            suggest_command("/memor", &commands),
            Some("/memory".to_string())
        );
        assert_eq!(suggest_command("/completely_unrelated", &commands), None);
    }

    #[test]
    fn test_format_auth_error_card_contains_resolution_steps() {
        let card = format_auth_error_card("HTTP 401: Invalid API key", "openai", "gpt-5.6-terra");
        assert!(card.contains("AUTHENTICATION REQUIRED"));
        assert!(card.contains("heav3ns-micro init"));
        assert!(card.contains("heav3ns-micro demo"));
        assert!(card.contains("OPENAI_API_KEY"));
        assert!(card.contains("ollama"));
    }

    #[test]
    fn test_color_styling_functions() {
        let s = bold("hello");
        assert!(s.contains("hello"));
        let g = green("success");
        assert!(g.contains("success"));
        let y = yellow("warning");
        assert!(y.contains("warning"));
        let r = red("error");
        assert!(r.contains("error"));
    }

    #[test]
    fn test_format_turn_summary_and_render_color_diff() {
        let summary = format_turn_summary("gpt-5.6-terra", 2500, 3);
        assert!(summary.contains("Turn Completed"));
        assert!(summary.contains("gpt-5.6-terra"));
        assert!(summary.contains("2.50s"));
        assert!(summary.contains("Tools: 3"));

        let diff_sample =
            "--- a/file.rs\n+++ b/file.rs\n@@ -1,3 +1,4 @@\n-old line\n+new line\n context";
        let colored = render_color_diff(diff_sample);
        assert!(colored.contains("old line"));
        assert!(colored.contains("new line"));
    }
}
