//! Mouse-drag text selection, copy extraction, and drag-mode detection.

use super::{text::slice_by_cells, RenderedLine};

use crate::theme::TOUCH_SCROLL_DRAG_MIN_ROWS;

// --- Selection Types ---

pub(super) trait RenderedLineSource {
    fn len(&self) -> usize;
    fn get(&self, idx: usize) -> Option<&RenderedLine>;
}

impl RenderedLineSource for [RenderedLine] {
    fn len(&self) -> usize {
        <[RenderedLine]>::len(self)
    }

    fn get(&self, idx: usize) -> Option<&RenderedLine> {
        <[RenderedLine]>::get(self, idx)
    }
}

impl RenderedLineSource for Vec<RenderedLine> {
    fn len(&self) -> usize {
        self.as_slice().len()
    }

    fn get(&self, idx: usize) -> Option<&RenderedLine> {
        self.as_slice().get(idx)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MouseDragMode {
    Undecided,
    Select,
    Scroll,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Selection {
    pub(super) anchor_x: usize, // 1-based, content-relative cell column
    pub(super) anchor_line_idx: usize,
    pub(super) focus_x: usize,
    pub(super) focus_line_idx: usize,
    pub(super) dragging: bool,
}

// --- Selection Range ---

pub(super) fn compute_selection_range(
    selection: Selection,
    line_idx: usize,
    line_cells: usize,
) -> Option<(usize, usize)> {
    let mut ax = selection.anchor_x;
    let mut ay = selection.anchor_line_idx;
    let mut fx = selection.focus_x;
    let mut fy = selection.focus_line_idx;

    if fy < ay || (fy == ay && fx < ax) {
        std::mem::swap(&mut ax, &mut fx);
        std::mem::swap(&mut ay, &mut fy);
    }

    if line_idx < ay || line_idx > fy {
        return None;
    }

    let mut start_col = 1usize;
    let mut end_col = line_cells;

    if line_idx == ay {
        start_col = ax;
    }
    if line_idx == fy {
        end_col = fx;
    }

    if start_col > end_col {
        return None;
    }

    if line_cells == 0 {
        return None;
    }

    start_col = start_col.max(1);
    end_col = end_col.min(line_cells);
    if start_col > line_cells {
        return None;
    }

    Some((start_col - 1, end_col))
}

// --- Text Extraction ---

/// Copy an ordered stream of selected global rows without retaining the rendered rows.
/// The caller clips the row interval against its captured total and omits unavailable rows.
pub(super) struct SelectionText {
    bounds: (usize, usize, usize, usize),
    text: String,
    previous: Option<(usize, bool)>,
    has_whitespace: bool,
}

impl SelectionText {
    /// Normalize selection columns while the caller retains responsibility for clipping row indices.
    pub(super) fn new(selection: Selection) -> Self {
        Self {
            bounds: normalize_selection_bounds(selection),
            text: String::new(),
            previous: None,
            has_whitespace: false,
        }
    }

    /// Append an available row from the clipped interval, preserving its absolute global index.
    pub(super) fn push(&mut self, index: usize, line: &RenderedLine) {
        if line.separator && !line.text.is_empty() {
            return;
        }
        let (ax, ay, fx, fy) = self.bounds;
        let (start, end) = column_range_for_line(index, line.cells, ax, ay, fx, fy);
        let fragment = extract_line_fragment(&line.text, line.cells, start, end);
        let fragment_whitespace = fragment.chars().any(char::is_whitespace);
        let contiguous = self
            .previous
            .is_some_and(|(previous, _)| previous.checked_add(1) == Some(index));
        let soft = self.previous.is_some_and(|(_, soft)| soft);
        if self.previous.is_some() && !(contiguous && soft) {
            self.text.push('\n');
            self.has_whitespace = true;
        } else if contiguous
            && soft
            && should_insert_soft_wrap_space(
                &self.text,
                &fragment,
                self.has_whitespace || fragment_whitespace,
            )
        {
            self.text.push(' ');
            self.has_whitespace = true;
        }
        self.text.push_str(&fragment);
        self.has_whitespace |= fragment_whitespace;
        self.previous = Some((index, line.soft_wrap_to_next));
    }

    /// Return the complete copied text and release the small join state.
    pub(super) fn finish(self) -> String {
        self.text
    }
}

pub(super) fn selected_text<S: RenderedLineSource + ?Sized>(
    selection: Selection,
    rendered_lines: &S,
) -> String {
    let (_, ay, _, fy) = normalize_selection_bounds(selection);
    let mut text = SelectionText::new(selection);
    if rendered_lines.len() > 0 {
        let start = ay.min(rendered_lines.len().saturating_sub(1));
        let end = fy.min(rendered_lines.len().saturating_sub(1));
        for index in start..=end {
            if let Some(line) = rendered_lines.get(index) {
                text.push(index, line);
            }
        }
    }
    text.finish()
}

fn normalize_selection_bounds(selection: Selection) -> (usize, usize, usize, usize) {
    let mut ax = selection.anchor_x;
    let mut ay = selection.anchor_line_idx;
    let mut fx = selection.focus_x;
    let mut fy = selection.focus_line_idx;
    if fy < ay || (fy == ay && fx < ax) {
        std::mem::swap(&mut ax, &mut fx);
        std::mem::swap(&mut ay, &mut fy);
    }
    (ax, ay, fx, fy)
}

fn column_range_for_line(
    idx: usize,
    line_cells: usize,
    ax: usize,
    ay: usize,
    fx: usize,
    fy: usize,
) -> (usize, usize) {
    let s_col = if idx == ay { ax } else { 1 };
    let e_col = if idx == fy { fx } else { line_cells };
    (s_col.max(1), e_col.min(line_cells))
}

fn extract_line_fragment(text: &str, line_cells: usize, s_col: usize, e_col: usize) -> String {
    if line_cells == 0 || s_col > e_col || s_col > line_cells {
        return String::new();
    }
    slice_by_cells(text, s_col - 1, e_col)
}

fn should_insert_soft_wrap_space(existing: &str, fragment: &str, has_whitespace: bool) -> bool {
    let Some(prev_ch) = existing.chars().last() else {
        return false;
    };
    let Some(next_ch) = fragment.chars().next() else {
        return false;
    };

    if prev_ch.is_whitespace() || next_ch.is_whitespace() {
        return false;
    }
    if matches!(next_ch, ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']' | '}') {
        return false;
    }
    if matches!(prev_ch, '(' | '[' | '{' | '/' | '-' | '_') {
        return false;
    }

    has_whitespace
}

// --- Selection Helpers ---

pub(super) fn normalize_selection_x(col0: usize, content_x: usize) -> usize {
    if col0 >= content_x {
        col0 - content_x + 1
    } else {
        1
    }
}

pub(super) fn shift_selection_focus(
    selection: &mut Selection,
    delta_lines: isize,
    max_line_idx: usize,
) {
    if delta_lines == 0 {
        return;
    }
    let next = selection.focus_line_idx.saturating_add_signed(delta_lines);
    selection.focus_line_idx = next.min(max_line_idx);
}

// --- Drag Mode ---

pub(super) fn decide_mouse_drag_mode(
    anchor_x: usize,
    anchor_y: usize,
    x: usize,
    y: usize,
) -> MouseDragMode {
    let row_delta = y.abs_diff(anchor_y);
    let col_delta = x.abs_diff(anchor_x);
    if row_delta >= TOUCH_SCROLL_DRAG_MIN_ROWS && col_delta <= 1 {
        MouseDragMode::Scroll
    } else if row_delta > 0 || col_delta > 0 {
        MouseDragMode::Select
    } else {
        MouseDragMode::Undecided
    }
}

#[cfg(test)]
#[path = "selection_stream_tests.rs"]
mod stream_tests;
