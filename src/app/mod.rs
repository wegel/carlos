//! Application entry point, CLI parsing, backend setup, and module declarations.

// --- Imports ---

use std::env;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};

#[cfg(test)]
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
#[cfg(test)]
use ratatui::buffer::Buffer;
#[cfg(test)]
use ratatui::layout::Rect;
#[cfg(test)]
use ratatui::style::Style;
#[cfg(test)]
use serde_json::json;

#[cfg(test)]
use self::context_usage::{context_label_reserved_cells, context_usage_label, ContextUsage};
use self::input::*;
#[cfg(test)]
use self::mobile_mouse::{
    apply_mobile_mouse_scroll, consume_mobile_mouse_char, parse_mobile_mouse_coords,
    parse_repeated_plain_mobile_pair, MobileMouseConsume,
};
pub(crate) use self::models::Role;
#[cfg(test)]
use self::models::StyledSegment;
use self::models::{DiffBlock, Message, MessageKind, RenderedLine, TerminalSize, ThreadSummary};
use self::notifications::*;
#[cfg(test)]
use self::perf::{DurationSamples, PerfMetrics};
#[cfg(test)]
use self::picker_render::*;
#[cfg(test)]
use self::render::*;
#[cfg(test)]
use self::selection::{
    compute_selection_range, decide_mouse_drag_mode, selected_text, MouseDragMode, Selection,
};
use self::state::AppState;
use self::terminal_ui::*;
#[cfg(test)]
use self::text::{
    slice_by_cells, visual_width, wrap_input_line, wrap_input_line_count, wrap_natural_by_cells,
    wrap_natural_count_by_cells,
};
#[cfg(test)]
use self::tools::*;
#[cfg(test)]
use self::transcript_render::*;
use crate::backend::BackendClient;
use crate::claude_backend::{
    claude_model_catalog, claude_project_dir_name, load_claude_local_history, ClaudeClient,
    ClaudeLaunchMode,
    CLAUDE_PENDING_THREAD_ID,
};
#[cfg(test)]
use crate::clipboard::*;
#[cfg(test)]
use crate::event::UiEvent;
use crate::protocol::*;
#[cfg(test)]
use crate::theme::*;

// --- Module Declarations ---

mod approval_state;
mod context_usage;
mod input;
mod input_events;
mod input_history_state;
mod mobile_mouse;
mod models;
mod notification_items;
mod notifications;
mod overlay_render;
mod perf;
mod perf_session;
mod picker_render;
mod ralph;
mod ralph_runtime_state;
mod render;
mod render_cache_state;
mod runtime_settings_state;
mod selection;
mod state;
mod terminal_ui;
mod text;
mod tool_diff;
mod tool_shell;
mod tools;
mod transcript_diff;
mod transcript_render;
mod transcript_styles;
mod viewport_state;

// --- Constants ---

const MSG_TOP: usize = 1; // 1-based row index
const MSG_CONTENT_X: usize = 2; // 0-based x

// --- Types ---

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Backend {
    #[default]
    Codex,
    Claude,
}

