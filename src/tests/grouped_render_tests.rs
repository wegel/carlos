//! Compare incremental immutable-row paint with a clean cache across roles and layouts.

use carlos_core::session::models::Role;
use carlos_proto::session::EntryKind;

use super::render_fixture;
use crate::app::layout_state::TranscriptTheme;
use crate::app::perf_view::PerfView;
use crate::app::render_cache_state::RenderCacheState;

#[test]
fn incremental_grouped_rows_match_fresh_layouts_with_all_original_styles() {
    for width in [60, 160] {
        for theme in [TranscriptTheme::Gutter, TranscriptTheme::Flow] {
            for hidden in [None, Some(0)] {
                for kind in [
                    EntryKind::Answer,
                    EntryKind::ToolCall,
                    EntryKind::Tool,
                    EntryKind::User,
                ] {
                    check_updates(width, theme, hidden, kind);
                }
            }
        }
    }
}

fn check_updates(width: usize, theme: TranscriptTheme, hidden: Option<usize>, kind: EntryKind) {
    let mut view = render_fixture::view(vec![
        render_fixture::row(Role::User, "question"),
        render_fixture::row(Role::ToolCall, "→ Read a.rs ×2"),
        render_fixture::row(Role::ToolCall, ""),
        render_fixture::row(Role::Assistant, "answer\n\nsecond paragraph"),
    ]);
    view.view.layout.active = theme;
    paint(&mut view, width, hidden);
    render_fixture::replace(&mut view, 1, "→ Read a.rs ×3");
    compare_fresh(&mut view, width, hidden);
    view.rows[2].entry.kind = kind;
    render_fixture::replace(&mut view, 2, "changed shell\nsecond line");
    compare_fresh(&mut view, width, hidden);
    render_fixture::replace(&mut view, 1, "→ Read a.rs ×3\nstreamed change");
    compare_fresh(&mut view, width, hidden);
}

fn compare_fresh(view: &mut PerfView, width: usize, hidden: Option<usize>) {
    let incremental = paint(view, width, hidden);
    view.view.render_cache = RenderCacheState::new();
    assert_eq!(incremental, paint(view, width, hidden));
}

fn paint(view: &mut PerfView, width: usize, hidden: Option<usize>) -> Vec<String> {
    view.view.render_cache.ensure_inputs(
        view.rows.as_slice(),
        width,
        hidden,
        false,
        view.view.layout.active,
    );
    render_fixture::snapshot_current(view)
        .iter()
        .map(|row| format!("{row:?}"))
        .collect()
}
