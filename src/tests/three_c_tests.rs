//! Keep 3c row counts, clipping, and context warnings correct at terminal edges.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::{render_main_view, AppState, Message, MessageKind, Role, TerminalSize};
use crate::app::models::ToolOutputDetails;
use crate::app::mouse_events::handle_mouse_event;
use crate::app::tool_output_state::ToolOutputMode;
use crate::app::tool_summary::summary_line;
use crate::app::transcript_render::{
    build_rendered_block_for_message, count_rendered_block_for_message,
    count_rendered_block_for_message_cached,
};
use crate::app::transcript_styles::RenderCountCache;
use crate::theme::{context_color, COLOR_DIFF_REMOVE, COLOR_GUTTER_USER, COLOR_TOOL_EDIT};

fn failed_read() -> Message {
    Message {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        text: "$ cat source.rs\nlet value = 1;\nexit code: 2".into(),
        file_path: Some("source.rs".into()),
        tool_output: Some(ToolOutputDetails {
            generated: false,
            summary: "→ Read source.rs".into(),
            failed: true,
        }),
    }
}

#[test]
fn failed_read_headers_and_grouped_blocks_keep_both_count_passes_in_sync() {
    let read = failed_read();
    for width in [0, 1, 7, 8, 12, 40, 120] {
        let expected = build_rendered_block_for_message(Some(&read), &read, width).len();
        assert_eq!(
            count_rendered_block_for_message(Some(&read), &read, width),
            expected
        );
        assert_eq!(
            count_rendered_block_for_message_cached(
                &mut RenderCountCache::new(),
                Some(&read),
                &read,
                width
            ),
            expected
        );
        let mut app = AppState::new("thread".into());
        app.messages = vec![read.clone(), read.clone()];
        app.tool_output.mode = ToolOutputMode::Compact;
        app.ensure_rendered_lines(width, None);
        let count = app.rendered_line_count();
        let lines = app.snapshot_rendered_lines();
        assert_eq!(lines.len(), count);
        assert!(lines.iter().all(|line| line.cells <= width));
        assert!(lines.iter().all(|line| !line.separator));
    }
}

#[test]
fn compact_unicode_labels_leave_room_for_failure_and_large_counts() {
    let mut message = failed_read();
    message.tool_output.as_mut().expect("details").summary =
        format!("run {}", "界e\u{301}".repeat(50));
    for failed in [false, true] {
        message.tool_output.as_mut().expect("details").failed = failed;
        for width in [8, 12, 20, 40, 120] {
            let line = summary_line(&message, width, usize::MAX);
            assert!(line.cells <= width, "{width}: {:?}", line.text);
            assert!(!line.soft_wrap_to_next);
            if failed {
                assert!(line.text.contains("failed"));
            }
        }
    }
}

#[test]
fn context_warnings_use_the_actual_fraction_without_overflow_or_rounding() {
    for (used, color) in [
        (70_000, COLOR_GUTTER_USER),
        (70_001, COLOR_TOOL_EDIT),
        (90_000, COLOR_TOOL_EDIT),
        (90_001, COLOR_DIFF_REMOVE),
    ] {
        assert_eq!(context_color(used, 100_000), color);
    }
    assert_eq!(context_color(0, 0), COLOR_GUTTER_USER);
    assert_eq!(context_color(u64::MAX, u64::MAX), COLOR_DIFF_REMOVE);
}

#[test]
fn mouse_selection_uses_the_same_gutter_as_the_terminal_after_resize() {
    let mut app = AppState::new("thread".into());
    app.append_message(Role::User, "abcdefghij");
    for (width, height, content_x) in [(100, 32, 2), (54, 32, 1), (100, 18, 1), (100, 32, 2)] {
        let size = TerminalSize { width, height };
        let mut terminal =
            Terminal::new(TestBackend::new(width as u16, height as u16)).expect("terminal");
        terminal
            .draw(|frame| render_main_view(frame, &mut app))
            .expect("draw after resize");
        let row = u16::from(app.layout.active.is_flow());
        for (kind, column) in [
            (MouseEventKind::Down(MouseButton::Left), content_x + 2),
            (MouseEventKind::Drag(MouseButton::Left), content_x + 4),
        ] {
            handle_mouse_event(
                &mut app,
                MouseEvent {
                    kind,
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                },
                size,
            );
        }
        let selection = app.viewport.selection.expect("drag selects text");
        assert_eq!(app.selected_text(selection), "cde");
    }
}