#[derive(Debug, Clone, Default)]
struct CliOptions {
    backend: Backend,
    mode_resume: bool,
    mode_continue: bool,
    mode_perf_session: bool,
    perf_synthetic: bool,
    resume_id: Option<String>,
    perf_session_path: Option<String>,
    perf_width: usize,
    perf_height: usize,
    perf_seed: u64,
    perf_turns: usize,
    perf_tool_lines: usize,
    ralph_prompt_path: Option<String>,
    ralph_done_marker: Option<String>,
    ralph_blocked_marker: Option<String>,
    show_help: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeDefaults {
    pub(super) model: Option<String>,
    pub(super) effort: Option<String>,
    pub(super) summary: Option<String>,
}

// --- Runtime Settings ---

fn resolve_initial_runtime_settings(
    runtime: crate::protocol::ThreadRuntimeSettings,
    defaults: &RuntimeDefaults,
    default_summary: Option<String>,
) -> crate::protocol::ThreadRuntimeSettings {
    crate::protocol::ThreadRuntimeSettings {
        model: runtime.model.or_else(|| defaults.model.clone()),
        effort: runtime.effort.or_else(|| defaults.effort.clone()),
        summary: runtime.summary.or(default_summary),
    }
}

// --- CLI Parsing ---

fn usage() {
    eprintln!(
        "Usage:\n  carlos [resume [SESSION_ID] | continue] [options]\n  carlos perf-session <SESSION_JSONL> [--width N] [--height N]\n  carlos perf-session --synthetic [--turns N] [--seed N] [--tool-lines N] [--width N] [--height N]\n\nOptions:\n  --backend <codex|claude>       backend to use (default: codex)\n  --ralph-prompt <path>          prompt file (default: .agents/ralph-prompt.md)\n  --ralph-done-marker <text>     completion marker (default: @@COMPLETE@@)\n  --ralph-blocked-marker <text>  blocked marker (default: @@BLOCKED@@)\n  --width <n>                    perf-session viewport width (default: 160)\n  --height <n>                   perf-session viewport height (default: 48)\n  --seed <n>                     perf-session synthetic seed (default: 1)\n  --turns <n>                    perf-session synthetic turns (default: 1000)\n  --tool-lines <n>               perf-session synthetic tool-output lines (default: 24)\n  --synthetic                    use generated perf-session content instead of a jsonl file\n  -h, --help                     show this help\n\nKeys:\n  Ctrl+R                         toggle Ralph mode on/off\n\nEnv:\n  CARLOS_BACKEND=claude          use Claude Code instead of codex app-server\n  CARLOS_METRICS=1               enable perf overlay + exit report (toggle: F8 or Ctrl+P)\n  CARLOS_REASONING_SUMMARY=...   auto | concise | detailed | none (default: auto)"
    );
}

fn resume_hint(thread_id: &str) -> String {
    format!("to resume this session use:\ncarlos resume {thread_id}")
}

fn styled_resume_hint(thread_id: &str) -> String {
    let plain = resume_hint(thread_id);
    let command = format!("carlos resume {thread_id}");
    plain.replace(&command, &format!("\x1b[94m{command}\x1b[0m"))
}

fn parse_cli_args(args: impl IntoIterator<Item = String>) -> Result<CliOptions> {
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

// --- Environment Helpers ---

fn env_flag_enabled(name: &str) -> bool {
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

fn env_reasoning_summary_override() -> Option<String> {
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

// --- Runtime Defaults Persistence ---

fn runtime_defaults_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("carlos").join("runtime-defaults.json"))
}

fn load_runtime_defaults_from(path: &std::path::Path) -> RuntimeDefaults {
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

fn load_runtime_defaults() -> RuntimeDefaults {
    let Some(path) = runtime_defaults_path() else {
        return RuntimeDefaults::default();
    };
    load_runtime_defaults_from(&path)
}

fn persist_runtime_defaults_to(path: &std::path::Path, defaults: &RuntimeDefaults) -> Result<()> {
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

// --- Model Catalog ---

fn fetch_model_catalog(client: &dyn BackendClient) -> Result<Vec<ModelInfo>> {
    let mut cursor: Option<String> = None;
    let mut out = Vec::new();

    loop {
        let resp = client.call(
            "model/list",
            params_model_list(cursor.as_deref()),
            Duration::from_secs(10),
        )?;
        let (mut page, next_cursor) = parse_model_list_page(&resp)?;
        out.append(&mut page);
        if next_cursor.is_none() {
            break;
        }
        cursor = next_cursor;
    }

    Ok(out)
}

// --- Run Entry Point ---

pub(crate) fn run() -> Result<()> {
    let opts = match parse_cli_args(env::args().skip(1)) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("error: {err}");
            usage();
            return Ok(());
        }
    };
    if opts.show_help {
        usage();
        return Ok(());
    }
    if opts.mode_perf_session {
        return perf_session::run_perf_session(&opts);
    }

    let cwd_path = env::current_dir()?;
    let cwd = cwd_path.to_string_lossy().to_string();
    let persisted_defaults = load_runtime_defaults();
    let default_summary = env_reasoning_summary_override()
        .or(persisted_defaults.summary.clone())
        .or(Some("auto".to_string()));

    match opts.backend {
        Backend::Codex => {
            run_codex_backend(&opts, &cwd_path, &cwd, persisted_defaults, default_summary)
        }
        Backend::Claude => {
            run_claude_backend(&opts, &cwd_path, &cwd, persisted_defaults, default_summary)
        }
    }
}

// --- App Configuration Helpers ---

fn configure_app_common(
    app: &mut AppState,
    cwd: &str,
    opts: &CliOptions,
    runtime_settings: crate::protocol::ThreadRuntimeSettings,
    persisted_defaults: &RuntimeDefaults,
    default_summary: Option<String>,
) {
    app.configure_ralph_options(
        PathBuf::from(cwd),
        opts.ralph_prompt_path.clone(),
        opts.ralph_done_marker.clone(),
        opts.ralph_blocked_marker.clone(),
    );
    if env_flag_enabled("CARLOS_METRICS") {
        app.enable_perf_metrics();
    }
    let runtime_settings =
        resolve_initial_runtime_settings(runtime_settings, persisted_defaults, default_summary);
    app.set_runtime_settings(
        runtime_settings.model,
        runtime_settings.effort,
        runtime_settings.summary,
    );
    app.set_status("ready");
}

fn finish_run(
    app: &mut AppState,
    client: &dyn BackendClient,
    server_events_rx: std::sync::mpsc::Receiver<String>,
) -> Result<()> {
    let out = run_conversation_tui(client, app, server_events_rx);
    eprintln!("{}", styled_resume_hint(&app.thread_id));
    if let Some(report) = app.perf_report() {
        eprintln!("{report}");
    }
    out
}

// --- Claude Session Helpers ---

fn claude_projects_root() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".claude").join("projects"))
}

