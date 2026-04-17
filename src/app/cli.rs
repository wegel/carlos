//! CLI parsing, environment helpers, and runtime defaults persistence.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use super::RuntimeDefaults;
use crate::protocol_params::ThreadRuntimeSettings;

// --- CLI Types ---
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Backend {
    #[default]
    Codex,
    Claude,
}

impl Backend {
    fn cli_name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct CliOptions {
    pub(super) backend: Backend,
    pub(super) mode_resume: bool,
    pub(super) mode_continue: bool,
    pub(super) mode_perf_session: bool,
    pub(super) perf_synthetic: bool,
    pub(super) resume_id: Option<String>,
    pub(super) perf_session_path: Option<String>,
    pub(super) perf_width: usize,
    pub(super) perf_height: usize,
    pub(super) perf_seed: u64,
    pub(super) perf_turns: usize,
    pub(super) perf_tool_lines: usize,
    pub(super) ralph_prompt_path: Option<String>,
    pub(super) ralph_done_marker: Option<String>,
    pub(super) ralph_blocked_marker: Option<String>,
    pub(super) show_help: bool,
}

// --- Runtime Settings ---
pub(super) fn resolve_initial_runtime_settings(
    runtime: ThreadRuntimeSettings,
    defaults: &RuntimeDefaults,
    default_summary: Option<String>,
) -> ThreadRuntimeSettings {
    ThreadRuntimeSettings {
        model: runtime.model.or_else(|| defaults.model.clone()),
        effort: runtime.effort.or_else(|| defaults.effort.clone()),
        summary: runtime.summary.or(default_summary),
    }
}

pub(super) fn usage() {
    eprintln!(
        "Usage:\n  carlos [resume [SESSION_ID] | continue] [options]\n  carlos perf-session <SESSION_JSONL> [--width N] [--height N]\n  carlos perf-session --synthetic [--turns N] [--seed N] [--tool-lines N] [--width N] [--height N]\n\nOptions:\n  --backend <codex|claude>       backend to use (default: codex)\n  --ralph-prompt <path>          prompt file (default: .agents/ralph-prompt.md)\n  --ralph-done-marker <text>     completion marker (default: @@COMPLETE@@)\n  --ralph-blocked-marker <text>  blocked marker (default: @@BLOCKED@@)\n  --width <n>                    perf-session viewport width (default: 160)\n  --height <n>                   perf-session viewport height (default: 48)\n  --seed <n>                     perf-session synthetic seed (default: 1)\n  --turns <n>                    perf-session synthetic turns (default: 1000)\n  --tool-lines <n>               perf-session synthetic tool-output lines (default: 24)\n  --synthetic                    use generated perf-session content instead of a jsonl file\n  -h, --help                     show this help\n\nKeys:\n  Ctrl+R                         toggle Ralph mode on/off\n\nEnv:\n  CARLOS_BACKEND=claude          use Claude Code instead of codex app-server\n  CARLOS_METRICS=1               enable perf overlay + exit report (toggle: F8 or Ctrl+P)\n  CARLOS_REASONING_SUMMARY=...   auto | concise | detailed | none (default: auto)"
    );
}

pub(super) fn resume_hint(backend: Backend, thread_id: &str) -> String {
    format!(
        "to resume this session use:\ncarlos --backend {} resume {thread_id}",
        backend.cli_name()
    )
}

pub(super) fn styled_resume_hint(backend: Backend, thread_id: &str) -> String {
    let plain = resume_hint(backend, thread_id);
    let command = format!(
        "carlos --backend {} resume {thread_id}",
        backend.cli_name()
    );
    plain.replace(&command, &format!("\x1b[94m{command}\x1b[0m"))
}

// --- CLI Parsing ---
pub(super) fn parse_cli_args(args: impl IntoIterator<Item = String>) -> Result<CliOptions> {
    let mut opts = CliOptions {
        backend: env_backend().unwrap_or(Backend::Codex),
        perf_width: 160,
        perf_height: 48,
        perf_seed: 1,
        perf_turns: 1_000,
        perf_tool_lines: 24,
        ..CliOptions::default()
    };
    let mut args = args.into_iter().peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => opts.show_help = true,
            "--backend" => opts.backend = parse_backend_name(&take_value(&mut args, &arg)?)?,
            "--synthetic" => opts.perf_synthetic = true,
            "--width" => opts.perf_width = parse_value(&mut args, &arg)?,
            "--height" => opts.perf_height = parse_value(&mut args, &arg)?,
            "--seed" => opts.perf_seed = parse_value(&mut args, &arg)?,
            "--turns" => opts.perf_turns = parse_value(&mut args, &arg)?,
            "--tool-lines" => opts.perf_tool_lines = parse_value(&mut args, &arg)?,
            "--ralph-prompt" => opts.ralph_prompt_path = Some(take_value(&mut args, &arg)?),
            "--ralph-done-marker" => opts.ralph_done_marker = Some(take_value(&mut args, &arg)?),
            "--ralph-blocked-marker" => {
                opts.ralph_blocked_marker = Some(take_value(&mut args, &arg)?);
            }
            sub @ ("resume" | "continue" | "perf-session") => {
                parse_mode_subcommand(sub, &mut opts, &mut args)?;
            }
            _ => bail!("unknown argument: {arg}"),
        }
    }
    validate_perf_session_opts(&opts)?;
    Ok(opts)
}

