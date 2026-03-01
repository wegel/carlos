use std::env;
use std::time::Duration;

use anyhow::Result;

#[cfg(test)]
use crossterm::event::{Event, KeyEventKind, KeyModifiers};
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
    MobileMouseConsume,
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
mod render;
mod selection;
mod state;
mod terminal_ui;
mod text;
mod tools;

const MSG_TOP: usize = 1; // 1-based row index
const MSG_CONTENT_X: usize = 2; // 0-based x

fn usage() {
    eprintln!(
        "Usage:\n  carlos\n  carlos resume [SESSION_ID]\n\nEnv:\n  CARLOS_METRICS=1  enable perf overlay + exit report (toggle: F8 or Ctrl+P)"
    );
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

pub(crate) fn run() -> Result<()> {
    let mut args = env::args();
    let _bin = args.next();

    let cmd = args.next();
    let mut mode_resume = false;
    let mut resume_id: Option<String> = None;

    if let Some(cmd) = cmd {
        if cmd == "resume" {
            mode_resume = true;
            resume_id = args.next();
        } else {
            usage();
            return Ok(());
        }
    }

    let mut client = AppServerClient::start()?;
    initialize_client(&client)?;
    let server_events_rx = client.take_events_rx()?;

    let cwd = env::current_dir()?.to_string_lossy().to_string();

    let (chosen_thread_id, start_resp) = if mode_resume {
        if let Some(rid) = resume_id {
            let resp = client.call(
                "thread/resume",
                params_thread_resume(&rid),
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
    if env_flag_enabled("CARLOS_METRICS") {
        app.enable_perf_metrics();
    }
    load_history_from_start_or_resume(&mut app, &start_resp)?;
    app.set_status("ready");

    let out = run_conversation_tui(&client, &mut app, server_events_rx);
    if let Some(report) = app.perf_report() {
        eprintln!("{report}");
    }
    out
}

#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
