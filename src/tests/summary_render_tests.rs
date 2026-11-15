//! Keep complete AI descriptions and accurate row counts in both transcript layouts.

use carlos_core::session::models::Role;
use carlos_core::session::models::ToolOutputDetails;

use crate::app::layout_state::TranscriptTheme;
use crate::app::perf_view::PerfView;
use crate::app::tool_output_state::ToolOutputMode;
use carlos_core::session::models::{EntryDraft, MessageKind};
use carlos_proto::session::LabelSource;

use super::render_fixture;

const LABEL: &str = "Read café records and 測試 language preferences, then write the combined results into the session archive";

fn generated_output(app: &mut PerfView, failed: bool) {
    app.rows.push(render_fixture::draft(EntryDraft {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        text: "raw\n".repeat(11) + "raw",
        file_path: None,
        tool_output: Some(ToolOutputDetails {
            generated: true,
            summary: format!("run {LABEL}"),
            failed,
        }),
    }));
}

#[test]
fn ai_descriptions_wrap_completely_with_matching_cache_counts_and_one_footer() {
    for phone in [false, true] {
        for failed in [false, true] {
            let mut app = render_fixture::view(Vec::new());
            app.view.layout.active = if phone {
                TranscriptTheme::Flow
            } else {
                TranscriptTheme::Gutter
            };
            app.view.layout.themes.desktop = if phone {
                TranscriptTheme::Flow
            } else {
                TranscriptTheme::Gutter
            };
            generated_output(&mut app, failed);
            let original = app.rows[0].text().to_owned();
            for mode in [
                ToolOutputMode::Compact,
                ToolOutputMode::Full,
                ToolOutputMode::Compact,
            ] {
                app.view.tool_output.mode = mode;
                for width in [8, 9, 12, 20, 40, 80, 160] {
                    assert_label_rows(&mut app, width, failed);
                    assert_eq!(app.rows[0].text(), original);
                }
            }
        }
    }
}

fn assert_label_rows(app: &mut PerfView, width: usize, failed: bool) {
    app.ensure_rendered_lines(width);
    let count = app.rendered_line_count();
    let lines = render_fixture::snapshot_current(app);
    assert_eq!(count, lines.len());
    assert!(lines.iter().all(|line| line.cells <= width));
    let text = lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let compact: String = text.split_whitespace().collect();
    let expected: String = LABEL.split_whitespace().collect();
    assert!(compact.contains(&expected), "width {width}: {text}");
    if width >= 20 {
        assert!(!text.contains('…'));
    }
    if app.view.tool_output.mode == ToolOutputMode::Compact {
        assert!(!text.contains("raw"));
        if width >= 20 {
            assert_eq!(text.matches("12").count(), 1);
        }
    } else {
        assert_eq!(text.matches("raw").count(), 12);
    }
    assert_eq!(text.matches("failed").count(), usize::from(failed));
    if app.view.layout.active.is_flow() {
        assert_eq!(text.matches('$').count(), 1);
    }
}

#[test]
fn ordinary_command_labels_still_use_one_clipped_row() {
    let mut app = render_fixture::view(Vec::new());
    app.view.tool_output.mode = ToolOutputMode::Compact;
    generated_output(&mut app, false);
    app.rows[0]
        .entry
        .tool
        .as_mut()
        .expect("metadata")
        .label_source = LabelSource::Literal;
    app.ensure_rendered_lines(40);
    let lines = render_fixture::snapshot_current(&mut app);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].text.contains('…'));
}
