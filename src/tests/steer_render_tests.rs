//! Paint steering headings without changing the selected canonical user text.

use carlos_core::session::models::Role;

use super::render_fixture;
use crate::app::layout_state::TranscriptTheme;
use crate::app::selection::Selection;

#[test]
fn steer_labels_do_not_change_copy_text_and_count_equals_draw_at_phone_widths() {
    let plain = render_fixture::row(Role::User, "same 日本語");
    let mut steer = render_fixture::row(Role::User, "same 日本語");
    steer.entry.format = carlos_proto::session::EntryFormat::UserSteer;
    let mut app = render_fixture::view(vec![plain, steer]);
    for theme in [TranscriptTheme::Gutter, TranscriptTheme::Flow] {
        app.view.layout.active = theme;
        for width in [8, 12, 18, 36, 80, 140] {
            app.ensure_rendered_lines(width);
            let count = app.rendered_line_count();
            let lines = render_fixture::snapshot_current(&mut app);
            assert_eq!(count, lines.len());
            assert!(lines.iter().all(|line| line.cells <= width));
            assert!(
                lines
                    .iter()
                    .any(|line| line.separator && line.text.contains("steer")),
                "width {width}"
            );
            let copied = render_fixture::selected(
                &mut app,
                Selection {
                    anchor_x: 1,
                    anchor_line_idx: 0,
                    focus_x: width,
                    focus_line_idx: count.saturating_sub(1),
                    dragging: false,
                },
            );
            assert!(!copied.contains("steer"));
            assert_eq!(app.rows[1].text(), "same 日本語");
            assert!(copied.contains("same"));
        }
    }
}
