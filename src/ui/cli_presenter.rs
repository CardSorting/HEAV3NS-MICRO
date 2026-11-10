// [LAYER: UI]
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Heav3nsUserConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub workspace_root: Option<String>,
}

impl Heav3nsUserConfig {
    pub fn config_dir() -> PathBuf {
        if let Ok(home) = env::var("HOME") {
            PathBuf::from(home).join(".heav3ns")
        } else {
            PathBuf::from(".heav3ns")
        }
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn load() -> Option<Self> {
        let path = Self::config_path();
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self) -> io::Result<()> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
        }
        let path = Self::config_path();
        let json_str = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(&path, json_str)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Parses workspace `.env` into an immutable map without mutating process-wide `std::env` (thread-safe under Tokio).
    pub fn read_dotenv_map(workspace: &Path) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        let env_file = workspace.join(".env");
        if let Ok(content) = fs::read_to_string(env_file) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let stripped = trimmed.strip_prefix("export ").unwrap_or(trimmed).trim();
                if let Some((k, v)) = stripped.split_once('=') {
                    let key = k.trim();
                    let val = v.trim().trim_matches('"').trim_matches('\'');
                    if !key.is_empty() && !val.is_empty() {
                        map.insert(key.to_string(), val.to_string());
                    }
                }
            }
        }
        map
    }

    /// Resolves an environment variable first from `std::env`, then from the parsed workspace `.env` map.
    pub fn resolve_env_or_dotenv(key: &str, dotenv: &BTreeMap<String, String>) -> Option<String> {
        env::var(key)
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| dotenv.get(key).cloned().filter(|s| !s.is_empty()))
    }
}

#[derive(Debug, Clone)]
pub struct CliOptions {
    pub mode: RunMode,
    pub format: String,
    pub output: Option<PathBuf>,
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub endpoint: Option<String>,
    pub prompt: Option<String>,
    pub stress: bool,
    pub last_only: bool,
    pub tests_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunMode {
    FlightTest,
    Bench,
    Smoke,
    StdioWorker,
    SinglePrompt,
    Interactive,
    Doctor,
    Context,
    Receipts,
    PreviewRequest,
    Models { refresh: bool },
    Init,
    Demo,
    Tools,
    ConfigShow,
    ConfigSet { key: String, value: String },
    Help,
    Version,
}

pub fn parse_args() -> CliOptions {
    parse_args_from(env::args().skip(1).collect())
}

pub fn parse_args_from(args: Vec<String>) -> CliOptions {
    let mut opts = CliOptions {
        mode: RunMode::Interactive,
        format: "text".to_string(),
        output: None,
        cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        model: None,
        provider: None,
        endpoint: None,
        prompt: None,
        stress: false,
        last_only: false,
        tests_dir: None,
    };

    if args.is_empty() {
        return opts;
    }

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "doctor" | "--doctor" => {
                opts.mode = RunMode::Doctor;
            }
            "context" | "--context" => {
                opts.mode = RunMode::Context;
            }
            "receipts" | "--receipts" => {
                opts.mode = RunMode::Receipts;
            }
            "preview-request" | "--preview-request" => {
                opts.mode = RunMode::PreviewRequest;
            }
            "models" | "--models" => {
                let refresh = args.iter().any(|a| a == "--refresh" || a == "refresh");
                opts.mode = RunMode::Models { refresh };
            }
            "--refresh" => {
                if let RunMode::Models { ref mut refresh } = opts.mode {
                    *refresh = true;
                }
            }
            "--last" => {
                opts.last_only = true;
            }
            "init" | "--init" | "setup" => {
                opts.mode = RunMode::Init;
            }
            "demo" | "--demo" | "tutorial" | "tour" => {
                opts.mode = RunMode::Demo;
            }
            "tools" | "--tools" => {
                opts.mode = RunMode::Tools;
            }
            "config" => {
                if let Some(sub) = args.get(i + 1) {
                    if sub == "set" {
                        if let (Some(k), Some(v)) = (args.get(i + 2), args.get(i + 3)) {
                            opts.mode = RunMode::ConfigSet { key: k.clone(), value: v.clone() };
                            i += 3;
                        } else {
                            opts.mode = RunMode::ConfigShow;
                            i += 1;
                        }
                    } else {
                        opts.mode = RunMode::ConfigShow;
                        i += 1;
                    }
                } else {
                    opts.mode = RunMode::ConfigShow;
                }
            }
            "flight" | "--flight" | "--benchmark-flight" => opts.mode = RunMode::FlightTest,
            "bench" | "--bench" => opts.mode = RunMode::Bench,
            "--stress" => opts.stress = true,
            "--tests-dir" => {
                if let Some(v) = args.get(i + 1) {
                    opts.tests_dir = Some(PathBuf::from(v));
                    i += 1;
                }
            }
            "smoke" | "--smoke" => opts.mode = RunMode::Smoke,
            "stdio" | "--stdio" | "--worker" | "--desktop-worker" | "--desktop-account" => opts.mode = RunMode::StdioWorker,
            "help" | "-h" | "--help" => opts.mode = RunMode::Help,
            "version" | "-v" | "-V" | "--version" => opts.mode = RunMode::Version,
            "--json" => opts.format = "json".to_string(),
            "--format" => {
                if let Some(v) = args.get(i + 1) {
                    opts.format = v.clone();
                    i += 1;
                }
            }
            "--output" | "-o" => {
                if let Some(v) = args.get(i + 1) {
                    opts.output = Some(PathBuf::from(v));
                    i += 1;
                }
            }
            "--cwd" => {
                if let Some(v) = args.get(i + 1) {
                    opts.cwd = PathBuf::from(v);
                    i += 1;
                }
            }
            "--model" | "-m" => {
                if let Some(v) = args.get(i + 1) {
                    opts.model = Some(v.clone());
                    i += 1;
                }
            }
            "--provider" => {
                if let Some(v) = args.get(i + 1) {
                    opts.provider = Some(v.clone());
                    i += 1;
                }
            }
            "--endpoint" => {
                if let Some(v) = args.get(i + 1) {
                    opts.endpoint = Some(v.clone());
                    i += 1;
                }
            }
            "run" => {
                if let Some(v) = args.get(i + 1) {
                    opts.prompt = Some(v.clone());
                    opts.mode = RunMode::SinglePrompt;
                    i += 1;
                }
            }
            "--prompt" | "-p" => {
                if let Some(v) = args.get(i + 1) {
                    opts.prompt = Some(v.clone());
                    opts.mode = RunMode::SinglePrompt;
                    i += 1;
                }
            }
            other if !other.starts_with('-') && opts.prompt.is_none() && opts.mode == RunMode::Interactive => {
                opts.prompt = Some(other.to_string());
                opts.mode = RunMode::SinglePrompt;
            }
            _ => {}
        }
        i += 1;
    }
    opts
}

