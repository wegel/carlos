//! Keep complete AI descriptions and accurate row counts in both transcript layouts.

use crate::app::layout_state::LayoutMode;
use crate::app::models::{Role, ToolOutputDetails};
use crate::app::tool_output_state::ToolOutputMode;
use crate::app::AppState;

const LABEL: &str = "Read café records and 測試 language preferences, then write the combined results into the session archive";

fn generated_output(app: &mut AppState, failed: bool) {
    let index = app.append_message(Role::ToolOutput, "raw\n".repeat(11) + "raw");
    app.messages[index].tool_output = Some(ToolOutputDetails {
        generated: true,
        summary: format!("run {LABEL}"),
        failed,
    });
}

#[test]
fn ai_descriptions_wrap_completely_with_matching_cache_counts_and_one_footer() {
    for phone in [false, true] {
        for failed in [false, true] {
            let mut app = AppState::new("thread".into());
            app.layout.phone = phone;
            app.layout.mode = if phone {
                LayoutMode::Phone
            } else {
                LayoutMode::Desktop
            };
            generated_output(&mut app, failed);
            let original = app.messages[0].text.clone();
            for mode in [
                ToolOutputMode::Compact,
                ToolOutputMode::Full,
                ToolOutputMode::Compact,
            ] {
                app.tool_output.mode = mode;
                for width in [8, 9, 12, 20, 40, 80, 160] {
                    assert_label_rows(&mut app, width, failed);
                    assert_eq!(app.messages[0].text, original);
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
    if app.tool_output.mode == ToolOutputMode::Compact {
        assert!(!text.contains("raw"));
        if width >= 20 {
            assert_eq!(text.matches("12").count(), 1);
        }
    } else {
        assert_eq!(text.matches("raw").count(), 12);
    }
    assert_eq!(text.matches("failed").count(), usize::from(failed));
    if app.layout.phone {
        assert_eq!(text.matches('$').count(), 1);
    }
}

#[test]
fn ordinary_command_labels_still_use_one_clipped_row() {
    let mut app = AppState::new("thread".into());
    app.tool_output.mode = ToolOutputMode::Compact;
    generated_output(&mut app, false);
    app.messages[0]
        .tool_output
        .as_mut()
        .expect("metadata")
        .generated = false;
    app.ensure_rendered_lines(40, None);
    let lines = app.snapshot_rendered_lines();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].text.contains('…'));
}
