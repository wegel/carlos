//! Compare streamed copy batches with the former selection loop over exact global row indices.

use carlos_core::session::models::Role;

use super::{
    column_range_for_line, extract_line_fragment, normalize_selection_bounds, selected_text,
    RenderedLineSource, Selection, SelectionText,
};
use crate::app::text::visual_width;
use crate::app::view_models::RenderedLine;

struct Sparse {
    rows: Vec<Option<RenderedLine>>,
}

impl RenderedLineSource for Sparse {
    fn len(&self) -> usize {
        self.rows.len()
    }
    fn get(&self, index: usize) -> Option<&RenderedLine> {
        self.rows.get(index)?.as_ref()
    }
}

fn row(text: &str, soft: bool, separator: bool) -> RenderedLine {
    RenderedLine {
        text: text.into(),
        cells: visual_width(text),
        styled_segments: Vec::new(),
        role: Role::Assistant,
        separator,
        soft_wrap_to_next: soft,
    }
}

fn selection(ax: usize, ay: usize, fx: usize, fy: usize) -> Selection {
    Selection {
        anchor_x: ax,
        anchor_line_idx: ay,
        focus_x: fx,
        focus_line_idx: fy,
        dragging: false,
    }
}

fn reference(selection: Selection, source: &Sparse) -> String {
    let (ax, ay, fx, fy) = normalize_selection_bounds(selection);
    if source.len() == 0 {
        return String::new();
    }
    let start = ay.min(source.len().saturating_sub(1));
    let end = fy.min(source.len().saturating_sub(1));
    let (mut text, mut previous, mut soft) = (String::new(), None, false);
    for index in start..=end {
        let Some(line) = source.get(index) else {
            continue;
        };
        if line.separator && !line.text.is_empty() {
            continue;
        }
        let (from, to) = column_range_for_line(index, line.cells, ax, ay, fx, fy);
        let fragment = extract_line_fragment(&line.text, line.cells, from, to);
        let contiguous = previous.is_some_and(|previous| previous + 1 == index);
        if previous.is_some() && !(contiguous && soft) {
            text.push('\n');
        } else if contiguous && soft && reference_space(&text, &fragment) {
            text.push(' ');
        }
        text.push_str(&fragment);
        previous = Some(index);
        soft = line.soft_wrap_to_next;
    }
    text
}

fn reference_space(text: &str, fragment: &str) -> bool {
    let (Some(previous), Some(next)) = (text.chars().last(), fragment.chars().next()) else {
        return false;
    };
    if previous.is_whitespace()
        || next.is_whitespace()
        || matches!(next, ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']' | '}')
        || matches!(previous, '(' | '[' | '{' | '/' | '-' | '_')
    {
        return false;
    }
    text.chars().any(char::is_whitespace) || fragment.chars().any(char::is_whitespace)
}

fn streamed(selection: Selection, source: &Sparse, cuts: &[usize]) -> String {
    let mut builder = SelectionText::new(selection);
    let (_, start, _, end) = normalize_selection_bounds(selection);
    if source.len() == 0 {
        return builder.finish();
    }
    let (start, end) = (start.min(source.len() - 1), end.min(source.len() - 1));
    for batch in cuts.windows(2) {
        for index in batch[0]..batch[1] {
            if index >= start && index <= end {
                if let Some(line) = source.get(index) {
                    builder.push(index, line);
                }
            }
        }
    }
    builder.finish()
}

fn fixture() -> Sparse {
    Sparse {
        rows: vec![
            Some(row("first words", true, false)),
            Some(row("continue", true, false)),
            Some(row(". punctuation", false, false)),
            Some(row("HEADER ONE", false, true)),
            Some(row("HEADER TWO", true, true)),
            None,
            Some(row("", false, true)),
            Some(row("path/", true, false)),
            Some(row("next words", true, false)),
            Some(row("e\u{301} 🌍 👨‍👩‍👧‍👦 日本語", false, false)),
            Some(row("\t last  ", false, false)),
            None,
        ],
    }
}

#[test]
fn split_batches_match_the_old_loop_across_headers_gaps_columns_and_unicode() {
    let source = fixture();
    for anchor_row in 0..=source.len() + 1 {
        for focus_row in 0..=source.len() + 1 {
            for anchor in [0, 1, 2, 4, 30] {
                for focus in [0, 1, 3, 8, 30] {
                    let selection = selection(anchor, anchor_row, focus, focus_row);
                    let expected = reference(selection, &source);
                    assert_eq!(selected_text(selection, &source), expected);
                    for split in 0..=source.len() {
                        assert_eq!(
                            streamed(selection, &source, &[0, split, source.len()]),
                            expected,
                            "selection={selection:?}, split={split}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn headers_are_skipped_but_empty_separator_rows_and_missing_indices_break_joins() {
    let source = Sparse {
        rows: vec![
            Some(row("first words", true, false)),
            Some(row("SECTION", true, true)),
            Some(row("MORE HEADER", false, true)),
            Some(row("next", true, false)),
            None,
            Some(row("tail", false, false)),
            Some(row("", false, true)),
            Some(row("end", false, false)),
        ],
    };
    let selection = selection(1, 0, 99, 7);
    let expected = "first words\nnext\ntail\n\nend";
    assert_eq!(reference(selection, &source), expected);
    assert_eq!(
        streamed(selection, &source, &[0, 1, 3, 4, 5, 6, 8]),
        expected
    );
}

#[test]
fn empty_sources_and_selections_beyond_the_end_keep_the_original_clamping() {
    for rows in [
        Vec::new(),
        vec![None],
        vec![Some(row("last row", false, false))],
        vec![Some(row("hidden heading", false, true))],
    ] {
        let source = Sparse { rows };
        for selection in [
            selection(8, 20, 2, 30),
            selection(30, 0, 1, 0),
            selection(0, 0, 0, 0),
        ] {
            assert_eq!(
                streamed(selection, &source, &[0, source.len()]),
                reference(selection, &source)
            );
            assert_eq!(
                selected_text(selection, &source),
                reference(selection, &source)
            );
        }
    }
}

#[test]
fn streamed_rows_can_be_discarded_immediately_and_word_runs_gain_no_spaces() {
    let count = 16384;
    let mut builder = SelectionText::new(selection(1, 0, 128, count - 1));
    for index in 0..count {
        let mut line = row(&"x".repeat(128), true, false);
        builder.push(index, &line);
        line.text.clear();
    }
    assert_eq!(builder.finish(), "x".repeat(128 * count));
}
