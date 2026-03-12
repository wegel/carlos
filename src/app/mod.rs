use std::env;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Result};

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
use self::text::{slice_by_cells, visual_width, wrap_input_line, wrap_natural_by_cells};
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
    resume_id: Option<String>,
    ralph_prompt_path: Option<String>,
    ralph_done_marker: Option<String>,
    ralph_blocked_marker: Option<String>,
    show_help: bool,
}

fn usage() {
    eprintln!(
        "Usage:\n  carlos [resume [SESSION_ID]] [options]\n\nOptions:\n  --ralph-prompt <path>          prompt file (default: .agents/ralph-prompt.md)\n  --ralph-done-marker <text>     completion marker (default: @@COMPLETE@@)\n  --ralph-blocked-marker <text>  blocked marker (default: @@BLOCKED@@)\n  -h, --help                     show this help\n\nKeys:\n  Ctrl+R                         toggle Ralph mode on/off\n\nEnv:\n  CARLOS_METRICS=1               enable perf overlay + exit report (toggle: F8 or Ctrl+P)"
    );
}

fn resume_hint(thread_id: &str) -> String {
    format!("to resume this session use `carlos resume {thread_id}`")
}

fn parse_cli_args(args: impl IntoIterator<Item = String>) -> Result<CliOptions> {
    let mut opts = CliOptions::default();
    let mut args = args.into_iter().peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                opts.show_help = true;
            }
            "resume" => {
                if opts.mode_resume {
                    bail!("`resume` specified more than once");
                }
                opts.mode_resume = true;
                if let Some(next) = args.peek() {
                    if !next.starts_with('-') {
                        opts.resume_id = args.next();
                    }
                }
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

    let mut client = AppServerClient::start()?;
    initialize_client(&client)?;
    let server_events_rx = client.take_events_rx()?;

    let cwd = env::current_dir()?.to_string_lossy().to_string();

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
    let runtime_settings = parse_thread_runtime_settings(&start_resp)?;
    app.set_runtime_settings(runtime_settings.model, runtime_settings.effort);
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
