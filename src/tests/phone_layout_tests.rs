//! Keep phone headings out of copied text and preserve anchors when layouts change.

use super::render_fixture;
use crate::app::perf_view::PerfView;
use carlos_core::perf_fixture::PerfRow;
use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;
use carlos_core::session::models::ToolOutputDetails;

use super::TerminalSize;
use crate::app::layout_state::TranscriptTheme;
use crate::app::selection::Selection;
use crate::app::tool_output_state::ToolOutputMode;

fn plain_transcript() -> PerfView {
    let mut view = render_fixture::view(vec![
        render_fixture::row(Role::User, "Inspect these files"),
        render_fixture::row(Role::ToolCall, "→ Read a.rs"),
        render_fixture::row(Role::ToolCall, "→ Read b.rs"),
        render_fixture::row(Role::Assistant, "The files are ready."),
    ]);
    view.view.tool_output.mode = ToolOutputMode::Compact;
    view
}

fn set_plain_layout(view: &mut PerfView, mode: TranscriptTheme) {
    view.view.layout.themes.desktop = mode;
    view.view.layout.themes.phone = mode;
    view.view.layout.update_size(TerminalSize {
        width: 100,
        height: 32,
    });
    view.ensure_rendered_lines(50);
}

#[test]
fn phone_groups_adjacent_reads_and_copy_keeps_only_the_body_text() {
    let mut app = plain_transcript();
    let original: Vec<_> = app.rows.iter().map(|row| row.text().to_owned()).collect();
    set_plain_layout(&mut app, TranscriptTheme::Flow);
    let lines = render_fixture::snapshot_current(&mut app);
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.text.starts_with("→ "))
            .count(),
        2
    );
    assert_eq!(lines.iter().filter(|line| line.separator).count(), 4);
    assert!(lines
        .windows(2)
        .any(|rows| rows[0].text.starts_with("→ a.rs") && rows[1].text.starts_with("→ b.rs")));
    assert_eq!(app.view.render_cache.first_content_row(3), 2);
    let last = lines.len() - 1;
    let selected = render_fixture::selected(
        &mut app,
        Selection {
            anchor_x: 1,
            anchor_line_idx: 0,
            focus_x: lines[last].cells,
            focus_line_idx: last,
            dragging: false,
        },
    );
    for body in [
        "Inspect these files",
        "a.rs",
        "b.rs",
        "The files are ready.",
    ] {
        assert!(selected.contains(body));
    }
    assert!(!selected.contains('─'));
    assert!(!selected.contains("→ read"));
    assert!(!selected.contains("❯ you"));
    assert_eq!(
        app.rows
            .iter()
            .map(|row| row.text().to_owned())
            .collect::<Vec<_>>(),
        original
    );
    set_plain_layout(&mut app, TranscriptTheme::Gutter);
    let desktop = render_fixture::snapshot_current(&mut app);
    assert!(desktop
        .iter()
        .filter(|line| line.separator)
        .all(|line| line.text.is_empty()));
    assert_ne!(desktop.len(), lines.len());
}

#[test]
fn phone_shares_headings_and_keeps_blank_lines_between_same_type_entries() {
    let mut app = render_fixture::view(Vec::new());
    for role in [Role::User, Role::Commentary, Role::Assistant] {
        render_fixture::append(&mut app, role, "First entry");
        render_fixture::append(&mut app, role, "Second entry");
    }
    set_plain_layout(&mut app, TranscriptTheme::Flow);
    let lines = render_fixture::snapshot_current(&mut app);
    assert_eq!(lines.len(), 14);
    let mut start = 0;
    for group in 0..3 {
        if group > 0 {
            assert!(lines[start].separator && lines[start].text.is_empty());
            start += 1;
        }
        let rows = &lines[start..start + 4];
        assert!(rows[0].text.starts_with('─'));
        assert_eq!(rows[1].text, "First entry");
        assert!(rows[2].separator && rows[2].text.is_empty());
        assert_eq!(rows[3].text, "Second entry");
        start += 4;
    }
}

