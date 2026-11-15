//! Check shared spacing while text wraps, tools unfold, and the viewport changes.

use carlos_core::session::models::Role;

use super::render_fixture;
use crate::app::layout_state::TranscriptTheme;
use crate::app::perf_view::PerfView;
use crate::app::render_cache_state::RenderCacheState;
use crate::app::tool_output_state::ToolOutputMode;

fn desktop() -> PerfView {
    let mut app = render_fixture::view(Vec::new());
    app.view.layout.active = TranscriptTheme::Gutter;
    app
}

#[test]
fn desktop_groups_tools_with_short_reasoning_and_separates_other_messages() {
    for mode in [ToolOutputMode::Compact, ToolOutputMode::Full] {
        let mut app = desktop();
        app.view.tool_output.mode = mode;
        for (role, text) in [
            (Role::User, "First request"),
            (Role::User, "Second request"),
            (Role::Reasoning, "Long thought\nMore detail"),
            (Role::ToolCall, "→ Read source.rs"),
            (Role::Reasoning, "**Checking inputs**"),
            (Role::Reasoning, "**Checking matches**"),
            (Role::ToolCall, "✱ Search value"),
            (Role::ToolCall, "run cargo test"),
            (Role::Reasoning, "Another thought\nMore detail"),
            (Role::Commentary, "First update"),
            (Role::Commentary, "Second update"),
            (Role::Assistant, "All done"),
        ] {
            render_fixture::append(&mut app, role, text);
        }
        app.ensure_rendered_lines(50);
        let count = app.rendered_line_count();
        let lines = render_fixture::snapshot_current(&mut app);
        assert_eq!(lines.len(), count);
        for index in [1, 2, 3, 8, 9, 10, 11] {
            let start = app
                .view
                .render_cache
                .block_start(index)
                .expect("rendered entry");
            assert!(lines[start].separator && lines[start].text.is_empty());
        }
        for index in 4..8 {
            let start = app
                .view
                .render_cache
                .block_start(index)
                .expect("rendered entry");
            assert!(!lines[start].separator);
        }
    }
}

#[test]
fn phone_and_desktop_count_spacing_using_the_space_available_for_reasoning() {
    let mut app = desktop();
    render_fixture::append(&mut app, Role::ToolCall, "run first");
    render_fixture::append(&mut app, Role::Reasoning, "12345678901234567");
    render_fixture::append(&mut app, Role::ToolOutput, "first\n\nsecond");
    for phone in [false, true, false] {
        app.view.layout.active = if phone {
            TranscriptTheme::Flow
        } else {
            TranscriptTheme::Gutter
        };
        app.ensure_rendered_lines(18);
        let count = app.rendered_line_count();
        let lines = render_fixture::snapshot_current(&mut app);
        assert_eq!(lines.len(), count);
        let reason = app
            .view
            .render_cache
            .block_start(1)
            .expect("rendered entry");
        assert_eq!(lines[reason].separator, phone);
        let output = app
            .view
            .render_cache
            .block_start(2)
            .expect("rendered entry");
        let body = output + app.view.render_cache.first_content_row(2);
        let body = body + usize::from(phone); // Phone includes the tool label.
        assert_eq!(lines[body].text, "first");
        assert!(lines[body + 1].text.is_empty() && !lines[body + 1].separator);
        assert_eq!(lines[body + 2].text, "second");
    }
}

#[test]
fn cache_rebuild_after_a_prepend_refreshes_changed_text_under_existing_ids() {
    let original: Vec<_> = [
        (Role::Assistant, "Earlier answer"),
        (Role::Assistant, "Old cached answer"),
        (Role::User, "Prepended request"),
    ]
    .into_iter()
    .map(|(role, text)| {
        crate::app::tests::test_message(carlos_core::session::models::EntryDraft {
            role,
            kind: carlos_core::session::models::MessageKind::Plain,
            text: text.into(),
            file_path: None,
            tool_output: None,
        })
    })
    .collect();
    let mut moved = vec![
        original[2].clone(),
        original[0].clone(),
        original[1].clone(),
    ];
    moved[2].text =
        "Changed answer with enough new words to wrap onto several rows.\n\nAnother paragraph."
            .into();
    for theme in [TranscriptTheme::Gutter, TranscriptTheme::Flow] {
        for width in [12, 28, 80] {
            let mut cache = RenderCacheState::new();
            cache.ensure_rendered_lines(&original[..2], width, None, false, theme);
            cache.snapshot_rendered_lines(&original[..2]);
            cache.mark_transcript_dirty_from(moved.len(), 0);
            cache.ensure_rendered_lines(&moved, width, None, false, theme);
            let actual = cache.snapshot_rendered_lines(&moved);
            let mut fresh = RenderCacheState::new();
            fresh.ensure_rendered_lines(&moved, width, None, false, theme);
            let expected = fresh.snapshot_rendered_lines(&moved);
            assert_eq!(cache.rendered_line_count(), fresh.rendered_line_count());
            assert_eq!(
                actual.iter().map(|line| &line.text).collect::<Vec<_>>(),
                expected.iter().map(|line| &line.text).collect::<Vec<_>>()
            );
        }
    }
}
