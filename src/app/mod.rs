//! Application entry point plus shared app-module exports.

use std::env;

use anyhow::Result;

pub(crate) use self::models::Role;
use self::models::{DiffBlock, Message, MessageKind, RenderedLine, TerminalSize, ThreadSummary};
use self::state::AppState;

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
