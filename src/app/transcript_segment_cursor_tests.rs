//! Compare borrowed styled spans with the old owned-suffix consumer.

use std::collections::VecDeque;

use ratatui::style::{Color, Style};

use super::SegmentCursor;
use crate::app::transcript_styles::take_styled_segments_by_cells;
use crate::app::view_models::StyledSegment;

#[test]
fn borrowed_cursor_matches_owned_consumption_including_tabs_and_zero_width_spans() {
    let texts = [
        "",
        " ",
        "界🙂é\tend  ",
        "👩‍💻x\u{fe0f}",
        "\u{200b}",
        "first\tlast",
        "a\t界\tend",
    ];
    for rotation in 0..texts.len() {
        let spans: Vec<_> = (0..texts.len())
            .map(|index| StyledSegment {
                text: texts[(rotation + index) % texts.len()].repeat(7),
                style: Style::default().fg(if index % 2 == 0 {
                    Color::Red
                } else {
                    Color::Green
                }),
            })
            .collect();
        for width in [1, 2, 7, 8, 12, 28, 80] {
            let mut expected = VecDeque::from(spans.clone());
            let mut cursor = SegmentCursor::new(&spans);
            for _ in 0..300 {
                let old = take_styled_segments_by_cells(&mut expected, width);
                let actual = cursor.take(width, true);
                assert_eq!(
                    format!("{actual:?}"),
                    format!("{old:?}"),
                    "rotation{rotation},width{width}"
                );
            }
        }
    }
}
