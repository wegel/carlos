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
        "#️⃣*️⃣1️⃣◻︎◻️🇫‍🇷\r\nend",
        "لا א‍ל ⵏ⵿ⴾ ᨕᨗ‍ᨐ ꓹꓼ 🇫🇷🙂\t",
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

#[test]
fn a_late_unicode_tab_suffix_does_not_trigger_repeated_full_prefix_width_scans() {
    let text = "ordinary words, [punctuation]: x-y; more text  ".repeat(24000) + " next, 界\t🙂";
    let spans = vec![StyledSegment {
        text,
        style: Style::default(),
    }];
    let mut cursor = SegmentCursor::new(&spans);
    while cursor.position() < spans[0].text.len() {
        cursor.take(28, false);
    }
    assert!(
        cursor.width_scan_bytes() < spans[0].text.len() * 3,
        "whole-suffix scans must shrink with the consumed ASCII prefix"
    );
}

#[test]
fn cached_lower_bounds_preserve_unicode_suffixes_under_changing_row_widths() {
    let atoms = [
        "لا",
        "א‍ל",
        "ⵏ⵿ⴾ",
        "ᨕᨗ‍ᨐ",
        "ꓹꓼ",
        "🇫🇷",
        "👩‍💻",
        "é",
        "\t",
        " ",
        "\u{200b}",
    ];
    let mut state = 8731_u64;
    for _ in 0..100 {
        let mut text = String::new();
        for _ in 0..300 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(atoms[(state >> 32) as usize % atoms.len()]);
        }
        let spans = vec![StyledSegment {
            text,
            style: Style::default().fg(Color::Red),
        }];
        let mut expected = VecDeque::from(spans.clone());
        let mut cursor = SegmentCursor::new(&spans);
        for width in [8, 1, 2, 28, 7, 120].into_iter().cycle().take(1000) {
            assert_eq!(
                format!("{:?}", cursor.take(width, true)),
                format!("{:?}", take_styled_segments_by_cells(&mut expected, width))
            );
        }
    }
}
