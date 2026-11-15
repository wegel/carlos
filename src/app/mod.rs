//! Application entry point plus shared app-module exports.

use std::env;

use anyhow::Result;

use self::state::AppState;
use self::view_models::{RenderedLine, TerminalSize};

// --- Module Tree ---

mod backend_setup;
mod child_activity_view;
mod claude_activity;
#[cfg(test)]
mod claude_browser;
#[cfg(test)]
mod claude_callbacks;
mod claude_context;
mod claude_edit;
mod claude_edit_view;
mod claude_models;
#[cfg(test)]
mod claude_notices;
#[cfg(test)]
mod claude_notifications;
#[cfg(test)]
mod claude_prompts;
mod claude_question_field;
mod claude_question_input;
mod claude_question_state;
#[cfg(test)]
#[path = "../tests/claude_question_tests.rs"]
mod claude_question_tests;
mod claude_question_view;
mod claude_quota;
#[cfg(test)]
mod claude_reset;
mod claude_rewind;
#[cfg(test)]
#[path = "../tests/claude_sdk_tests.rs"]
mod claude_sdk_tests;
mod claude_session_history;
#[cfg(test)]
#[path = "../tests/claude_session_tests.rs"]
mod claude_session_tests;
#[cfg(test)]
#[path = "../tests/claude_task_tests.rs"]
mod claude_task_tests;
mod claude_task_view;
mod claude_tasks;
#[cfg(test)]
#[path = "../tests/claude_thinking_tests.rs"]
mod claude_thinking_tests;
mod cli;
mod codex_callbacks;
mod codex_goal_state;
mod codex_goal_text;
mod codex_goal_view;
mod codex_goals;
mod codex_lifecycle;
mod codex_panel;
mod codex_panel_state;
mod codex_panel_text;
mod codex_panel_view;
mod codex_questions;
mod codex_queue;
mod codex_queue_receive;
mod codex_queue_state;
mod codex_queue_view;
mod codex_reconnect;
mod codex_reconnect_install;
mod codex_reconnect_runtime;
mod codex_reconnect_streams;
mod codex_reply;
mod codex_settings;
mod codex_settings_draft;
mod codex_state;
mod codex_task_status;
mod context_settings;
mod context_usage;
mod daemon_launcher;
mod dictation_setup;
mod dictation_state;
mod flow_transcript;
mod input;
mod input_events;
mod input_history_state;
mod input_mode;
mod item_history;
mod layout_state;
mod markdown_tables;
mod mobile_mouse;
mod mouse_events;
mod mouse_gestures;
mod native_questions;
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
mod rate_limits;
mod remote;
mod render;
mod render_cache_state;
mod render_entry_cache;
mod render_input;
mod render_status;
mod render_transcript;
mod runtime_settings_state;
mod selection;
mod session_name;
mod source_highlight;
mod state;
mod state_approval_view;
mod state_dictation;
mod state_input;
#[cfg(test)]
mod state_items;
mod state_ralph;
mod state_settings;
mod state_status;
mod state_summaries;
mod state_thread;
mod state_transcript;
mod status_line;
mod style_convert;
mod task_badge;
mod task_panel_rows;
mod task_panel_view;
mod terminal_ui;
mod text;
mod tool_command_view;
mod tool_output_command;
mod tool_output_items;
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
mod turn_approval;
mod turn_interrupt;
mod turn_submit;
mod view_dictation;
mod view_dictation_io;
mod view_models;
mod view_state;
mod viewport_state;
mod weekly_usage;

// --- Shared Types ---

const MSG_TOP: usize = 1;
const MSG_CONTENT_X: usize = 2;

// --- Runtime Wiring ---