fn file_mtime_secs(path: &std::path::Path) -> i64 {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn claude_preview_text_from_record(record: &serde_json::Value) -> Option<String> {
    let message = record.get("message")?.as_object()?;
    match message.get("content")? {
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        serde_json::Value::Array(parts) => parts.iter().find_map(|part| {
            let text = match part.get("type").and_then(serde_json::Value::as_str) {
                Some("text") => part.get("text").and_then(serde_json::Value::as_str),
                Some("tool_result") => part.get("content").and_then(serde_json::Value::as_str),
                _ => None,
            }?;
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }),
        _ => None,
    }
}

fn summarize_claude_session_file(path: &std::path::Path, cwd: &str) -> Option<ThreadSummary> {
    let session_id = path.file_stem()?.to_string_lossy().to_string();
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let modified = file_mtime_secs(path);

    let mut name = None;
    let mut preview = None;
    let mut record_cwd = None;

    for line in reader.lines().filter_map(|line| line.ok()) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };

        if name.is_none() {
            name = record
                .get("customTitle")
                .or_else(|| record.get("agentName"))
                .or_else(|| record.get("slug"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }

        if record_cwd.is_none() {
            record_cwd = record
                .get("cwd")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }

        if preview.is_none() {
            preview = claude_preview_text_from_record(&record);
        }
    }

    Some(ThreadSummary {
        id: session_id.clone(),
        name,
        preview: preview.unwrap_or_else(|| session_id.clone()),
        cwd: record_cwd.unwrap_or_else(|| cwd.to_string()),
        created_at: modified,
        updated_at: modified,
    })
}

fn load_claude_thread_summaries(cwd_path: &std::path::Path, cwd: &str) -> Result<Vec<ThreadSummary>> {
    let Some(projects_root) = claude_projects_root() else {
        return Ok(Vec::new());
    };
    let project_dir = projects_root.join(claude_project_dir_name(cwd_path));
    if !project_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in fs::read_dir(&project_dir)
        .with_context(|| format!("failed to read Claude projects dir {}", project_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        if let Some(summary) = summarize_claude_session_file(&path, cwd) {
            out.push(summary);
        }
    }
    Ok(out)
}

// --- Codex Backend ---

fn run_codex_backend(
    opts: &CliOptions,
    _cwd_path: &std::path::Path,
    cwd: &str,
    persisted_defaults: RuntimeDefaults,
    default_summary: Option<String>,
) -> Result<()> {
    let mut client = AppServerClient::start()?;
    initialize_client(&client)?;
    let server_events_rx = client.take_events_rx()?;

    let (chosen_thread_id, start_resp) = if opts.mode_resume || opts.mode_continue {
        if let Some(rid) = opts.resume_id.as_deref() {
            let resp = client.call(
                "thread/resume",
                params_thread_resume(rid),
                Duration::from_secs(20),
            )?;
            let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
            (thread_id, resp)
        } else {
            let list_resp = client.call(
                "thread/list",
                params_thread_list(cwd),
                Duration::from_secs(15),
            )?;
            let list = parse_thread_list(&list_resp)?;
            let picked = if opts.mode_continue {
                sort_threads_for_picker(&list)
                    .into_iter()
                    .next()
                    .map(|t| t.id)
            } else {
                pick_thread(&list, true, |thread| {
                    let resp = client.call(
                        "thread/archive",
                        params_thread_archive(&thread.id),
                        Duration::from_secs(20),
                    )?;
                    extract_result_object(&resp).map(|_| ())
                })?
            };
            let Some(session_id) = picked else {
                return Ok(());
            };

            let resp = client.call(
                "thread/resume",
                params_thread_resume(&session_id),
                Duration::from_secs(20),
            )?;
            let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
            (thread_id, resp)
        }
    } else {
        let resp = client.call(
            "thread/start",
            params_thread_start(cwd),
            Duration::from_secs(20),
        )?;
        let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
        (thread_id, resp)
    };

    let mut app = AppState::new(chosen_thread_id);
    app.set_runtime_capabilities(true, true);
    if let Ok(models) = fetch_model_catalog(&client) {
        app.set_available_models(models);
    }
    configure_app_common(
        &mut app,
        cwd,
        opts,
        parse_thread_runtime_settings(&start_resp)?,
        &persisted_defaults,
        default_summary,
    );
    load_history_from_start_or_resume(&mut app, &start_resp)?;
    finish_run(&mut app, &client, server_events_rx)
}

// --- Claude Backend ---

fn run_claude_backend(
    opts: &CliOptions,
    cwd_path: &std::path::Path,
    cwd: &str,
    persisted_defaults: RuntimeDefaults,
    _default_summary: Option<String>,
) -> Result<()> {
    let launch_mode = if opts.mode_continue {
        ClaudeLaunchMode::Continue
    } else if opts.mode_resume && opts.resume_id.is_none() {
        let list = load_claude_thread_summaries(cwd_path, cwd)?;
        let Some(session_id) = pick_thread(&list, false, |_| Ok(()))? else {
            return Ok(());
        };
        ClaudeLaunchMode::Resume(session_id)
    } else if let Some(session_id) = opts.resume_id.clone() {
        ClaudeLaunchMode::Resume(session_id)
    } else {
        ClaudeLaunchMode::New
    };

    let local_history = load_claude_local_history(cwd_path, &launch_mode)?;
    let initial_thread_id = match &launch_mode {
        ClaudeLaunchMode::Resume(session_id) => session_id.clone(),
        ClaudeLaunchMode::Continue => local_history
            .as_ref()
            .map(|history| history.session_id.clone())
            .unwrap_or_else(|| CLAUDE_PENDING_THREAD_ID.to_string()),
        ClaudeLaunchMode::New => CLAUDE_PENDING_THREAD_ID.to_string(),
    };
    let mut client = ClaudeClient::start(cwd_path, launch_mode)?;
    let server_events_rx = client.take_events_rx()?;
    let start_resp = client.synthetic_start_response(
        &initial_thread_id,
        local_history.as_ref().map(|history| &history.thread),
    );
    let chosen_thread_id = parse_thread_id_from_start_or_resume(&start_resp)?;

    let mut app = AppState::new(chosen_thread_id);
    app.set_runtime_capabilities(true, false);
    app.set_available_models(claude_model_catalog());
    configure_app_common(
        &mut app,
        cwd,
        opts,
        crate::protocol::ThreadRuntimeSettings {
            model: None,
            effort: None,
            summary: None,
        },
        &RuntimeDefaults::default(),
        None,
    );
    if persisted_defaults.model.is_some() || persisted_defaults.effort.is_some() {
        app.queue_runtime_settings(
            persisted_defaults.model.clone(),
            persisted_defaults.effort.clone(),
            None,
        );
    }
    load_history_from_start_or_resume(&mut app, &start_resp)?;
    if let Some(request_line) = local_history
        .as_ref()
        .and_then(|history| history.pending_approval_request.as_deref())
    {
        let _ = handle_server_message_line(&mut app, request_line);
    }
    if (opts.mode_resume || opts.mode_continue)
        && local_history
            .as_ref()
            .map(|history| history.imported_item_count == 0)
            .unwrap_or(true)
    {
        app.append_message(
            Role::System,
            "Claude local transcript history could not be reconstructed from session storage"
                .to_string(),
        );
    }
    finish_run(&mut app, &client, server_events_rx)
}

// --- Tests ---

#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