pub fn print_help() {
    println!(
        "HEAV3NS-MICRO v0.1.0 — Sovereign Agentic Coding Runtime
Distilled from LUMI-NEW (zero bloat, 16-MiB Arena slab, self-healing tool parser, 29 native tools)

USAGE:
  heav3ns-micro [COMMAND] [OPTIONS] [PROMPT]

START HERE:
  heav3ns-micro                  Launch interactive agent workspace REPL (default)
  heav3ns-micro init             Run interactive setup wizard to configure providers & keys
  heav3ns-micro demo             Run instant zero-key guided tour of tools, VFS & memory
  heav3ns-micro doctor           Audit environment credentials, local AI, and runtime SLA
  heav3ns-micro context          Inspect token budget HUD, KV-cache prefix, and tool routing
  heav3ns-micro preview-request  Zero-side-effect outbound wire manifest & byte accounting
  heav3ns-micro receipts         Inspect prose-free CAS turn receipts (--last, --json)
  heav3ns-micro tools            Explore 31 built-in tools across all 7 Sovereign Pillars

CORE COMMANDS:
  run <PROMPT>                   Execute a single agent turn and output the result
  config show                    Display current user settings from ~/.heav3ns/config.json
  config set <KEY> <VALUE>       Update setting (provider, model, endpoint, api_key)
  flight                         Run the 5-scenario Agent Flight Test suite (FT-01..FT-05)
  bench                          Run substrate micro-benchmarks + flight suite
  smoke                          Run deterministic runtime self-check & smoke verification
  stdio                          Run multiplexed async JSONL stdio worker protocol

OPTIONS:
  -m, --model <NAME>             Model name (e.g. gpt-5.6-terra, gemini-2.0-flash, llama3.2)
  --provider <NAME>              Provider: openai, gemini, anthropic, openrouter, deepseek, groq, xai, ollama
  --endpoint <URL>               Custom API endpoint URL (default depends on provider)
  --cwd <DIR>                    Workspace root directory (default: current directory)
  -p, --prompt <TEXT>            Execute single agent prompt and exit
  --last                         Show only the latest turn receipt (with `receipts`)
  --json                         Emit machine-readable JSON output
  --format <text|json>           Output format for flight and bench (default: text)
  -o, --output <PATH>            Write flight report JSON to file
  -h, --help                     Show this help message

SUPPORTED PROVIDERS:
  • OpenAI:        export OPENAI_API_KEY=\"sk-...\" (default model: gpt-5.6-terra)
  • Google Gemini: export GEMINI_API_KEY=\"...\"   (heav3ns-micro --provider gemini -m gemini-2.0-flash)
  • Anthropic:     export ANTHROPIC_API_KEY=\"...\" (heav3ns-micro --provider anthropic -m claude-3-5-sonnet-20241022)
  • OpenRouter:    export OPENROUTER_API_KEY=\"...\" (heav3ns-micro --provider openrouter -m anthropic/claude-3.7-sonnet)
  • DeepSeek/Groq: export DEEPSEEK_API_KEY / GROQ_API_KEY / XAI_API_KEY
  • Ollama:        Zero key needed! Auto-detected on localhost:11434 (heav3ns-micro --provider ollama -m llama3.2)
  • Local/vLLM:    heav3ns-micro --endpoint http://localhost:8000/v1 -m custom-model

INTERACTIVE REPL SLASH COMMANDS:
  /help       List all 28+ slash commands     /mode       Step 5-stop execution posture
  /context    Visual token budget & prompt HUD/preview    Outbound wire manifest & bytes
  /receipts   Prose-free CAS turn receipts    /undo       Path-scoped CAS turn rollback
  /diff       Unified LCS diff of changes     /commit     Commit staged VFS changes to disk
  /doctor     Inspect SLA and environment     /tools      Core + 8-LRU deferred tool status
  /stats      Arena & token allocation stats  /exit       Exit interactive REPL

EXAMPLES:
  # 1. First time setup:
  heav3ns-micro init

  # 2. Test without any API key:
  heav3ns-micro demo

  # 3. Diagnose your setup:
  heav3ns-micro doctor

  # 4. Use free local Ollama:
  heav3ns-micro --provider ollama -m llama3.2

  # 5. Execute a quick prompt:
  heav3ns-micro \"Inspect src/main.rs and summarize the CLI options\"
"
    );
}

