//! Compare cached task rows against the preserved shared wrapper and bound long-token metadata.

use super::PanelRows;
use crate::app::claude_question_view::wrap_lines;

#[test]
fn cached_rows_match_the_existing_panel_at_every_scroll_and_phone_width() {
    let cases = [
        vec!["".into()],
        vec!["head".into(), "".into(), "tail\n".into()],
        vec!["word word     word-foo foo/bar cjk日本語 中文 tabs\tend  ".into()],
        vec!["e\u{301}a 🌍 family 👨‍👩‍👧‍👦🇫🇷 cjk日本語  ".repeat(20)],
        vec!["abc \t \t   ".repeat(30)],
        vec!["a".repeat(4096), "  ".repeat(200), "last".into()],
    ];
    for lines in cases {
        for width in [1, 2, 3, 8, 18, 36, 80] {
            let expected = wrap_lines(lines.clone(), width);
            let mut cached = PanelRows::default();
            cached.rebuild(lines.clone(), width);
            for height in [0, 1, 5, 12] {
                for scroll in [0, 1, 4, expected.len() / 2, expected.len(), usize::MAX] {
                    let start = scroll.min(expected.len().saturating_sub(height));
                    let end = start.saturating_add(height).min(expected.len());
                    assert_eq!(
                        cached.window(scroll, height),
                        expected[start..end],
                        "width={width}, height={height}, scroll={scroll}, input={lines:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_four_mib_token_uses_one_row_run_even_at_one_column() {
    let text = "a".repeat(4 * 1024 * 1024);
    let mut cached = PanelRows::default();
    cached.rebuild(vec![text], 1);
    assert_eq!(cached.rows.count, 4 * 1024 * 1024);
    assert_eq!(cached.rows.runs.len(), 1);
    assert_eq!(cached.rows.runs.capacity(), 1);
    assert_eq!(cached.window(usize::MAX, 3), ["a", "a", "a"]);
    assert!(cached.text.capacity_bytes() <= 32);
}

#[test]
fn irregular_word_breaks_tabs_and_empty_rows_keep_their_exact_text() {
    let lines = vec!["aé🌍中 b c de  f\tg\n\nlast  ".repeat(80)];
    for width in [1, 2, 8, 36] {
        let expected = wrap_lines(lines.clone(), width);
        let mut cached = PanelRows::default();
        cached.rebuild(lines.clone(), width);
        assert_eq!(cached.window(0, expected.len() + 1), expected);
        assert_eq!(cached.width(), width);
        assert_ne!(cached.width(), width + 1);
    }
}
