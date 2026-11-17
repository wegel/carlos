//! CLI parsing, environment helpers, and runtime defaults persistence.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

// --- CLI Types ---

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Backend {
    #[default]
    Codex,
    Claude,
    Grok,
}

#[derive(Debug, Clone, Default)]
pub(super) struct CliOptions {
    pub(super) backend: Backend,
    pub(super) codex_socket: Option<PathBuf>,
    pub(super) daemon_socket: Option<PathBuf>,
    pub(super) daemon: bool,
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
    pub(super) ralph_waiting_marker: Option<String>,
    pub(super) dictation_profile: Option<String>,
    pub(super) show_help: bool,
    pub(super) session_control: Option<super::session_cli::SessionControl>,
}

// --- Runtime Settings ---

pub(super) fn usage() {
    eprintln!(
        "Usage:\n  carlos [resume [SESSION_ID] | continue] [options]\n  carlos session <stop|reopen> ID [--daemon-socket PATH]\n  carlos perf-session <SESSION_JSONL> [--width N] [--height N]\n  carlos perf-session --synthetic [--turns N] [--seed N] [--tool-lines N] [--width N] [--height N]\n\nOptions:\n  --backend <codex|claude|grok>  backend to use (default: codex)\n  --codex-socket <path>          attach to an existing local Codex server\n  --daemon                      use the managed local daemon (the default)\n  --daemon-socket <path>         connect the terminal to a local Carlos daemon\n  --dictation-profile <name>     dictation profile to activate for this session\n  --ralph-prompt <path>          prompt file (default: .agents/ralph-prompt.md)\n  --ralph-done-marker <text>     completion marker (default: @@COMPLETE@@)\n  --ralph-blocked-marker <text>  blocked marker (default: @@BLOCKED@@)\n  --ralph-waiting-marker <text>  human gate marker (default: @@AWAITING_HUMAN@@)\n  --width <n>                    perf-session viewport width (default: 160)\n  --height <n>                   perf-session viewport height (default: 48)\n  --seed <n>                     perf-session synthetic seed (default: 1)\n  --turns <n>                    perf-session synthetic turns (default: 1000)\n  --tool-lines <n>               perf-session synthetic tool-output lines (default: 24)\n  --synthetic                    use generated perf-session content instead of a jsonl file\n  -h, --help                     show this help\n\nKeys:\n  F5                             reconnect a disconnected Codex session\n  Ctrl+R                         toggle Ralph mode on/off\n\nEnv:\n  CARLOS_BACKEND=claude|grok     use Claude Code or Grok instead of codex app-server\n  CARLOS_STRICT_DAEMON_BUILD=1   require an exact managed daemon build (default: off)\n  CARLOS_METRICS=1               enable perf overlay + exit report (toggle: F8 or Ctrl+P)\n  CARLOS_REASONING_SUMMARY=...   auto | concise | detailed | none (default: concise)"
    );
}

/// Print the daemon's logical address without exposing a native backend ID.
pub(super) fn daemon_resume_hint(socket: &Path, session: &carlos_proto::ids::SessionId) -> String {
    format!(
        "to resume this session use:\ncarlos --daemon-socket {} resume {}",
        shell_quote(&socket.to_string_lossy()),
        session
    )
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/_-.".contains(&byte))
    {
        value.into()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
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
            "--daemon" => opts.daemon = true,
            "--codex-socket" => {
                let path = PathBuf::from(take_value(&mut args, &arg)?);
                opts.codex_socket = Some(if path.is_absolute() {
                    path
                } else {
                    env::current_dir()?.join(path)
                });
            }
            "--daemon-socket" => {
                opts.daemon_socket = Some(PathBuf::from(take_value(&mut args, &arg)?))
            }
            "--backend" => opts.backend = parse_backend_name(&take_value(&mut args, &arg)?)?,
            "--dictation-profile" => opts.dictation_profile = Some(take_value(&mut args, &arg)?),
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
            "--ralph-waiting-marker" => {
                opts.ralph_waiting_marker = Some(take_value(&mut args, &arg)?);
            }
            sub @ ("resume" | "continue" | "perf-session" | "session") => {
                parse_mode_subcommand(sub, &mut opts, &mut args)?;
            }
            _ => bail!("unknown argument: {arg}"),
        }
    }
    validate_routes(&opts)?;
    Ok(opts)
}

fn validate_routes(opts: &CliOptions) -> Result<()> {
    if opts.session_control.is_some() && opts.codex_socket.is_some() {
        bail!("Session controls use a logical ID; --codex-socket is a conversation launch choice");
    }
    if opts.codex_socket.is_some() && (opts.backend != Backend::Codex || opts.mode_perf_session) {
        bail!("--codex-socket requires the Codex conversation backend");
    }
    validate_perf_session_opts(opts)?;
    if opts.daemon && (opts.daemon_socket.is_some() || opts.mode_perf_session) {
        bail!("--daemon cannot be combined with --daemon-socket or perf-session");
    }
    Ok(())
}

fn take_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| anyhow::anyhow!("missing value for {flag}"))
}

fn parse_value<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    take_value(args, flag)?
        .parse()
        .context(format!("invalid {flag}"))
}

fn parse_mode_subcommand(
    sub: &str,
    opts: &mut CliOptions,
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
) -> Result<()> {
    if opts.mode_resume
        || opts.mode_continue
        || opts.mode_perf_session
        || opts.session_control.is_some()
    {
        bail!("choose only one mode");
    }
    match sub {
        "session" => opts.session_control = Some(super::session_cli::parse(args)?),
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

fn parse_backend_name(value: &str) -> Result<Backend> {
    match value.trim().to_ascii_lowercase().as_str() {
        "codex" => Ok(Backend::Codex),
        "claude" => Ok(Backend::Claude),
        "grok" => Ok(Backend::Grok),
        other => bail!("invalid backend: {other}"),
    }
}

fn env_backend() -> Option<Backend> {
    let value = env::var("CARLOS_BACKEND").ok()?;
    parse_backend_name(value.trim()).ok()
}

// --- Runtime Defaults ---

/// Print the logical daemon address with only the command in the existing blue style.
pub(super) fn styled_daemon_resume_hint(
    socket: &Path,
    session: &carlos_proto::ids::SessionId,
) -> String {
    let plain = daemon_resume_hint(socket, session);
    let (label, command) = plain.split_once('\n').expect("resume command");
    format!("{label}\n{}", crate::theme::ansi_blue(command))
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