use self::backend_setup::{run_claude_backend, run_codex_backend, run_grok_backend};
use self::cli::{
    env_reasoning_summary_override, load_runtime_defaults, parse_cli_args,
    persist_runtime_defaults, usage, Backend, CliOptions,
};

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

    if let Some(socket) = &opts.daemon_socket {
        return remote::run(socket, &opts);
    }
    if opts.daemon {
        return daemon_launcher::run(&opts);
    }
    let cwd_path = env::current_dir()?;
    let cwd = cwd_path.to_string_lossy().to_string();
    let persisted_defaults = load_runtime_defaults();
    let default_summary = env_reasoning_summary_override()
        .or(persisted_defaults.summary.clone())
        .or(Some("concise".to_string()));

    match opts.backend {
        Backend::Codex => {
            run_codex_backend(&opts, &cwd_path, &cwd, persisted_defaults, default_summary)
        }
        Backend::Claude => {
            run_claude_backend(&opts, &cwd_path, &cwd, persisted_defaults, default_summary)
        }
        Backend::Grok => {
            run_grok_backend(&opts, &cwd_path, &cwd, persisted_defaults, default_summary)
        }
    }
}

// --- Test Bridge ---

#[cfg(test)]
#[path = "../tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/claude_native_state_tests.rs"]
mod claude_native_state_tests;
#[cfg(test)]
#[path = "../tests/claude_visibility_tests.rs"]
mod claude_visibility_tests;

#[cfg(test)]
#[path = "../tests/claude_quota_tests.rs"]
mod claude_quota_tests;

#[cfg(test)]
#[path = "../tests/view_hint_sequences.rs"]
mod view_hint_sequences;
#[cfg(test)]
#[path = "../tests/view_hint_support.rs"]
mod view_hint_support;

#[cfg(test)]
#[path = "../tests/view_ownership_tests.rs"]
mod view_ownership_tests;

#[cfg(test)]
#[path = "../tests/approval_lifetime_sequences.rs"]
mod approval_lifetime_sequences;
#[cfg(test)]
#[path = "../tests/approval_lifetime_support.rs"]
mod approval_lifetime_support;
#[cfg(test)]
#[path = "../tests/approval_lifetime_tests.rs"]
mod approval_lifetime_tests;

#[cfg(test)]
#[path = "../tests/approval_operation_sequences.rs"]
mod approval_operation_sequences;

#[cfg(test)]
#[path = "../tests/input_history_split_tests.rs"]
mod input_history_split_tests;

#[cfg(test)]
#[path = "../tests/claude_metadata_parity_tests.rs"]
mod claude_metadata_parity_tests;

#[cfg(test)]
#[path = "../tests/claude_read_parity_tests.rs"]
mod claude_read_parity_tests;

#[cfg(test)]
#[path = "../tests/claude_browser_parity_tests.rs"]
mod claude_browser_parity_tests;
#[cfg(test)]
#[path = "../tests/claude_process_parity_tests.rs"]
mod claude_process_parity_tests;

#[cfg(test)]
#[path = "../tests/claude_task_bounds_tests.rs"]
mod claude_task_bounds_tests;

#[cfg(test)]
#[path = "../tests/claude_edit_parity_support.rs"]
mod claude_edit_parity_support;
#[cfg(test)]
#[path = "../tests/claude_edit_parity_tests.rs"]
mod claude_edit_parity_tests;
#[cfg(test)]
#[path = "../tests/claude_prompt_parity_tests.rs"]
mod claude_prompt_parity_tests;

#[cfg(test)]
#[path = "../tests/claude_fence_parity_tests.rs"]
mod claude_fence_parity_tests;
#[cfg(test)]
#[path = "../tests/claude_history_parity_tests.rs"]
mod claude_history_parity_tests;

mod codex_settings_view;

mod claude_updates;

#[cfg(test)]
#[path = "../tests/idless_item_tests.rs"]
mod idless_item_tests;

mod backend_updates;

#[cfg(test)]
mod event_forwarder_support;

#[cfg(test)]
mod event_forwarder_tests;

#[cfg(test)]
mod event_shutdown_tests;

#[cfg(test)]
#[path = "../tests/claude_process_support.rs"]
mod claude_process_test_support;
#[cfg(test)]
#[path = "../tests/claude_backend_tests/normalized_app.rs"]
mod normalized_test_app;

mod core_events;

#[cfg(test)]
#[path = "../tests/client_recovery_tests.rs"]
mod client_recovery_tests;
