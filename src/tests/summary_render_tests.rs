//! Keep complete AI descriptions and accurate row counts in both transcript layouts.

use crate::app::layout_state::TranscriptTheme;
use crate::app::models::{Role, ToolOutputDetails};
use crate::app::tool_output_state::ToolOutputMode;
use crate::app::AppState;

const LABEL: &str = "Read café records and 測試 language preferences, then write the combined results into the session archive";

fn generated_output(app: &mut AppState, failed: bool) {
    let index = app.append_message(Role::ToolOutput, "raw\n".repeat(11) + "raw");
    app.session.transcript_set_tool(
        index,
        ToolOutputDetails {
            generated: true,
            summary: format!("run {LABEL}"),
            failed,
        },
    );
}

#[test]
fn ai_descriptions_wrap_completely_with_matching_cache_counts_and_one_footer() {
    for phone in [false, true] {
        for failed in [false, true] {
            let mut app = AppState::new("thread".into());
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
            let original = app.session.messages()[0].text.clone();
            for mode in [
                ToolOutputMode::Compact,
                ToolOutputMode::Full,
                ToolOutputMode::Compact,
            ] {
                app.view.tool_output.mode = mode;
                for width in [8, 9, 12, 20, 40, 80, 160] {
                    assert_label_rows(&mut app, width, failed);
                    assert_eq!(app.session.messages()[0].text, original);
                }
            }
        }
    }
}

fn assert_label_rows(app: &mut AppState, width: usize, failed: bool) {
    app.ensure_rendered_lines(width, None);
    let count = app.rendered_line_count();
    let lines = app.snapshot_rendered_lines();
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
    let mut app = AppState::new("thread".into());
    app.view.tool_output.mode = ToolOutputMode::Compact;
    generated_output(&mut app, false);
    let mut details = app.session.messages()[0]
        .tool_output
        .clone()
        .expect("metadata");
    details.generated = false;
    app.session.transcript_set_tool(0, details);
    app.ensure_rendered_lines(40, None);
    let lines = app.snapshot_rendered_lines();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].text.contains('…'));
}
