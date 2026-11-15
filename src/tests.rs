//! Top-level test module aggregator: re-exports the per-subsystem test trees.

#[path = "tests/markdown_table_tests.rs"]
mod markdown_table_tests;

pub(super) use super::cli::{
    load_runtime_defaults_from, parse_cli_args, persist_runtime_defaults_to,
    resolve_initial_runtime_settings, resume_hint, styled_resume_hint, Backend,
};
pub(super) use super::notifications::{handle_notification_line, handle_server_message_line};
pub(super) use super::perf_session;
pub(super) use super::picker_delete_dialog::draw_picker_delete_dialog;
pub(super) use super::render::draw_rendered_line;
pub(super) use super::render_input::input_cursor_visual_position;
pub(super) use super::selection::{
    compute_selection_range, selected_text, shift_selection_focus, Selection,
};
pub(super) use super::state::AppState;
pub(super) use super::terminal_ui::sort_threads_for_picker;
pub(super) use super::text::{
    slice_by_cells, visual_width, wrap_input_line, wrap_input_line_count, wrap_natural_by_cells,
    wrap_natural_count_by_cells,
};
pub(super) use super::transcript_render::{
    build_rendered_block_for_message, build_rendered_lines, build_rendered_lines_with_hidden,
    count_rendered_block_for_message, normalize_styled_segments_for_part,
};
pub(super) use super::view_models::{RenderedLine, StyledSegment, TerminalSize};
pub(super) use crate::clipboard::{
    detect_osc52_wrap, is_ssh_session, osc52_sequences_for_env, Osc52Wrap,
};
pub(super) use crate::theme::{COLOR_DIFF_ADD, COLOR_DIFF_REMOVE, COLOR_PRIMARY, COLOR_TEXT};
pub(super) use carlos_core::protocol_params::{
    params_thread_archive, params_turn_interrupt, params_turn_start, parse_thread_runtime_settings,
};
pub(super) use carlos_core::session::actions::ServerRequestAction;
pub(super) use carlos_core::session::models::Message;
pub(super) use carlos_core::session::tools::{command_execution_diff_output, extract_diff_blocks};
pub(super) use ratatui::buffer::Buffer;
pub(super) use ratatui::layout::Rect;
pub(super) use ratatui::style::Style;
pub(super) use serde_json::json;

#[path = "tests/claude_context_tests.rs"]
mod claude_context_tests;
#[path = "tests/claude_edit_tests.rs"]
mod claude_edit_tests;
#[path = "tests/daemon_resume_hint_tests.rs"]
mod daemon_resume_hint_tests;

#[path = "tests/codex_order_tests.rs"]
mod codex_order_tests;
#[path = "tests/codex_question_support.rs"]
mod codex_question_support;
#[path = "tests/codex_question_tests.rs"]
mod codex_question_tests;
#[path = "tests/codex_queue_tests.rs"]
mod codex_queue_tests;
#[path = "tests/grouped_render_tests.rs"]
mod grouped_render_tests;
#[path = "tests/late_summary_render_tests.rs"]
mod late_summary_render_tests;
#[path = "tests/notification_render_tests.rs"]
mod notification_render_tests;
#[path = "tests/notification_tests.rs"]
mod notification_tests;
#[path = "tests/phone_layout_tests.rs"]
mod phone_layout_tests;
#[path = "tests/render_fixture.rs"]
pub(super) mod render_fixture;
#[path = "tests/runtime_tests.rs"]
mod runtime_tests;
#[path = "tests/shell_command_tests.rs"]
mod shell_command_tests;
#[path = "tests/steer_render_tests.rs"]
mod steer_render_tests;

#[path = "tests/summary_render_tests.rs"]
mod summary_render_tests;

#[path = "tests/theme_preferences_tests.rs"]
mod theme_preferences_tests;
#[path = "tests/three_c_tests.rs"]
mod three_c_tests;
#[path = "tests/tool_output_tests.rs"]
mod tool_output_tests;
#[path = "tests/tool_result_render_tests.rs"]
mod tool_result_render_tests;
#[path = "tests/transcript_spacing_tests.rs"]
mod transcript_spacing_tests;
#[path = "tests/ui_render_tests.rs"]
mod ui_render_tests;

#[path = "tests/codex_panel_tests.rs"]
mod codex_panel_tests;

#[path = "tests/codex_panel_read_tests.rs"]
mod codex_panel_read_tests;

#[path = "tests/codex_goal_tests.rs"]
mod codex_goal_tests;

#[path = "tests/codex_goal_lifecycle_tests.rs"]
mod codex_goal_lifecycle_tests;

#[path = "tests/codex_reconnect_support.rs"]
mod codex_reconnect_support;
#[path = "tests/codex_reconnect_tests.rs"]
mod codex_reconnect_tests;

#[path = "tests/codex_reconnect_identity_tests.rs"]
mod codex_reconnect_identity_tests;

#[path = "tests/shell_directory_tests.rs"]
mod shell_directory_tests;

/// Construct test rows through the same allocator used by production appends.
fn test_message(draft: carlos_core::session::models::EntryDraft) -> Message {
    let mut transcript = carlos_core::session::transcript::Transcript::new();
    let id = transcript.append(draft);
    transcript.entry(id).expect("adopted test row").clone()
}

#[path = "tests/review_label_tests.rs"]
mod review_label_tests;

#[path = "tests/codex_panel_support.rs"]
mod codex_panel_support;

pub(super) use carlos_core::session::thread_list::parse_thread_list;
