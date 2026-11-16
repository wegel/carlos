//! Keep 3c row counts, clipping, and context warnings correct at terminal edges.

use carlos_core::session::models::Message;
use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;
use carlos_core::session::models::ToolOutputDetails;

use super::render_fixture;
use crate::app::tool_output_state::ToolOutputMode;
use crate::app::tool_summary::summary_line;
use crate::app::transcript_render::{
    build_rendered_block_for_message, count_rendered_block_for_message,
    count_rendered_block_for_message_cached,
};
use crate::app::transcript_styles::RenderCountCache;
use crate::app::view_models::{DisplayKind, DisplayMessage};
use crate::theme::{context_color, COLOR_DIFF_REMOVE, COLOR_GUTTER_USER, COLOR_TOOL_EDIT};

fn failed_read() -> Message {
    crate::app::tests::test_message(carlos_core::session::models::EntryDraft {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        text: "$ cat source.rs\nlet value = 1;\nexit code: 2".into(),
        file_path: Some("source.rs".into()),
        tool_facts: None,
        tool_output: Some(ToolOutputDetails {
            generated: false,
            summary: "→ Read source.rs".into(),
            failed: true,
        }),
    })
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
        let mut app = render_fixture::view(
            (0..2)
                .map(|_| {
                    render_fixture::draft(carlos_core::session::models::EntryDraft {
                        role: read.role,
                        kind: read.kind,
                        text: read.text.clone(),
                        file_path: read.file_path.clone(),
                        tool_facts: read.tool_facts.clone(),
                        tool_output: read.tool_output.clone(),
                    })
                })
                .collect(),
        );
        app.view.tool_output.mode = ToolOutputMode::Compact;
        app.ensure_rendered_lines(width);
        let count = app.rendered_line_count();
        let lines = render_fixture::snapshot_current(&mut app);
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
            let line = summary_line(
                DisplayMessage {
                    source: (&message).into(),
                    kind: DisplayKind::ToolSummary { rows: usize::MAX },
                },
                width,
            );
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
