//! Exercises table structure, cell wrapping, styles, and exact transcript height.

use ratatui::style::Modifier;

use super::{
    build_rendered_block_for_message, count_rendered_block_for_message, visual_width, Message,
    MessageKind, RenderedLine, Role,
};
use super::{selected_text, Selection};

fn render(text: &str, role: Role, width: usize) -> Vec<RenderedLine> {
    let message = Message {
        role,
        text: text.to_string(),
        kind: MessageKind::Plain,
        tool_output: None,
        file_path: None,
    };
    let lines = build_rendered_block_for_message(None, &message, width);
    assert_eq!(
        count_rendered_block_for_message(None, &message, width),
        lines.len(),
        "width {width}: {text}"
    );
    lines
}

fn content(lines: &[RenderedLine]) -> String {
    lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn table_cells_wrap_inside_stable_columns_and_pad_shorter_cells() {
    let source = "| Name | Description |\n| :--- | ---: |\n| Alpha | a long sentence that must wrap within the description column and retain its final word FINISH |\n| Beta | short |";
    for width in [30, 42, 70] {
        let lines = render(source, Role::Assistant, width);
        let rows: Vec<_> = lines
            .iter()
            .filter(|line| line.text.starts_with('│'))
            .collect();
        assert!(rows.len() > 3, "{width}: {}", content(&lines));
        let boundaries = |line: &str| {
            line.char_indices()
                .filter(|(_, ch)| *ch == '│')
                .map(|(index, _)| visual_width(&line[..index]))
                .collect::<Vec<_>>()
        };
        let expected = boundaries(&rows[0].text);
        assert_eq!(expected.len(), 3);
        for row in rows {
            assert_eq!(boundaries(&row.text), expected, "{}", row.text);
            assert!(row.cells <= width);
            assert!(!row.soft_wrap_to_next);
        }
        assert!(content(&lines).contains("FINISH"));
        assert!(content(&lines).contains("short │"));
    }
}

#[test]
fn table_count_matches_draw_for_assistant_reasoning_and_every_phone_width() {
    let source = "Before.\n\n| A | B | C |\n| :- | :-: | -: |\n| 日本語😀 | **emphasis preserved across wrapped words** | abcdefghijklmnopqrstuvwxyz0123456789 |\n| | small | end |\n\nAfter.";
    for role in [Role::Assistant, Role::Reasoning] {
        for width in 8..100 {
            let lines = render(source, role, width);
            assert!(
                lines.iter().all(|line| line.cells <= width),
                "{width}: {}",
                content(&lines)
            );
            assert!(content(&lines).contains("After."));
            if width >= 40 {
                assert!(content(&lines).contains('┌'));
            }
        }
    }
}

#[test]
fn narrow_tables_show_complete_labelled_fields_instead_of_broken_borders() {
    let source = "| First | Second | Third |\n| - | - | - |\n| alpha | bravo | charlie-final |\n| delta | echo | foxtrot-final |";
    let lines = render(source, Role::Assistant, 12);
    let output = content(&lines);
    assert!(output.contains("Row 1"));
    assert!(output.contains("Row 2"));
    assert!(!output.contains('┌'));
    let compact: String = output.chars().filter(|ch| !ch.is_whitespace()).collect();
    for value in [
        "First: alpha",
        "Second: bravo",
        "charlie-final",
        "delta",
        "echo",
        "foxtrot-final",
    ] {
        assert!(
            compact.contains(&value.replace(' ', "")),
            "{value}: {output}"
        );
    }
}

#[test]
fn cell_wrapping_keeps_unicode_graphemes_long_tokens_and_inline_styles() {
    let token = "👩‍💻e\u{301}日本語".repeat(8);
    let source = format!("| Value | Notes |\n| - | - |\n| **{token}** | end |");
    let lines = render(&source, Role::Assistant, 30);
    let cells: Vec<_> = lines
        .iter()
        .filter(|line| line.text.starts_with('│'))
        .skip(1)
        .map(|line| line.text.split('│').nth(1).expect("first column").trim())
        .collect();
    assert_eq!(cells.concat(), token);
    for line in &lines {
        assert!(line.cells <= 30);
        for segment in &line.styled_segments {
            if segment.text.contains('👩') {
                assert!(segment.style.add_modifier.contains(Modifier::BOLD));
            }
            assert!(!segment.text.starts_with('\u{200d}'));
            assert!(!segment.text.starts_with('\u{301}'));
        }
    }
}

#[test]
fn parser_preserves_escaped_pipes_code_links_empty_cells_and_reference_context() {
    let source = "Before [outside][r].\n\n<!-- ordinary comment -->\n\n| Key | Content |\n| - | - |\n| a\\|b | `x\\|y` and [inside][r] |\n| | |\n\nAfter [outside][r].\n\n[r]: https://example.test/ref";
    let lines = render(source, Role::Assistant, 80);
    let output = content(&lines);
    for expected in [
        "a|b",
        "x|y",
        "inside",
        "ordinary comment",
        "Before outside (https://example.test/ref)",
        "After outside (https://example.test/ref)",
    ] {
        assert!(output.contains(expected), "{expected}: {output}");
    }
    assert_eq!(output.matches("https://example.test/ref").count(), 3);
    assert!(!output.contains("CARLOSTABLE"));
}

#[test]
fn multiple_tables_keep_nested_prefixes_and_do_not_parse_fenced_code_as_tables() {
    let source = "> Before\n>\n> | A | B |\n> | - | - |\n> | words wrap within this quoted column | end |\n>\n> After\n\n- Item\n\n  | C | D |\n  | - | - |\n  | other long content wraps within the list | tail |\n\n```text\n| not | a table |\n| --- | --- |\n```\n\nCARLOSTABLE literal";
    let lines = render(source, Role::Assistant, 40);
    let output = content(&lines);
    assert_eq!(output.matches('┌').count(), 2, "{output}");
    assert!(output.contains("| not | a table |"));
    assert!(output.contains("CARLOSTABLE literal"));
    assert!(lines.iter().all(|line| line.cells <= 40), "{output}");
    let quote = lines
        .iter()
        .find(|line| line.text.contains('┌'))
        .expect("quoted border");
    assert!(quote.text.starts_with("> "), "{}", quote.text);
}

#[test]
fn table_column_alignment_and_physical_copy_lines_remain_intact() {
    let source = "| Left | Center | Right |\n| :--- | :---: | ---: |\n| L | C | R |\n| longer-left | wider-center | widest-right |";
    let lines = render(source, Role::Assistant, 60);
    let row = lines
        .iter()
        .find(|line| line.text.contains(" L "))
        .expect("short aligned row");
    let cells: Vec<_> = row.text.split('│').collect();
    assert!(cells[1].starts_with(" L"));
    assert!(cells[3].ends_with("R "));
    let center = cells[2];
    let left = center.len() - center.trim_start().len();
    let right = center.len() - center.trim_end().len();
    assert!(left.abs_diff(right) <= 1);
    let copied = selected_text(
        Selection {
            anchor_x: 0,
            anchor_line_idx: 0,
            focus_x: 60,
            focus_line_idx: lines.len() - 1,
            dragging: false,
        },
        &lines,
    );
    assert_eq!(copied, content(&lines));
}

#[test]
fn adjacent_prose_tables_comments_and_anchor_like_text_keep_their_boundaries() {
    let source = "Intro [ref][r].\n\n| A | B |\n| - | - |\n| first | value |\n\nBetween CARLOSTABLE CARLOSTABLEX.\n\n<!-- normal -->\n\n| C | D |\n| - | - |\n| second | value |\n\nTail [ref][r].\n\n[r]: https://example.test/";
    let lines = render(source, Role::Assistant, 60);
    let output = content(&lines);
    assert_eq!(output.matches('┌').count(), 2);
    assert!(output.find("Intro").expect("intro") < output.find('┌').expect("table"));
    assert!(output.find("Between").expect("middle") < output.rfind('┌').expect("last table"));
    assert!(output.contains("CARLOSTABLE CARLOSTABLEX"));
    assert!(output.contains("<!-- normal -->"));
    assert_eq!(output.matches("https://example.test/").count(), 2);
    assert!(!output.contains("<!--CARLOSTABLE"));
}

#[test]
fn incomplete_streaming_table_prefixes_always_count_the_lines_they_draw() {
    let source = "| A | B |\n| - | - |\n| **long wrapped styled words** | 日本語e\u{301}👩‍💻 |";
    for (offset, _) in source
        .char_indices()
        .chain(std::iter::once((source.len(), '\0')))
    {
        for role in [Role::Assistant, Role::Reasoning] {
            for width in [8, 16, 36, 80] {
                render(&source[..offset], role, width);
            }
        }
    }
}

#[test]
fn impossible_nested_quote_and_list_columns_fall_back_without_losing_values() {
    let sources = [
        ">> | A | B | C |\n>> | - | - | - |\n>> | first | second | third |",
        "- Outer\n  - Inner\n\n    | A | B | C |\n    | - | - | - |\n    | first | second | third |",
        "| A | B | C |\n| - | - | - |\n| [x](https://例.test/) | second | third |",
    ];
    for source in sources {
        let lines = render(source, Role::Assistant, 13);
        let output = content(&lines);
        assert!(!output.contains('┌'), "{output}");
        assert!(output.contains("Row 1"), "{output}");
        assert!(lines.iter().all(|line| line.cells <= 13), "{output}");
    }
}

#[test]
fn narrow_fallback_keeps_adjacent_prose_as_a_separate_block() {
    let source = "Intro\n\n| A | B | C |\n| - | - | - |\n| a | b | x |\n# Tail";
    let lines = render(source, Role::Assistant, 12);
    assert!(
        lines.iter().any(|line| line.text == "Tail"),
        "{}",
        content(&lines)
    );
    let quoted = "> | A | B | C |\n> | - | - | - |\n> | a | b | x |\n> # Tail";
    let lines = render(quoted, Role::Assistant, 12);
    assert!(
        lines.iter().any(|line| line.text == "> Tail"),
        "{}",
        content(&lines)
    );
}
