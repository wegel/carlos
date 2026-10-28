//! Keep phone headings out of copied text and preserve anchors when layouts change.

use super::{AppState, Message, MessageKind, Role, TerminalSize};
use crate::app::layout_state::TranscriptTheme;
use crate::app::models::ToolOutputDetails;
use crate::app::selection::Selection;
use crate::app::tool_output_state::ToolOutputMode;

fn transcript() -> AppState {
    let mut app = AppState::new("thread".into());
    app.tool_output.mode = ToolOutputMode::Compact;
    app.append_message(Role::User, "Inspect these files");
    app.append_message(Role::ToolCall, "→ Read a.rs");
    app.append_message(Role::ToolCall, "→ Read b.rs");
    app.append_message(Role::Assistant, "The files are ready.");
    app
}

fn set_layout(app: &mut AppState, mode: TranscriptTheme) {
    app.layout.themes.desktop = mode;
    app.layout.themes.phone = mode;
    app.layout.update_size(TerminalSize {
        width: 100,
        height: 32,
    });
    // Keep width identical to ensure the cache also keys on the layout itself.
    app.ensure_rendered_lines(50, None);
}

#[test]
fn phone_groups_adjacent_reads_and_copy_keeps_only_the_body_text() {
    let mut app = transcript();
    let original: Vec<_> = app.messages.iter().map(|m| m.text.clone()).collect();
    set_layout(&mut app, TranscriptTheme::Flow);
    let lines = app.snapshot_rendered_lines();
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
    assert_eq!(app.render_cache.first_content_row(3), 2);
    let last = lines.len() - 1;
    let selected = app.selected_text(Selection {
        anchor_x: 1,
        anchor_line_idx: 0,
        focus_x: lines[last].cells,
        focus_line_idx: last,
        dragging: false,
    });
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
        app.messages
            .iter()
            .map(|m| m.text.clone())
            .collect::<Vec<_>>(),
        original
    );
    set_layout(&mut app, TranscriptTheme::Gutter);
    let desktop = app.snapshot_rendered_lines();
    assert!(desktop
        .iter()
        .filter(|line| line.separator)
        .all(|line| line.text.is_empty()));
    assert_ne!(desktop.len(), lines.len());
}

#[test]
fn switching_layouts_at_the_same_width_keeps_the_scrolled_message_visible() {
    let mut app = transcript();
    set_layout(&mut app, TranscriptTheme::Gutter);
    app.viewport.auto_follow_bottom = false;
    app.viewport.scroll_top = app.render_cache.rendered_block_offsets[2];
    for mode in [
        TranscriptTheme::Flow,
        TranscriptTheme::Gutter,
        TranscriptTheme::Flow,
    ] {
        set_layout(&mut app, mode);
        app.snapshot_rendered_lines();
        assert_eq!(
            app.viewport.scroll_top,
            app.render_cache.rendered_block_offsets[2]
        );
        let body = app.viewport.scroll_top + app.render_cache.first_content_row(2);
        assert!(app
            .rendered_line_at(body)
            .expect("anchored read")
            .text
            .contains("b.rs"));
    }
}

#[test]
fn fork_and_history_reload_keep_the_local_layout_choice() {
    let mut app = transcript();
    set_layout(&mut app, TranscriptTheme::Flow);
    app.replace_transcript_from(transcript());
    assert_eq!(app.layout.themes.desktop, TranscriptTheme::Flow);
    app.reset_for_forked_thread("fork");
    assert_eq!(app.layout.themes.desktop, TranscriptTheme::Flow);
}

#[test]
fn phone_shares_headings_and_keeps_blank_lines_between_same_type_entries() {
    let mut app = AppState::new("thread".into());
    for role in [Role::User, Role::Commentary, Role::Assistant] {
        app.append_message(role, "First entry");
        app.append_message(role, "Second entry");
    }
    set_layout(&mut app, TranscriptTheme::Flow);
    let lines = app.snapshot_rendered_lines();
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

fn tool_entries() -> Vec<Message> {
    [
        ("→ Read source.rs", MessageKind::Plain, false),
        ("→ Read absent.rs", MessageKind::Plain, true),
        ("run cargo test", MessageKind::Plain, false),
        ("Δ Diff source.rs (+1, -1)", MessageKind::Diff, false),
        ("✱ Search value", MessageKind::Plain, false),
    ]
    .into_iter()
    .map(|(summary, kind, failed)| Message {
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
    .collect()
}

#[test]
fn phone_tool_labels_preserve_full_output_and_counts_at_narrow_widths() {
    for mode in [ToolOutputMode::Compact, ToolOutputMode::Full] {
        for width in [0, 1, 7, 8, 9, 10, 12, 50] {
            let mut app = AppState::new("thread".into());
            app.messages = tool_entries();
            app.tool_output.mode = mode;
            app.layout.active = TranscriptTheme::Flow;
            app.ensure_rendered_lines(width, None);
            let count = app.rendered_line_count();
            let lines = app.snapshot_rendered_lines();
            assert_eq!(count, lines.len(), "{mode:?}, {width}");
            if width >= 8 {
                for index in 1..5 {
                    assert_eq!(app.render_cache.first_content_row(index), 0);
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
    let mut app = transcript();
    app.messages.insert(
        3,
        Message {
            role: Role::Reasoning,
            kind: MessageKind::Plain,
            text: "**Checking results**".into(),
            file_path: None,
            tool_output: None,
        },
    );
    app.messages
        .insert(4, tool_entries().pop().expect("search entry"));
    set_layout(&mut app, TranscriptTheme::Flow);
    let lines = app.snapshot_rendered_lines();
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
    let mut app = AppState::new("thread".into());
    app.append_message(Role::Reasoning, "**Checking the palette**");
    app.append_message(Role::Reasoning, "First paragraph\n\nSecond paragraph");
    set_layout(&mut app, TranscriptTheme::Flow);
    let lines = app.snapshot_rendered_lines();
    assert_eq!(lines[0].text, "◇ Checking the palette");
    assert!(lines[1].text.is_empty());
    assert!(lines[2].text.contains("◇ reasoning"));
    app.ensure_rendered_lines(18, None);
    let count = app.rendered_line_count();
    let narrow = app.snapshot_rendered_lines();
    assert_eq!(narrow.len(), count);
    assert!(narrow[0].text.contains("◇ reasoning"));
    assert!(narrow.iter().all(|line| line.cells <= 18));
    app.ensure_rendered_lines(50, None);
    app.messages[0].text.push_str("\n\nMore details");
    app.mark_transcript_dirty_from(0);
    app.ensure_rendered_lines(50, None);
    let grown = app.snapshot_rendered_lines();
    assert!(grown[0].text.contains("◇ reasoning"));
    assert_eq!(
        grown
            .iter()
            .filter(|line| line.text.contains("◇ reasoning"))
            .count(),
        1
    );
}
