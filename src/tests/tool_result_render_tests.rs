//! Paint compact and full tool sources without a mutable native session.

use carlos_core::session::models::{EntryDraft, MessageKind, Role, ToolOutputDetails};

use super::render_fixture;
use crate::app::perf_view::PerfView;
use crate::app::tool_output_state::ToolOutputMode;
use crate::app::view_models::RenderedLine;

fn tool(text: String, label: &str, file: Option<&str>, failed: bool) -> PerfView {
    render_fixture::view(vec![render_fixture::draft(EntryDraft {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        text,
        file_path: file.map(str::to_string),
        tool_facts: None,
        tool_output: Some(ToolOutputDetails {
            summary: label.into(),
            generated: false,
            failed,
        }),
    })])
}

fn paint(view: &mut PerfView, mode: ToolOutputMode, width: usize) -> Vec<RenderedLine> {
    view.view.tool_output.mode = mode;
    render_fixture::snapshot(view, width)
}

fn count_is_visible(lines: &[RenderedLine]) -> bool {
    lines.iter().any(|line| {
        line.text
            .split_whitespace()
            .last()
            .is_some_and(|n| n.parse::<usize>().is_ok())
    })
}

#[test]
fn read_output_modes_keep_the_line_count_and_last_source_row() {
    let body = (0..10)
        .map(|i| format!("let source_{i} = {i};"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut view = tool(
        format!("$ cat main.rs\n{body}\nexit code: 0"),
        "→ Read main.rs",
        Some("main.rs"),
        false,
    );
    assert!(count_is_visible(&paint(
        &mut view,
        ToolOutputMode::Compact,
        50
    )));
    assert!(paint(&mut view, ToolOutputMode::Full, 50)
        .iter()
        .any(|line| line.text.contains("source_9")));
}

#[test]
fn compact_streams_count_every_append_and_failures_keep_the_original_full_body() {
    let body = (0..10)
        .map(|i| format!("let source_{i} = {i};"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut view = tool(String::new(), "→ Read main.rs", Some("main.rs"), false);
    let mut text = String::new();
    for delta in [body.as_str(), "\nlast streamed line"] {
        text.push_str(delta);
        render_fixture::replace(&mut view, 0, text.clone());
        assert!(count_is_visible(&paint(
            &mut view,
            ToolOutputMode::Compact,
            50
        )));
    }
    view.rows[0].entry.tool.as_mut().expect("details").status =
        carlos_proto::session::ToolStatus::Failed;
    render_fixture::replace(
        &mut view,
        0,
        format!("$ cat main.rs\n{body}\ncommand failed\nexit code: 2"),
    );
    let compact = paint(&mut view, ToolOutputMode::Compact, 50);
    assert!(compact.iter().any(|line| line.text.contains("failed")));
    assert!(compact
        .iter()
        .all(|line| !line.text.contains("command failed")));
    assert!(paint(&mut view, ToolOutputMode::Full, 50)
        .iter()
        .any(|line| line.text.contains("command failed")));
}

#[test]
fn compact_diff_results_keep_counts_for_success_and_explicit_failure() {
    let diff = "diff --git a/main.rs b/main.rs\n--- a/main.rs\n+++ b/main.rs\n@@ -1,4 +1,4 @@\n-a\n-b\n-c\n-d\n+A\n+B\n+C\n+D";
    for failed in [false, true] {
        let mut view = tool(diff.into(), "Δ Diff", None, failed);
        view.rows[0].entry.format = carlos_proto::session::EntryFormat::Diff;
        assert!(count_is_visible(&paint(
            &mut view,
            ToolOutputMode::Compact,
            80
        )));
    }
}

#[test]
fn rtk_reads_keep_the_original_command_source_colors_and_number_gutter() {
    let mut view = tool("$ rtk --verbose proxy --skip-env -- rtk read -n 'src/main file.rs'\n1 │ fn main() {\n2 │     let value = \"hello\";\n3 │ }\nexit code: 0".into(),
        "→ Read src/main file.rs", Some("src/main file.rs"), false);
    let lines = paint(&mut view, ToolOutputMode::Full, 100);
    assert!(lines[0]
        .text
        .starts_with("$ rtk --verbose proxy --skip-env -- rtk read"));
    let line = lines
        .iter()
        .find(|line| line.text.contains("let value"))
        .expect("source row");
    assert_eq!(line.text, "2 │     let value = \"hello\";");
    let keyword = line
        .styled_segments
        .iter()
        .find(|span| span.text.contains("let"))
        .expect("keyword")
        .style
        .fg;
    let string = line
        .styled_segments
        .iter()
        .find(|span| span.text.contains("hello"))
        .expect("string")
        .style
        .fg;
    assert_ne!(keyword, string);
    assert!(paint(&mut view, ToolOutputMode::Compact, 50)
        .iter()
        .any(|line| line.text.contains("src/main file.rs")));
}

#[test]
fn failed_rtk_tests_keep_saved_log_references_in_full() {
    let mut view = tool("$ rtk cargo test -p example\nFAILED: 1/3 tests\n[full output: /tmp/rtk-fixture-test.log]\nexit code: 101".into(),
        "run Test cargo test -p example", None, true);
    assert!(paint(&mut view, ToolOutputMode::Compact, 80)
        .iter()
        .any(
            |line| line.text.contains("Test cargo test -p example") && line.text.contains("failed")
        ));
    assert!(paint(&mut view, ToolOutputMode::Full, 80)
        .iter()
        .any(|line| line
            .text
            .contains("[full output: /tmp/rtk-fixture-test.log]")));
}
