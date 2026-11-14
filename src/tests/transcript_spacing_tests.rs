//! Check shared spacing while text wraps, tools unfold, and the viewport changes.

use carlos_core::session::models::Role;

use super::AppState;
use crate::app::layout_state::TranscriptTheme;
use crate::app::render_cache_state::RenderCacheState;
use crate::app::tool_output_state::ToolOutputMode;

fn desktop() -> AppState {
    let mut app = AppState::new("thread".into());
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
            app.append_message(role, text);
        }
        app.ensure_rendered_lines(50, None);
        let count = app.rendered_line_count();
        let lines = app.snapshot_rendered_lines();
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
fn desktop_recounts_spacing_on_resize_without_losing_the_scrolled_message() {
    let mut app = desktop();
    app.append_message(Role::ToolCall, "run first");
    app.append_message(Role::Reasoning, "**Checking the palette**");
    app.append_message(Role::ToolCall, "run second");
    app.ensure_rendered_lines(50, None);
    app.snapshot_rendered_lines();
    app.view.viewport.auto_follow_bottom = false;
    app.view.viewport.scroll_top = app
        .view
        .render_cache
        .block_start(2)
        .expect("rendered entry");
    for (width, blank) in [(18, true), (50, false), (18, true)] {
        app.ensure_rendered_lines(width, None);
        let count = app.rendered_line_count();
        let lines = app.snapshot_rendered_lines();
        assert_eq!(lines.len(), count);
        for index in [1, 2] {
            assert_eq!(
                lines[app
                    .view
                    .render_cache
                    .block_start(index)
                    .expect("rendered entry")]
                .separator,
                blank
            );
        }
        assert_eq!(
            app.view.viewport.scroll_top,
            app.view
                .render_cache
                .block_start(2)
                .expect("rendered entry")
        );
    }
}

#[test]
fn phone_and_desktop_count_spacing_using_the_space_available_for_reasoning() {
    let mut app = desktop();
    app.append_message(Role::ToolCall, "run first");
    app.append_message(Role::Reasoning, "12345678901234567");
    app.append_message(Role::ToolOutput, "first\n\nsecond");
    for phone in [false, true, false] {
        app.view.layout.active = if phone {
            TranscriptTheme::Flow
        } else {
            TranscriptTheme::Gutter
        };
        app.ensure_rendered_lines(18, None);
        let count = app.rendered_line_count();
        let lines = app.snapshot_rendered_lines();
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
    let mut app = desktop();
    app.append_message(Role::Assistant, "Earlier answer");
    app.append_message(Role::Assistant, "Old cached answer");
    app.append_message(Role::User, "Prepended request");
    let original = app.session.messages();
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
