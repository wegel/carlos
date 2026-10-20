//! Top-level test module aggregator: re-exports the per-subsystem test trees.

pub(super) use super::backend_setup::{
    claude_runtime_defaults, codex_runtime_defaults, complete_codex_model_catalog,
    load_claude_thread_summaries, resolve_backend_runtime_settings,
};
pub(super) use super::cli::{
    load_runtime_defaults_from, parse_cli_args, persist_runtime_defaults_to,
    resolve_initial_runtime_settings, resume_hint, styled_resume_hint, Backend,
};
pub(super) use super::codex_history::{parse_rollout_history, prefer_complete_rollout_history};
pub(super) use super::context_usage::{
    context_label_reserved_cells, context_usage_label, ContextUsage,
};
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
pub(super) use super::models::{
    Message, MessageKind, RenderedLine, Role, StyledSegment, TerminalSize, ThreadSummary,
};
pub(super) use super::notifications::{
    append_history_from_thread, handle_notification_line, handle_server_message_line,
    is_key_press_like, kitt_head_index, load_history_from_start_or_resume, parse_thread_list,
    ServerRequestAction,
};
pub(super) use super::perf::{DurationSamples, PerfMetrics};
pub(super) use super::picker_delete_dialog::draw_picker_delete_dialog;
pub(super) use super::render::{
    compute_input_layout, draw_rendered_line, is_newline_enter, normalize_pasted_text,
    render_main_view,
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
pub(super) use super::tool_shell::parse_ssh_remote_command;
pub(super) use super::tools::{
    command_execution_diff_output, extract_diff_blocks, format_tool_item, strip_terminal_controls,
    strip_terminal_controls_preserving_sgr,
};
pub(super) use super::transcript_render::{
    build_rendered_block_for_message, build_rendered_lines, build_rendered_lines_with_hidden,
    count_rendered_block_for_message, normalize_styled_segments_for_part,
};
pub(super) use super::turn_submit::{
    submit_dequeued_turn_text, submit_turn_text, submit_turn_text_with_history,
};
pub(super) use super::{
    input_events, perf_session, ralph, state, turn_submit, RuntimeDefaults, MSG_CONTENT_X,
};
pub(super) use crate::clipboard::{
    detect_osc52_wrap, is_ssh_session, osc52_sequences_for_env, Osc52Wrap,
};
pub(super) use crate::event::UiEvent;
pub(super) use crate::protocol_params::{
    params_thread_archive, params_turn_interrupt, params_turn_start, parse_thread_runtime_settings,
    ModelInfo, ThreadRuntimeSettings,
};
pub(super) use crate::theme::{
    COLOR_DIFF_ADD, COLOR_DIFF_REMOVE, COLOR_PRIMARY, COLOR_STEP2, COLOR_TEXT,
};
pub(super) use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
pub(super) use ratatui::buffer::Buffer;
pub(super) use ratatui::layout::Rect;
pub(super) use ratatui::style::Style;
pub(super) use serde_json::json;

#[path = "tests/claude_backend_tests.rs"]
mod claude_backend_tests;
#[path = "tests/codex_history_tests.rs"]
mod codex_history_tests;
#[path = "tests/grok_backend_tests.rs"]
mod grok_backend_tests;
#[path = "tests/input_tests.rs"]
mod input_tests;
#[path = "tests/notification_tests.rs"]
mod notification_tests;
#[path = "tests/runtime_tests.rs"]
mod runtime_tests;
#[path = "tests/tool_tests.rs"]
mod tool_tests;
#[path = "tests/turn_lifecycle_tests.rs"]
mod turn_lifecycle_tests;
#[path = "tests/ui_render_tests.rs"]
mod ui_render_tests;
