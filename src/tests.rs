//! Top-level test module aggregator: re-exports the per-subsystem test trees.

#[path = "tests/markdown_table_tests.rs"]
mod markdown_table_tests;

pub(super) use super::backend_setup::{
    claude_runtime_defaults, codex_runtime_defaults, complete_codex_model_catalog,
    resolve_backend_runtime_settings,
};
pub(super) use super::cli::{
    load_runtime_defaults_from, parse_cli_args, persist_runtime_defaults_to,
    resolve_initial_runtime_settings, resume_hint, styled_resume_hint, Backend,
};
pub(super) use super::context_usage::context_usage_label;
pub(super) use super::dictation_state::{DictationPhase, DictationProfileState};
pub(super) use super::input::{
    can_submit_queued_turn, is_mobile_mouse_key_candidate, is_priority_server_line,
    prioritize_events,
};
pub(super) use super::input_events::handle_terminal_event;
pub(super) use super::mobile_mouse::{
    apply_mobile_mouse_scroll, consume_mobile_mouse_char, parse_mobile_mouse_coords,
    parse_repeated_plain_mobile_pair, MobileMouseConsume,
};
pub(super) use super::notifications::{
    handle_notification_line, handle_server_message_line, is_key_press_like, kitt_head_index,
};
pub(super) use super::perf::{DurationSamples, PerfMetrics};
pub(super) use super::picker_delete_dialog::draw_picker_delete_dialog;
pub(super) use super::render::{
    draw_rendered_line, is_newline_enter, normalize_pasted_text, render_main_view,
};
pub(super) use super::render_input::input_cursor_visual_position;
pub(super) use super::selection::{
    compute_selection_range, decide_mouse_drag_mode, selected_text, shift_selection_focus,
    MouseDragMode, Selection,
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
pub(super) use super::turn_submit::{
    submit_dequeued_turn_text, submit_turn_text, submit_turn_text_with_history,
};
pub(super) use super::view_models::{RenderedLine, StyledSegment, TerminalSize};
pub(super) use super::{input_events, perf_session, state, turn_submit};
pub(super) use crate::clipboard::{
    detect_osc52_wrap, is_ssh_session, osc52_sequences_for_env, Osc52Wrap,
};
pub(super) use crate::event::UiEvent;
pub(super) use crate::theme::{COLOR_DIFF_ADD, COLOR_DIFF_REMOVE, COLOR_PRIMARY, COLOR_TEXT};
pub(super) use carlos_core::protocol_params::{
    params_thread_archive, params_turn_interrupt, params_turn_start, parse_thread_runtime_settings,
    ModelInfo, ThreadRuntimeSettings,
};
pub(super) use carlos_core::session::actions::ServerRequestAction;
pub(super) use carlos_core::session::codex::history::{
    parse_rollout_history, prefer_complete_rollout_history,
};
pub(super) use carlos_core::session::models::Message;
pub(super) use carlos_core::session::tool_shell::parse_ssh_remote_command;
pub(super) use carlos_core::session::tools::{
    command_execution_diff_output, extract_diff_blocks, format_tool_item, strip_terminal_controls,
    strip_terminal_controls_preserving_sgr,
};
pub(super) use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
pub(super) use ratatui::buffer::Buffer;
pub(super) use ratatui::layout::Rect;
pub(super) use ratatui::style::Style;
pub(super) use serde_json::json;

#[path = "tests/claude_backend_tests.rs"]
mod claude_backend_tests;
#[path = "tests/claude_context_tests.rs"]
mod claude_context_tests;
#[path = "tests/claude_edit_tests.rs"]
mod claude_edit_tests;
#[path = "tests/codex_history_tests.rs"]
mod codex_history_tests;
#[path = "tests/codex_order_tests.rs"]
mod codex_order_tests;
#[path = "tests/codex_question_support.rs"]
mod codex_question_support;
#[path = "tests/codex_question_tests.rs"]
mod codex_question_tests;
#[path = "tests/codex_queue_tests.rs"]
mod codex_queue_tests;
#[path = "tests/codex_settings_tests.rs"]
mod codex_settings_tests;
#[path = "tests/codex_startup_settings_tests.rs"]
mod codex_startup_settings_tests;
#[path = "tests/grok_backend_tests.rs"]
mod grok_backend_tests;
#[path = "tests/input_tests.rs"]
mod input_tests;
#[path = "tests/native_activity_tests.rs"]
mod native_activity_tests;
#[path = "tests/notification_tests.rs"]
mod notification_tests;
#[path = "tests/phone_layout_tests.rs"]
mod phone_layout_tests;
#[path = "tests/render_fixture.rs"]
pub(super) mod render_fixture;
#[path = "tests/rtk_transcript_tests.rs"]
mod rtk_transcript_tests;
#[path = "tests/runtime_tests.rs"]
mod runtime_tests;
#[path = "tests/session_name_tests.rs"]
mod session_name_tests;
#[path = "tests/shell_command_tests.rs"]
mod shell_command_tests;
#[path = "tests/summary_render_tests.rs"]
mod summary_render_tests;
#[path = "tests/summary_tests.rs"]
mod summary_tests;
#[path = "tests/theme_preferences_tests.rs"]
mod theme_preferences_tests;
#[path = "tests/three_c_tests.rs"]
mod three_c_tests;
#[path = "tests/tool_output_item_tests.rs"]
mod tool_output_item_tests;
#[path = "tests/tool_output_tests.rs"]
mod tool_output_tests;
#[path = "tests/tool_tests.rs"]
mod tool_tests;
#[path = "tests/transcript_spacing_tests.rs"]
mod transcript_spacing_tests;
#[path = "tests/turn_lifecycle_tests.rs"]
mod turn_lifecycle_tests;
#[path = "tests/ui_render_tests.rs"]
mod ui_render_tests;

#[path = "tests/codex_steer_tests.rs"]
mod codex_steer_tests;

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

#[path = "tests/codex_task_status_tests.rs"]
mod codex_task_status_tests;
#[path = "tests/input_status_tests.rs"]
mod input_status_tests;

#[path = "tests/codex_task_ownership_tests.rs"]
mod codex_task_ownership_tests;

#[path = "tests/shell_directory_tests.rs"]
mod shell_directory_tests;

#[path = "tests/tool_command_view_tests.rs"]
mod tool_command_view_tests;

/// Construct test rows through the same allocator used by production appends.
fn test_message(draft: carlos_core::session::models::EntryDraft) -> Message {
    let mut transcript = carlos_core::session::transcript::Transcript::new();
    let id = transcript.append(draft);
    transcript.entry(id).expect("adopted test row").clone()
}

#[path = "tests/approval_identity_tests.rs"]
mod approval_identity_tests;
#[path = "tests/entry_history_identity_tests.rs"]
mod entry_history_identity_tests;
#[path = "tests/entry_identity_tests.rs"]
mod entry_identity_tests;
#[path = "tests/entry_lookup_differential_tests.rs"]
mod entry_lookup_differential_tests;
#[path = "tests/history_conversation_tests.rs"]
mod history_conversation_tests;
#[path = "tests/item_source_backend_tests.rs"]
mod item_source_backend_tests;

#[path = "tests/entry_review_cursor_tests.rs"]
mod entry_review_cursor_tests;
#[path = "tests/entry_review_history_tests.rs"]
mod entry_review_history_tests;

#[path = "tests/head_entry_reference/mod.rs"]
mod head_entry_reference;

#[path = "tests/entry_lookup_extra_tests.rs"]
mod entry_lookup_extra_tests;

#[path = "tests/codex_panel_support.rs"]
mod codex_panel_support;

pub(super) use super::item_history::{
    append_history_from_thread, load_history_from_start_or_resume,
};
pub(super) use carlos_core::session::thread_list::parse_thread_list;
