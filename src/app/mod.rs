//! Application entry point plus shared app-module exports.

use std::env;

use anyhow::Result;

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
#[cfg(test)]
use self::dictation_state::{DictationPhase, DictationProfileState};
#[cfg(test)]
use self::input::{
    can_submit_queued_turn, is_mobile_mouse_key_candidate, is_priority_server_line,
    prioritize_events,
};
#[cfg(test)]
use self::input_events::handle_terminal_event;
#[cfg(test)]
use self::mobile_mouse::{
    apply_mobile_mouse_scroll, consume_mobile_mouse_char, parse_mobile_mouse_coords,
    parse_repeated_plain_mobile_pair, MobileMouseConsume,
};
pub(crate) use self::models::Role;
#[cfg(test)]
use self::models::StyledSegment;
use self::models::{DiffBlock, Message, MessageKind, RenderedLine, TerminalSize, ThreadSummary};
#[cfg(test)]
use self::notifications::{
    append_history_from_thread, handle_notification_line, handle_server_message_line,
    is_key_press_like, kitt_head_index, load_history_from_start_or_resume, parse_thread_list,
    ServerRequestAction,
};
#[cfg(test)]
use self::perf::{DurationSamples, PerfMetrics};
#[cfg(test)]
use self::picker_delete_dialog::draw_picker_delete_dialog;
#[cfg(test)]
use self::render::{
    compute_input_layout, draw_rendered_line, is_newline_enter, normalize_pasted_text,
    render_main_view,
};
#[cfg(test)]
use self::selection::{
    compute_selection_range, decide_mouse_drag_mode, selected_text, MouseDragMode, Selection,
};
use self::state::AppState;
#[cfg(test)]
use self::terminal_ui::sort_threads_for_picker;
#[cfg(test)]
use self::text::{
    slice_by_cells, visual_width, wrap_input_line, wrap_input_line_count, wrap_natural_by_cells,
    wrap_natural_count_by_cells,
};
#[cfg(test)]
use self::tool_shell::parse_ssh_remote_command;
#[cfg(test)]
use self::tools::{
    command_execution_diff_output, extract_diff_blocks, format_tool_item,
    strip_terminal_controls, strip_terminal_controls_preserving_sgr,
};
#[cfg(test)]
use self::transcript_render::{
    build_rendered_block_for_message, build_rendered_lines, build_rendered_lines_with_hidden,
    count_rendered_block_for_message, normalize_styled_segments_for_part,
};
#[cfg(test)]
use crate::clipboard::{detect_osc52_wrap, is_ssh_session, osc52_sequences_for_env, Osc52Wrap};
#[cfg(test)]
use crate::event::UiEvent;
#[cfg(test)]
use crate::protocol_params::{
    params_thread_archive, params_turn_interrupt, params_turn_start, parse_thread_runtime_settings,
};
#[cfg(test)]
use crate::theme::{COLOR_DIFF_ADD, COLOR_DIFF_REMOVE, COLOR_PRIMARY, COLOR_STEP2, COLOR_TEXT};

// --- Module Tree ---
mod approval_parsing;
mod approval_state;
mod backend_setup;
mod cli;
mod context_usage;
mod dictation_state;
mod input;
mod input_events;
mod input_history_state;
mod item_history;
mod mobile_mouse;
mod models;
mod mouse_events;
mod notification_items;
mod notifications;
mod overlay_render;
mod perf;
mod perf_bench;
mod perf_session;
mod perf_synthetic;
mod picker_delete_dialog;
mod picker_render;
mod ralph;
mod ralph_runtime_state;
mod render;
mod render_cache_state;
mod render_input;
mod runtime_settings_state;
mod selection;
mod state;
mod state_dictation;
mod state_input;
mod state_settings;
mod state_transcript;
mod style_convert;
mod terminal_ui;
mod text;
mod tool_diff;
mod tool_format;
mod tool_shell;
mod tools;
mod transcript_diff;
mod transcript_render;
mod transcript_styles;
mod transcript_wrap;
mod turn_submit;
mod viewport_state;

// --- Shared Types ---
const MSG_TOP: usize = 1;
const MSG_CONTENT_X: usize = 2;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeDefaults {
    pub(super) model: Option<String>,
    pub(super) effort: Option<String>,
    pub(super) summary: Option<String>,
}

// --- Runtime Wiring ---
use self::backend_setup::{run_claude_backend, run_codex_backend};
use self::cli::{
    env_reasoning_summary_override, load_runtime_defaults, parse_cli_args, persist_runtime_defaults,
    usage, Backend, CliOptions,
};
#[cfg(test)]
use self::backend_setup::load_claude_thread_summaries;
#[cfg(test)]
use self::cli::{
    load_runtime_defaults_from, persist_runtime_defaults_to, resolve_initial_runtime_settings,
    resume_hint, styled_resume_hint,
};
#[cfg(test)]
use self::render_input::input_cursor_visual_position;

// --- App Entry ---
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

// --- Test Bridge ---
#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