pub use crate::ui::stdio_worker::run_stdio_worker;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_subcommands() {
        let opt_doc = parse_args_from(vec!["doctor".into()]);
        assert_eq!(opt_doc.mode, RunMode::Doctor);

        let opt_ctx = parse_args_from(vec!["context".into(), "--json".into()]);
        assert_eq!(opt_ctx.mode, RunMode::Context);
        assert_eq!(opt_ctx.format, "json");

        let opt_init = parse_args_from(vec!["init".into()]);
        assert_eq!(opt_init.mode, RunMode::Init);

        let opt_demo = parse_args_from(vec!["demo".into()]);
        assert_eq!(opt_demo.mode, RunMode::Demo);

        let opt_tools = parse_args_from(vec!["tools".into()]);
        assert_eq!(opt_tools.mode, RunMode::Tools);

        let opt_flight = parse_args_from(vec!["flight".into()]);
        assert_eq!(opt_flight.mode, RunMode::FlightTest);

        let opt_cfg_set = parse_args_from(vec!["config".into(), "set".into(), "provider".into(), "gemini".into()]);
        assert_eq!(opt_cfg_set.mode, RunMode::ConfigSet { key: "provider".into(), value: "gemini".into() });

        let opt_run = parse_args_from(vec!["run".into(), "hello world".into()]);
        assert_eq!(opt_run.mode, RunMode::SinglePrompt);
        assert_eq!(opt_run.prompt, Some("hello world".into()));

        let opt_raw_prompt = parse_args_from(vec!["write unit test".into()]);
        assert_eq!(opt_raw_prompt.mode, RunMode::SinglePrompt);
        assert_eq!(opt_raw_prompt.prompt, Some("write unit test".into()));

        let opt_ver = parse_args_from(vec!["--version".into()]);
        assert_eq!(opt_ver.mode, RunMode::Version);
        let opt_v = parse_args_from(vec!["-v".into()]);
        assert_eq!(opt_v.mode, RunMode::Version);

        let opt_rec = parse_args_from(vec!["receipts".into(), "--last".into(), "--json".into()]);
        assert_eq!(opt_rec.mode, RunMode::Receipts);
        assert!(opt_rec.last_only);
        assert_eq!(opt_rec.format, "json");

        let opt_prev = parse_args_from(vec!["preview-request".into(), "--json".into()]);
        assert_eq!(opt_prev.mode, RunMode::PreviewRequest);
    }
}