fn take_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| anyhow::anyhow!("missing value for {flag}"))
}

fn parse_value<T: std::str::FromStr>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    take_value(args, flag)?.parse().context(format!("invalid {flag}"))
}

fn parse_mode_subcommand(
    sub: &str,
    opts: &mut CliOptions,
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
) -> Result<()> {
    if opts.mode_resume || opts.mode_continue || opts.mode_perf_session {
        bail!("choose only one mode");
    }
    match sub {
        "resume" => {
            opts.mode_resume = true;
            if args.peek().is_some_and(|n| !n.starts_with('-')) {
                opts.resume_id = args.next();
            }
        }
        "continue" => opts.mode_continue = true,
        "perf-session" => {
            opts.mode_perf_session = true;
            if args.peek().is_some_and(|n| !n.starts_with('-')) {
                opts.perf_session_path = args.next();
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_perf_session_opts(opts: &CliOptions) -> Result<()> {
    if !opts.mode_perf_session {
        return Ok(());
    }
    if opts.perf_synthetic && opts.perf_session_path.is_some() {
        bail!("choose either perf-session <SESSION_JSONL> or perf-session --synthetic");
    }
    if !opts.perf_synthetic && opts.perf_session_path.is_none() && !opts.show_help {
        bail!("missing session path for perf-session");
    }
    Ok(())
}

// --- Env Helpers ---
pub(super) fn env_flag_enabled(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return true;
            }
            !matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        }
        Err(_) => false,
    }
}

pub(super) fn env_reasoning_summary_override() -> Option<String> {
    match env::var("CARLOS_REASONING_SUMMARY") {
        Ok(value) => match value.trim() {
            "" => Some("auto".to_string()),
            "auto" | "concise" | "detailed" | "none" => Some(value.trim().to_string()),
            _ => Some("auto".to_string()),
        },
        Err(_) => None,
    }
}

fn parse_backend_name(value: &str) -> Result<Backend> {
    match value.trim().to_ascii_lowercase().as_str() {
        "codex" => Ok(Backend::Codex),
        "claude" => Ok(Backend::Claude),
        other => bail!("invalid backend: {other}"),
    }
}

fn env_backend() -> Option<Backend> {
    let value = env::var("CARLOS_BACKEND").ok()?;
    parse_backend_name(value.trim()).ok()
}

// --- Runtime Defaults ---
fn runtime_defaults_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("carlos").join("runtime-defaults.json"))
}

pub(super) fn load_runtime_defaults_from(path: &Path) -> RuntimeDefaults {
    let Ok(contents) = fs::read_to_string(path) else {
        return RuntimeDefaults::default();
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return RuntimeDefaults::default();
    };
    RuntimeDefaults {
        model: parsed
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned),
        effort: parsed
            .get("effort")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned),
        summary: parsed
            .get("summary")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned),
    }
}

pub(super) fn load_runtime_defaults() -> RuntimeDefaults {
    let Some(path) = runtime_defaults_path() else {
        return RuntimeDefaults::default();
    };
    load_runtime_defaults_from(&path)
}

pub(super) fn persist_runtime_defaults_to(path: &Path, defaults: &RuntimeDefaults) -> Result<()> {
    let Some(parent) = path.parent() else {
        bail!("invalid runtime defaults path");
    };
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let payload = serde_json::json!({
        "model": defaults.model,
        "effort": defaults.effort,
        "summary": defaults.summary,
    });
    let bytes = serde_json::to_vec_pretty(&payload)?;
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub(super) fn persist_runtime_defaults(defaults: &RuntimeDefaults) -> Result<()> {
    let Some(path) = runtime_defaults_path() else {
        bail!("cannot determine config directory");
    };
    persist_runtime_defaults_to(&path, defaults)
}
