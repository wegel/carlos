//! Compare bounded ANSI windows with the existing complete body, including cross-checkpoint styles.

use std::sync::Arc;

use carlos_core::session::models::Role;

use super::AnsiPages;
use crate::app::transcript_styles::append_wrapped_ansi_lines;
use crate::app::view_models::RenderedLine;

#[test]
fn ansi_windows_keep_text_style_and_wrap_flags_at_every_boundary() {
    let cases = [
        "\x1b[31mred line\n".repeat(280),
        format!(
            "{}\x1b[0m\nplain after a control-only reset\n",
            "\x1b[1;3;38;2;23;42;99mstyled 🌍\n".repeat(128)
        ),
        format!(
            "\x1b[31m{}\x1b[22;39;49m\r\nend\r",
            "words 🙂 wide 界\tend  \n".repeat(300)
        ),
        format!(
            "\x1b[4;48;5;8m{}\x1b[m\n",
            "**literal** 👩‍💻 é \n".repeat(260)
        ),
        "\x1b[31mline\n\x1b[0m\n\x1b[32m\nlast\x1b[".to_string(),
    ];
    for source in cases {
        for width in [8, 12, 28, 80, 120] {
            let mut expected = Vec::new();
            append_wrapped_ansi_lines(&mut expected, Role::ToolOutput, &source, width);
            let pages = AnsiPages::new(Arc::new(source.clone()), Role::ToolOutput, width)
                .expect("bounded index");
            assert_eq!(pages.original_text(), source);
            assert_eq!(pages.rows(), expected.len(), "width{width}");
            for start in (0..expected.len()).step_by(17) {
                let actual = pages.window(start, 37).expect("visible rows");
                let end = (start + 37).min(expected.len());
                assert_eq!(
                    rows(&actual),
                    rows(&expected[start..end]),
                    "width{width}, start{start}"
                );
            }
            assert!(pages.index_capacity_bytes() < source.len() * 4 + 1024);
        }
    }
}

#[test]
fn saved_general_lines_keep_raw_ranges_and_cross_span_graphemes_exact() {
    let cases = [
        "\x1b[31m31m ".to_string()
            + &"punctuation, words: [a-b] 👩\x1b[32m‍💻\x1b[31m\t界 é  ".repeat(600),
        "\x1b]title 31m\x0731m\x1b[31m".to_string() + &"لا א‍ל ⵏ⵿ⴾ ᨕᨗ‍ᨐ ꓹꓼ 🇫🇷🙂\t ".repeat(700),
        "\x1b[31m".to_string() + &"👩‍💻".repeat(2000) + &" ".repeat(103),
        "\x1b[31m".to_string() + &"long ascii words ".repeat(1800) + "#️⃣\tend",
    ];
    for mut source in cases {
        let mut pages =
            AnsiPages::new(Arc::new(source.clone()), Role::User, 28).expect("general long line");
        assert!(pages.checkpoints[0].long.is_some());
        for addition in ["", "\x1b[0m next words", "\r\nlast\trow  "] {
            source.push_str(addition);
            pages
                .append(addition)
                .expect("reindex complete changed logical line");
            for width in [8, 28, 120] {
                pages = pages.resize(width).expect("reindex width");
                let mut expected = Vec::new();
                append_wrapped_ansi_lines(&mut expected, Role::User, &source, width);
                assert_eq!(pages.rows(), expected.len());
                for start in [0, expected.len() / 2, expected.len().saturating_sub(37)] {
                    let end = (start + 37).min(expected.len());
                    assert_eq!(
                        rows(&pages.window(start, 37).expect("saved viewport")),
                        rows(&expected[start..end]),
                        "width{width},start{start}"
                    );
                }
            }
        }
    }
}

