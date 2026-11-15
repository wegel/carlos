//! UI rendering test tree: drawing, wrapping, markdown/diff, selection, transcript.

use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;
use carlos_core::session::models::ThreadSummary;

use super::{
    build_rendered_block_for_message, build_rendered_lines, build_rendered_lines_with_hidden,
    command_execution_diff_output, compute_selection_range, count_rendered_block_for_message,
    draw_picker_delete_dialog, draw_rendered_line, extract_diff_blocks,
    input_cursor_visual_position, json, normalize_styled_segments_for_part, params_turn_interrupt,
    selected_text, shift_selection_focus, slice_by_cells, visual_width, wrap_input_line,
    wrap_input_line_count, wrap_natural_by_cells, wrap_natural_count_by_cells, AppState, Buffer,
    Rect, RenderedLine, Selection, Style, StyledSegment, TerminalSize, COLOR_DIFF_ADD,
    COLOR_DIFF_REMOVE, COLOR_PRIMARY, COLOR_TEXT,
};

use super::render_fixture;

include!("ui_render_tests/selection_and_history.rs");

include!("ui_render_tests/transcript_and_input.rs");

include!("ui_render_tests/markdown_and_diff.rs");

include!("ui_render_tests/drawing_and_wrapping.rs");

#[path = "ui_render_tests/source_highlight.rs"]
mod source_highlight;
