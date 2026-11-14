//! Probe the legacy reasoning counter and existing whole-document logical-line helper.

#[path = "transcript_spacing_probe.rs"]
mod probe;

use carlos_core::session::models::{MessageKind, Role};

use super::one_row::{plain_uses_one_row, reasoning_uses_one_row};
use super::{inline_entry, single_line_reasoning};
use crate::app::text::wrap_natural_count_by_cells;
use crate::app::transcript_input::RenderInput;
use crate::app::transcript_styles::{count_wrapped_markdown_lines, styled_plain_text};
use crate::app::transcript_wrap::markdown_logical_lines;

#[test]
fn the_legacy_summary_shortcut_remains_distinct_from_the_rendered_parser() {
    let text = "**hi   **";
    let lines = markdown_logical_lines(Role::Reasoning, text, 8);
    let parsed: usize = lines
        .iter()
        .map(|line| wrap_natural_count_by_cells(&styled_plain_text(line), 8))
        .sum();
    assert_eq!(parsed, 2);
    assert_eq!(count_wrapped_markdown_lines(Role::Reasoning, text, 8), 1);
    assert!(single_line_reasoning(input(Role::Reasoning, text), 8));
}

#[test]
fn one_hard_slice_matches_the_plain_counter_on_exhaustive_small_inputs() {
    let atoms = ["a", " ", "界", "\u{200b}", "é", "क्", "🇫", "‍", "\t", "\r"];
    for length in 0..=4 {
        for mut encoded in 0..atoms.len().pow(length) {
            let mut text = String::new();
            for _ in 0..length {
                text.push_str(atoms[encoded % atoms.len()]);
                encoded /= atoms.len();
            }
            for width in [0, 1, 2, 3, 8, 12] {
                assert_eq!(
                    plain_uses_one_row(&text, width),
                    wrap_natural_count_by_cells(&text, width) == 1,
                    "width{width},text{text:?}"
                );
            }
        }
    }
}

#[test]
fn one_reasoning_row_keeps_legacy_shortcuts_and_parser_fallbacks() {
    let atoms = [
        "a", " ", "*", "_", "`", "[x]", ":url", "\n", "\t", "界", "क्",
    ];
    for length in 0..=4 {
        for mut encoded in 0..atoms.len().pow(length) {
            let mut text = String::new();
            for _ in 0..length {
                text.push_str(atoms[encoded % atoms.len()]);
                encoded /= atoms.len();
            }
            for width in [0, 7, 8, 12, 28] {
                assert_eq!(
                    reasoning_uses_one_row(&text, width),
                    count_wrapped_markdown_lines(Role::Reasoning, &text, width) == 1,
                    "width{width},text{text:?}"
                );
            }
        }
    }
}

#[test]
fn randomized_unicode_and_markdown_keep_the_exact_old_one_row_result() {
    let atoms = [
        "a",
        "longword",
        " **",
        "** ",
        "*",
        "_",
        "`",
        "[x][id]",
        "[id]: /target",
        "|a|",
        "\n",
        "\t",
        "界",
        "क्",
        "لا",
        "א‍ל",
        "ⵏ⵿ⴾ",
        "ᨕᨗ‍ᨐ",
        "🇫🇷",
        "👩‍💻",
        "\u{200b}",
        "#️⃣",
        "\x1b[31m",
    ];
    let mut state = 83421_u64;
    for count in 0..1000 {
        let mut text = String::new();
        for _ in 0..count % 80 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(atoms[(state >> 32) as usize % atoms.len()]);
        }
        for width in [1, 2, 8, 12, 28, 120] {
            assert_eq!(
                plain_uses_one_row(&text, width),
                wrap_natural_count_by_cells(&text, width) == 1,
                "plain width{width},text{text:?}"
            );
            assert_eq!(
                reasoning_uses_one_row(&text, width),
                count_wrapped_markdown_lines(Role::Reasoning, &text, width) == 1,
                "markdown width{width},text{text:?}"
            );
            assert_eq!(
                single_line_reasoning(input(Role::Reasoning, &text), width),
                old(Role::Reasoning, &text, width),
                "predicate width{width},text{text:?}"
            );
        }
    }
}

#[test]
fn role_width_newlines_tables_and_late_reference_rules_stay_unchanged() {
    let texts = [
        "",
        " ",
        "  \n**hi**\n  ",
        "first\nsecond",
        "[label][id]\n\n[id]: /late",
        "| first | second |\n|---|---|\n| x | y |",
        "# title",
        "> quote",
        "- item",
        "```rs\nlet x = 3;\n```",
        "**hi   **",
        "\n\t\n",
    ];
    for role in [
        Role::Reasoning,
        Role::Assistant,
        Role::Commentary,
        Role::System,
        Role::User,
        Role::ToolCall,
        Role::ToolOutput,
    ] {
        for text in texts {
            for width in [0, 1, 7, 8, 12, 28] {
                let expected = old(role, text, width);
                assert_eq!(
                    single_line_reasoning(input(role, text), width),
                    expected,
                    "role{role:?},width{width},text{text:?}"
                );
                assert_eq!(
                    inline_entry(input(role, text), width),
                    matches!(role, Role::ToolCall | Role::ToolOutput) || expected
                );
            }
        }
    }
}

#[test]
fn a_large_visible_line_rejects_while_a_single_overwide_grapheme_keeps_its_old_row() {
    let visible = "x".repeat(1024 * 1024) + "🙂";
    assert!(!single_line_reasoning(input(Role::Reasoning, &visible), 28));
    let grapheme = "क्".repeat(32) + "क";
    assert!(old(Role::Reasoning, &grapheme, 8));
    for text in [&grapheme, &(grapheme.clone() + " ")] {
        for width in [8, 12, 28] {
            assert_eq!(
                single_line_reasoning(input(Role::Reasoning, text), width),
                old(Role::Reasoning, text, width)
            );
        }
    }
}

fn input(role: Role, text: &str) -> RenderInput<'_> {
    RenderInput {
        role,
        text,
        kind: MessageKind::Plain,
        file_path: None,
        label: None,
        label_generated: false,
        failed: false,
    }
}

fn old(role: Role, text: &str, width: usize) -> bool {
    width >= 8
        && role == Role::Reasoning
        && !text.trim().contains('\n')
        && count_wrapped_markdown_lines(Role::Reasoning, text, width) == 1
}