fn tool_entries() -> Vec<PerfRow> {
    [
        ("→ Read source.rs", MessageKind::Plain, false),
        ("→ Read absent.rs", MessageKind::Plain, true),
        ("run cargo test", MessageKind::Plain, false),
        ("Δ Diff source.rs (+1, -1)", MessageKind::Diff, false),
        ("✱ Search value", MessageKind::Plain, false),
    ]
    .into_iter()
    .map(|(summary, kind, failed)| {
        render_fixture::draft(carlos_core::session::models::EntryDraft {
            role: Role::ToolOutput,
            kind,
            text: if kind == MessageKind::Diff {
                "@@ -1 +1 @@\n-old\n+new"
            } else {
                "output detail"
            }
            .into(),
            file_path: Some("source.rs".into()),
            tool_output: Some(ToolOutputDetails {
                generated: false,
                summary: summary.into(),
                failed,
            }),
        })
    })
    .collect()
}

#[test]
fn phone_tool_labels_preserve_full_output_and_counts_at_narrow_widths() {
    for mode in [ToolOutputMode::Compact, ToolOutputMode::Full] {
        for width in [0, 1, 7, 8, 9, 10, 12, 50] {
            let mut app = render_fixture::view(Vec::new());
            app.rows = tool_entries();
            app.view.tool_output.mode = mode;
            app.view.layout.active = TranscriptTheme::Flow;
            app.ensure_rendered_lines(width);
            let count = app.rendered_line_count();
            let lines = render_fixture::snapshot_current(&mut app);
            assert_eq!(count, lines.len(), "{mode:?}, {width}");
            if width >= 8 {
                for index in 1..5 {
                    assert_eq!(app.view.render_cache.first_content_row(index), 0);
                }
                for prefix in ["→ ", "$ ", "Δ ", "✱ "] {
                    assert!(lines.iter().any(|line| line.text.starts_with(prefix)));
                }
                assert!(!lines.iter().any(|line| line.text.contains("─ → read")));
                assert!(lines.iter().all(|line| line.cells <= width));
            }
            if mode == ToolOutputMode::Full && width >= 50 {
                assert!(lines.iter().any(|line| line.text == "output detail"));
                assert!(lines.iter().any(|line| line.text.contains("+new")));
            }
        }
    }
}

#[test]
fn phone_keeps_searches_and_short_reasoning_inside_the_tool_block() {
    let mut app = plain_transcript();
    app.rows.insert(
        3,
        render_fixture::draft(carlos_core::session::models::EntryDraft {
            role: Role::Reasoning,
            kind: MessageKind::Plain,
            text: "**Checking results**".into(),
            file_path: None,
            tool_output: None,
        }),
    );
    app.rows
        .insert(4, tool_entries().pop().expect("search entry"));
    set_plain_layout(&mut app, TranscriptTheme::Flow);
    let lines = render_fixture::snapshot_current(&mut app);
    let read = lines
        .iter()
        .position(|line| line.text.starts_with("→ a.rs"))
        .expect("read row");
    assert!(lines[read - 1].text.is_empty());
    assert!(lines[read + 1].text.starts_with("→ b.rs"));
    assert_eq!(lines[read + 2].text, "◇ Checking results");
    assert!(lines[read + 3].text.starts_with("✱ value"));
    assert!(lines[read + 4].text.is_empty());
}

#[test]
fn phone_reasoning_gains_a_heading_when_it_wraps_or_grows() {
    let mut app = render_fixture::view(Vec::new());
    render_fixture::append(&mut app, Role::Reasoning, "**Checking the palette**");
    render_fixture::append(
        &mut app,
        Role::Reasoning,
        "First paragraph\n\nSecond paragraph",
    );
    set_plain_layout(&mut app, TranscriptTheme::Flow);
    let lines = render_fixture::snapshot_current(&mut app);
    assert_eq!(lines[0].text, "◇ Checking the palette");
    assert!(lines[1].text.is_empty());
    assert!(lines[2].text.contains("◇ reasoning"));
    app.ensure_rendered_lines(18);
    let count = app.rendered_line_count();
    let narrow = render_fixture::snapshot_current(&mut app);
    assert_eq!(narrow.len(), count);
    assert!(narrow[0].text.contains("◇ reasoning"));
    assert!(narrow.iter().all(|line| line.cells <= 18));
    app.ensure_rendered_lines(50);
    let grown = format!("{}\n\nMore details", app.rows[0].text());
    render_fixture::replace(&mut app, 0, grown);
    app.ensure_rendered_lines(50);
    let grown = render_fixture::snapshot_current(&mut app);
    assert!(grown[0].text.contains("◇ reasoning"));
    assert_eq!(
        grown
            .iter()
            .filter(|line| line.text.contains("◇ reasoning"))
            .count(),
        1
    );
}
