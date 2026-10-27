//! UI rendering test tree: drawing, wrapping, markdown/diff, selection, transcript.

use super::{
    build_rendered_block_for_message, build_rendered_lines, build_rendered_lines_with_hidden,
    command_execution_diff_output, compute_input_layout, compute_selection_range,
    count_rendered_block_for_message, draw_picker_delete_dialog, draw_rendered_line,
    extract_diff_blocks, input_cursor_visual_position, json, normalize_styled_segments_for_part,
    params_turn_interrupt, ralph, render_main_view, selected_text, shift_selection_focus,
    slice_by_cells, visual_width, wrap_input_line, wrap_input_line_count, wrap_natural_by_cells,
    wrap_natural_count_by_cells, AppState, Buffer, DictationProfileState, Message, MessageKind,
    Rect, RenderedLine, Role, Selection, Style, StyledSegment, TerminalSize, ThreadSummary,
    COLOR_DIFF_ADD, COLOR_DIFF_REMOVE, COLOR_PRIMARY, COLOR_TEXT,
};

fn usable_dictation_profile() -> DictationProfileState {
    DictationProfileState {
        id: "en".to_string(),
        name: "English".to_string(),
        model_label: Some("/tmp/model.bin".to_string()),
        model_usable: true,
        #[cfg(feature = "dictation")]
        model_path: Some(std::path::PathBuf::from("/tmp/model.bin")),
        #[cfg(feature = "dictation")]
        language: Some("en".to_string()),
        #[cfg(feature = "dictation")]
        vocabulary: None,
    }
}

fn rendered_buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    buffer.content().iter().map(|cell| cell.symbol()).collect()
}

fn render_app_text(app: &mut AppState) -> String {
    let backend = ratatui::backend::TestBackend::new(120, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| render_main_view(frame, app))
        .expect("draw");
    rendered_buffer_text(terminal.backend().buffer())
}

include!("ui_render_tests/selection_and_history.rs");

include!("ui_render_tests/transcript_and_input.rs");

include!("ui_render_tests/markdown_and_diff.rs");

include!("ui_render_tests/drawing_and_wrapping.rs");

#[path = "ui_render_tests/source_highlight.rs"]
mod source_highlight;
