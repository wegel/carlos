//! Exercise the borrowed-row visitor against the original complete-line wrapper.

use super::{visit_legacy_rows, visit_rows};
use crate::app::text::wrap_natural_by_cells;

#[test]
fn row_visitor_matches_complete_wrapping_for_whitespace_unicode_and_optimal_word_breaks() {
    let atoms = [
        "a",
        "abc-def",
        "界",
        "🙂",
        "👩‍💻",
        "é",
        "\t",
        " ",
        "  ",
        "\u{200b}",
        "\u{fe0f}",
    ];
    let mut state = 4231_u64;
    for count in 0..200 {
        let mut source = String::new();
        for _ in 0..count {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            source.push_str(atoms[(state >> 32) as usize % atoms.len()]);
        }
        for width in [8, 9, 12, 28, 80, 120] {
            let mut actual = Vec::new();
            let rows = visit_rows(&source, width, |_, part| actual.push(part.to_string()));
            assert_eq!(
                actual,
                wrap_natural_by_cells(&source, width),
                "width{width}: {source:?}"
            );
            assert_eq!(rows, actual.len());
        }
    }
    for source in [
        "".to_string(),
        " ".repeat(103),
        "a".repeat(1024) + &" ".repeat(103),
    ] {
        for width in [8, 28, 120] {
            let mut actual = Vec::new();
            visit_rows(&source, width, |_, part| actual.push(part.to_string()));
            assert_eq!(actual, wrap_natural_by_cells(&source, width));
        }
    }
}

#[test]
fn compact_optimal_fragments_preserve_the_previous_public_wrap_path() {
    let cases = [
        "To be, or not to be: that is the question".to_string(),
        "   foobar baz     another-long-word\ninner line  ".repeat(40),
        "界🙂é\t👩‍💻 #️⃣ ◻︎\u{200b}  ".repeat(800),
        "la-a-bc  ".repeat(4000),
        "\x1b[31mcolored words\x1b[0m  ".repeat(1000),
    ];
    for text in cases {
        for width in [8, 10, 12, 28, 80, 120] {
            let mut expected = Vec::new();
            let mut actual = Vec::new();
            visit_legacy_rows(&text, width, |_, part| expected.push(part.to_string()));
            visit_rows(&text, width, |_, part| actual.push(part.to_string()));
            assert_eq!(actual, expected, "width{width},text{text:?}");
        }
    }
}
