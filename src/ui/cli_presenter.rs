// [LAYER: UI]
use std::env;
use std::path::PathBuf;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    FlightTest,
    Bench,
    Smoke,
    StdioWorker,
    SinglePrompt,
    Interactive,
    Help,
}

pub fn parse_args() -> CliOptions {
    let mut opts = CliOptions {
        mode: RunMode::Interactive,
        format: "text".to_string(),
        output: None,
        cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        model: None,
        provider: None,
        endpoint: None,
        prompt: None,
    };

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--flight" | "--benchmark-flight" => opts.mode = RunMode::FlightTest,
            "--bench" => opts.mode = RunMode::Bench,
            "--smoke" => opts.mode = RunMode::Smoke,
            "--stdio" | "--worker" | "--desktop-worker" | "--desktop-account" => opts.mode = RunMode::StdioWorker,
            "-h" | "--help" => opts.mode = RunMode::Help,
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
            "--prompt" | "-p" => {
                if let Some(v) = args.get(i + 1) {
                    opts.prompt = Some(v.clone());
                    opts.mode = RunMode::SinglePrompt;
                    i += 1;
                }
            }
            other if !other.starts_with('-') && opts.prompt.is_none() => {
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
        "HEAV3NS-MICRO v0.1.0 — Hyper-Compressed Rust Micro Agent Runtime
Distilled from LUMI-NEW (zero bloat, 16-MiB Arena slab, self-healing tool parser, 23 native tools, wave scheduler)

USAGE:
  heav3ns-micro [OPTIONS] [PROMPT]

MODES:
  --flight, --benchmark-flight   Run the 5-scenario Agent Flight Test suite (FT-01..FT-05)
  --bench                        Run substrate micro-benchmarks + Flight Test suite
  --smoke                        Run deterministic runtime self-check & smoke verification
  --stdio                        Run headless newline-delimited JSONL stdio worker protocol
  -p, --prompt <TEXT>            Execute a single agent turn and exit

OPTIONS:
  --format <text|json>                  Output format for --flight (default: text)
  -o, --output <PATH>                   Write JSON flight report to file
  --cwd <DIR>                           Workspace root directory
  -m, --model <NAME>                    Active model name (default: gpt-5.6-terra)
  --provider <openai|anthropic|gemini>  Provider wire protocol (default: openai)
  --endpoint <URL>                      Provider API endpoint URL
  -h, --help                            Show this help message"
    );
}

pub use crate::ui::stdio_worker::run_stdio_worker;
