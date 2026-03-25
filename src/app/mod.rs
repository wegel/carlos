use std::env;
use std::fs;
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
use self::models::{
    DiffBlock, Message, MessageKind, RenderedLine, StyledSegment, TerminalSize, ThreadSummary,
};
use self::notifications::*;
#[cfg(test)]
use self::perf::{DurationSamples, PerfMetrics};
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
use crate::clipboard::*;
#[cfg(test)]
use crate::event::UiEvent;
use crate::protocol::*;
#[cfg(test)]
use crate::theme::*;

mod context_usage;
mod input;
mod mobile_mouse;
mod models;
mod notifications;
mod perf;
mod perf_session;
mod ralph;
mod render;
mod selection;
mod state;
mod terminal_ui;
mod text;
mod tools;

const MSG_TOP: usize = 1; // 1-based row index
const MSG_CONTENT_X: usize = 2; // 0-based x

#[derive(Debug, Clone, Default)]
struct CliOptions {
    mode_resume: bool,
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

fn usage() {
    eprintln!(
        "Usage:\n  carlos [resume [SESSION_ID]] [options]\n  carlos perf-session <SESSION_JSONL> [--width N] [--height N]\n  carlos perf-session --synthetic [--turns N] [--seed N] [--tool-lines N] [--width N] [--height N]\n\nOptions:\n  --ralph-prompt <path>          prompt file (default: .agents/ralph-prompt.md)\n  --ralph-done-marker <text>     completion marker (default: @@COMPLETE@@)\n  --ralph-blocked-marker <text>  blocked marker (default: @@BLOCKED@@)\n  --width <n>                    perf-session viewport width (default: 160)\n  --height <n>                   perf-session viewport height (default: 48)\n  --seed <n>                     perf-session synthetic seed (default: 1)\n  --turns <n>                    perf-session synthetic turns (default: 1000)\n  --tool-lines <n>               perf-session synthetic tool-output lines (default: 24)\n  --synthetic                    use generated perf-session content instead of a jsonl file\n  -h, --help                     show this help\n\nKeys:\n  Ctrl+R                         toggle Ralph mode on/off\n\nEnv:\n  CARLOS_METRICS=1               enable perf overlay + exit report (toggle: F8 or Ctrl+P)\n  CARLOS_REASONING_SUMMARY=...   auto | concise | detailed | none (default: auto)"
    );
}

fn resume_hint(thread_id: &str) -> String {
    format!("to resume this session use `carlos resume {thread_id}`")
}

fn parse_cli_args(args: impl IntoIterator<Item = String>) -> Result<CliOptions> {
    let mut opts = CliOptions {
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
            "-h" | "--help" => {
                opts.show_help = true;
            }
            "resume" => {
                if opts.mode_resume || opts.mode_perf_session {
                    bail!("choose only one mode");
                }
                opts.mode_resume = true;
                if let Some(next) = args.peek() {
                    if !next.starts_with('-') {
                        opts.resume_id = args.next();
                    }
                }
            }
            "perf-session" => {
                if opts.mode_resume || opts.mode_perf_session {
                    bail!("choose only one mode");
                }
                opts.mode_perf_session = true;
                if let Some(next) = args.peek() {
                    if !next.starts_with('-') {
                        opts.perf_session_path = args.next();
                    }
                }
            }
            "--synthetic" => {
                opts.perf_synthetic = true;
            }
            "--width" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --width"))?;
                opts.perf_width = value.parse().context("invalid --width")?;
            }
            "--height" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --height"))?;
                opts.perf_height = value.parse().context("invalid --height")?;
            }
            "--seed" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --seed"))?;
                opts.perf_seed = value.parse().context("invalid --seed")?;
            }
            "--turns" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --turns"))?;
                opts.perf_turns = value.parse().context("invalid --turns")?;
            }
            "--tool-lines" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --tool-lines"))?;
                opts.perf_tool_lines = value.parse().context("invalid --tool-lines")?;
            }
            "--ralph-prompt" => {
                let path = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --ralph-prompt"))?;
                opts.ralph_prompt_path = Some(path);
            }
            "--ralph-done-marker" => {
                let marker = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --ralph-done-marker"))?;
                opts.ralph_done_marker = Some(marker);
            }
            "--ralph-blocked-marker" => {
                let marker = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for --ralph-blocked-marker"))?;
                opts.ralph_blocked_marker = Some(marker);
            }
            _ => {
                bail!("unknown argument: {arg}");
            }
        }
    }

    if opts.mode_perf_session {
        if opts.perf_synthetic {
            if opts.perf_session_path.is_some() {
                bail!("choose either perf-session <SESSION_JSONL> or perf-session --synthetic");
            }
        } else if opts.perf_session_path.is_none() && !opts.show_help {
            bail!("missing session path for perf-session");
        }
    }

    Ok(opts)
}

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

fn fetch_model_catalog(client: &AppServerClient) -> Result<Vec<ModelInfo>> {
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

    let mut client = AppServerClient::start()?;
    initialize_client(&client)?;
    let server_events_rx = client.take_events_rx()?;

    let cwd = env::current_dir()?.to_string_lossy().to_string();
    let persisted_defaults = load_runtime_defaults();
    let default_summary = env_reasoning_summary_override()
        .or(persisted_defaults.summary.clone())
        .or(Some("auto".to_string()));

    let (chosen_thread_id, start_resp) = if opts.mode_resume {
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
                params_thread_list(&cwd),
                Duration::from_secs(15),
            )?;
            let list = parse_thread_list(&list_resp)?;
            let picked = pick_thread(&list)?;
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
            params_thread_start(&cwd),
            Duration::from_secs(20),
        )?;
        let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
        (thread_id, resp)
    };

    let mut app = AppState::new(chosen_thread_id);
    app.configure_ralph_options(
        PathBuf::from(&cwd),
        opts.ralph_prompt_path,
        opts.ralph_done_marker,
        opts.ralph_blocked_marker,
    );
    if env_flag_enabled("CARLOS_METRICS") {
        app.enable_perf_metrics();
    }
    if let Ok(models) = fetch_model_catalog(&client) {
        app.set_available_models(models);
    }
    let runtime_settings = resolve_initial_runtime_settings(
        parse_thread_runtime_settings(&start_resp)?,
        &persisted_defaults,
        default_summary.clone(),
    );
    app.set_runtime_settings(
        runtime_settings.model,
        runtime_settings.effort,
        runtime_settings.summary,
    );
    load_history_from_start_or_resume(&mut app, &start_resp)?;
    app.set_status("ready");

    let out = run_conversation_tui(&client, &mut app, server_events_rx);
    eprintln!("{}", resume_hint(&app.thread_id));
    if let Some(report) = app.perf_report() {
        eprintln!("{report}");
    }
    out
}

#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
