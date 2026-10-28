//! Check shared spacing while text wraps, tools unfold, and the viewport changes.

use super::{AppState, Role};
use crate::app::layout_state::TranscriptTheme;
use crate::app::tool_output_state::ToolOutputMode;

fn desktop() -> AppState {
    let mut app = AppState::new("thread".into());
    app.layout.active = TranscriptTheme::Gutter;
    app
}

#[test]
fn desktop_groups_tools_with_short_reasoning_and_separates_other_messages() {
    for mode in [ToolOutputMode::Compact, ToolOutputMode::Full] {
        let mut app = desktop();
        app.tool_output.mode = mode;
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
            let start = app.render_cache.rendered_block_offsets[index];
            assert!(lines[start].separator && lines[start].text.is_empty());
        }
        for index in 4..8 {
            let start = app.render_cache.rendered_block_offsets[index];
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
    app.viewport.auto_follow_bottom = false;
    app.viewport.scroll_top = app.render_cache.rendered_block_offsets[2];
    for (width, blank) in [(18, true), (50, false), (18, true)] {
        app.ensure_rendered_lines(width, None);
        let count = app.rendered_line_count();
        let lines = app.snapshot_rendered_lines();
        assert_eq!(lines.len(), count);
        for index in [1, 2] {
            assert_eq!(
                lines[app.render_cache.rendered_block_offsets[index]].separator,
                blank
            );
        }
        assert_eq!(
            app.viewport.scroll_top,
            app.render_cache.rendered_block_offsets[2]
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
        app.layout.active = if phone {
            TranscriptTheme::Flow
        } else {
            TranscriptTheme::Gutter
        };
        app.ensure_rendered_lines(18, None);
        let count = app.rendered_line_count();
        let lines = app.snapshot_rendered_lines();
        assert_eq!(lines.len(), count);
        let reason = app.render_cache.rendered_block_offsets[1];
        assert_eq!(lines[reason].separator, phone);
        let output = app.render_cache.rendered_block_offsets[2];
        let body = output + app.render_cache.first_content_row(2);
        let body = body + usize::from(phone); // Phone includes the tool label.
        assert_eq!(lines[body].text, "first");
        assert!(lines[body + 1].text.is_empty() && !lines[body + 1].separator);
        assert_eq!(lines[body + 2].text, "second");
    }
}
