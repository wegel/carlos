//! Compare incremental session reports with a fresh layout across themes and hidden rows.

use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;

use crate::app::layout_state::TranscriptTheme;
use crate::app::render_cache_state::RenderCacheState;
use crate::app::state::AppState;
use crate::app::view_models::RenderedLine;

#[test]
fn grouped_shell_updates_dirty_the_preceding_label_and_keep_following_spacing() {
    for width in [60, 160] {
        for theme in [TranscriptTheme::Gutter, TranscriptTheme::Flow] {
            for hidden in [None, Some(0)] {
                for role in [
                    Role::Assistant,
                    Role::ToolCall,
                    Role::ToolOutput,
                    Role::User,
                ] {
                    check_grouped_update(width, theme, hidden, role);
                }
            }
        }
    }
}

fn check_grouped_update(width: usize, theme: TranscriptTheme, hidden: Option<usize>, role: Role) {
    let mut app = AppState::new("thread".into());
    app.view.layout.active = theme;
    app.append_message(Role::User, "question");
    for key in ["first", "shell"] {
        app.upsert_mapped_message(
            key,
            Role::ToolCall,
            "→ Read a.rs".into(),
            MessageKind::Plain,
            None,
        );
    }
    app.append_message(Role::Assistant, "answer\n\nsecond paragraph");
    app.ensure_rendered_lines(width, hidden);
    assert_eq!(app.view.render_cache.transcript_dirty_from, None);
    app.upsert_mapped_message(
        "shell",
        Role::ToolCall,
        "→ Read a.rs".into(),
        MessageKind::Plain,
        None,
    );
    assert_eq!(app.view.render_cache.transcript_dirty_from, Some(1));
    compare_fresh_layout(&mut app, width, hidden);
    app.upsert_mapped_message(
        "shell",
        role,
        "changed shell\nsecond line".into(),
        MessageKind::Plain,
        None,
    );
    assert_eq!(app.view.render_cache.transcript_dirty_from, Some(2));
    compare_fresh_layout(&mut app, width, hidden);
    app.upsert_agent_delta("first", "\nstreamed change");
    assert_eq!(app.view.render_cache.transcript_dirty_from, Some(1));
    compare_fresh_layout(&mut app, width, hidden);
}

fn compare_fresh_layout(app: &mut AppState, width: usize, hidden: Option<usize>) {
    app.ensure_rendered_lines(width, hidden);
    let incremental = line_facts(app.snapshot_rendered_lines());
    app.view.render_cache = RenderCacheState::new();
    app.ensure_rendered_lines(width, hidden);
    assert_eq!(incremental, line_facts(app.snapshot_rendered_lines()));
}

fn line_facts(lines: Vec<RenderedLine>) -> Vec<String> {
    lines.into_iter().map(|line| format!("{line:?}")).collect()
}
