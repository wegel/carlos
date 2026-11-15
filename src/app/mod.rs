//! Launch the terminal client and keep its widgets separate from daemon-owned sessions.

use std::env;

use anyhow::Result;

use self::view_models::{RenderedLine, TerminalSize};

mod child_activity_view;
mod claude_question_field;
mod claude_question_state;
mod claude_question_view;
mod cli;
mod codex_goal_state;
mod codex_goal_text;
mod codex_goal_view;
mod codex_queue_state;
mod codex_queue_view;
mod context_settings;
mod context_usage;
mod daemon_launcher;
mod dictation_setup;
mod dictation_state;
mod flow_transcript;
mod input;
mod input_events;
mod layout_state;
mod markdown_tables;
mod mobile_mouse;
mod mouse_gestures;
mod notifications;
mod overlay_render;
mod perf;
mod perf_bench;
mod perf_session;
mod perf_view;
mod picker_delete_dialog;
mod picker_render;
mod question_editor;
mod question_wire_draft;
mod remote;
mod render;
mod render_cache_state;
mod render_entry_cache;
mod render_input;
mod render_transcript;
mod runtime_settings_state;
mod selection;
mod source_highlight;
mod status_line;
mod style_convert;
mod task_panel_rows;
mod task_panel_view;
mod terminal_ui;
mod text;
mod tool_output_command;
mod tool_output_preferences;
mod tool_output_state;
mod tool_summary;
mod transcript_block;
mod transcript_body;
mod transcript_diff;
mod transcript_diff_source;
mod transcript_input;
mod transcript_render;
mod transcript_source;
mod transcript_spacing;
mod transcript_styles;
mod transcript_wrap;
mod view_dictation;
mod view_dictation_io;
mod view_models;
mod view_state;
mod viewport_state;
mod weekly_usage;

const MSG_TOP: usize = 1;

pub(crate) fn run() -> Result<()> {
    let options = match cli::parse_cli_args(env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("error: {error}");
            cli::usage();
            return Ok(());
        }
    };
    if options.show_help {
        cli::usage();
        return Ok(());
    }
    if options.mode_perf_session {
        return perf_session::run_perf_session(&options);
    }
    if let Some(socket) = &options.daemon_socket {
        return remote::run(socket, &options);
    }
    daemon_launcher::run(&options)
}

#[cfg(test)]
#[path = "../tests.rs"]
mod tests;

#[cfg(test)]
mod local_input_tests;
#[cfg(test)]
mod perf_tests;