#[test]
fn long_logical_lines_keep_the_existing_word_breaks_and_style_consumption() {
    for source in [
        "\x1b[31m".to_string() + &"x".repeat(32 * 1024),
        "\x1b[31m".to_string() + &"界🙂é\t red \x1b[32mgreen words \x1b[0m".repeat(700),
        "\x1b[31m".to_string() + &"👩‍💻".repeat(1800) + "  ",
    ] {
        for width in [8, 28, 120] {
            let mut expected = Vec::new();
            append_wrapped_ansi_lines(&mut expected, Role::ToolOutput, &source, width);
            let pages = AnsiPages::new(Arc::new(source.clone()), Role::ToolOutput, width)
                .expect("long line has no product byte cap");
            assert_eq!(pages.rows(), expected.len());
            for start in [0, 37, expected.len().saturating_sub(53)] {
                let end = (start + 53).min(expected.len());
                assert_eq!(
                    rows(&pages.window(start, 53).expect("long line viewport")),
                    rows(&expected[start..end]),
                    "width{width},start{start}"
                );
            }
        }
    }
}

#[test]
fn appends_revisit_only_the_last_batch_without_losing_style_or_row_identity() {
    let mut source = "\x1b[31mred line\n".repeat(129);
    let mut pages = AnsiPages::new(Arc::new(source.clone()), Role::ToolOutput, 28)
        .expect("initial checkpoints");
    let first_checkpoint = pages.checkpoints[0].byte;
    for addition in ["unclosed tail", " gets wider 🙂", "\x1b[0m\n", "last\n"] {
        source.push_str(addition);
        pages.append(addition).expect("append");
        let mut expected = Vec::new();
        append_wrapped_ansi_lines(&mut expected, Role::ToolOutput, &source, 28);
        assert_eq!(pages.rows(), expected.len());
        assert_eq!(
            rows(&pages.window(0, pages.rows()).expect("all")),
            rows(&expected)
        );
        assert_eq!(pages.checkpoints[0].byte, first_checkpoint);
    }
    let resized = pages.resize(12).expect("same source at narrow width");
    let mut expected = Vec::new();
    append_wrapped_ansi_lines(&mut expected, Role::ToolOutput, &source, 12);
    assert_eq!(
        rows(&resized.window(0, resized.rows()).expect("all")),
        rows(&expected)
    );
}

#[test]
fn a_saved_literal_word_can_grow_resize_then_change_style_without_losing_its_prefix() {
    let mut source = "\x1b[31m".to_string() + &"x".repeat(32768);
    let mut pages =
        AnsiPages::new(Arc::new(source.clone()), Role::ToolOutput, 28).expect("literal index");
    assert!(pages.checkpoints[0].literal.is_some());
    for addition in ["MORE9", "Z", "\x1b[0mwhite", "\nnext line"] {
        source.push_str(addition);
        pages
            .append(addition)
            .expect("append literal or rebuild changed style");
        for width in [28, 12, 120] {
            pages = pages.resize(width).expect("resize");
            let mut expected = Vec::new();
            append_wrapped_ansi_lines(&mut expected, Role::ToolOutput, &source, width);
            assert_eq!(pages.rows(), expected.len());
            let start = expected.len().saturating_sub(50);
            assert_eq!(
                rows(&pages.window(start, 50).expect("tail")),
                rows(&expected[start..])
            );
        }
    }
}

fn rows(lines: &[RenderedLine]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            format!(
                "{:?}",
                (
                    &line.text,
                    &line.styled_segments,
                    line.cells,
                    line.separator,
                    line.soft_wrap_to_next,
                    line.role
                )
            )
        })
        .collect()
}

#[test]
fn consuming_an_ansi_index_returns_the_existing_original_owner() {
    let source = Arc::new("\x1b[31mred original".to_string());
    let pointer = Arc::as_ptr(&source);
    let pages = AnsiPages::new(source, Role::ToolOutput, 28).expect("ANSI index");
    let source = pages.into_original();
    assert_eq!(Arc::as_ptr(&source), pointer);
    assert_eq!(Arc::strong_count(&source), 1);
    assert_eq!(source.as_str(), "\x1b[31mred original");
}
