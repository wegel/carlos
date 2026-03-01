use super::{slice_by_cells, RenderedLine, MSG_CONTENT_X, MSG_TOP};
use crate::theme::TOUCH_SCROLL_DRAG_MIN_ROWS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MouseDragMode {
    Undecided,
    Select,
    Scroll,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Selection {
    pub(super) anchor_x: usize, // 1-based, content-relative cell column
    pub(super) anchor_y: usize, // 1-based screen row
    pub(super) focus_x: usize,
    pub(super) focus_y: usize,
    pub(super) dragging: bool,
}

pub(super) fn compute_selection_range(
    selection: Selection,
    row: usize,
    line_cells: usize,
) -> Option<(usize, usize)> {
    let mut ax = selection.anchor_x;
    let mut ay = selection.anchor_y;
    let mut fx = selection.focus_x;
    let mut fy = selection.focus_y;

    if fy < ay || (fy == ay && fx < ax) {
        std::mem::swap(&mut ax, &mut fx);
        std::mem::swap(&mut ay, &mut fy);
    }

    if row < ay || row > fy {
        return None;
    }

    let mut start_col = 1usize;
    let mut end_col = line_cells;

    if row == ay {
        start_col = ax;
    }
    if row == fy {
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

pub(super) fn selected_text(
    selection: Selection,
    rendered_lines: &[RenderedLine],
    msg_bottom: usize,
    scroll_top: usize,
) -> String {
    let msg_top = MSG_TOP;

    let mut ax = selection.anchor_x;
    let mut ay = selection.anchor_y;
    let mut fx = selection.focus_x;
    let mut fy = selection.focus_y;
    if fy < ay || (fy == ay && fx < ax) {
        std::mem::swap(&mut ax, &mut fx);
        std::mem::swap(&mut ay, &mut fy);
    }

    if msg_bottom < msg_top || fy < msg_top || ay > msg_bottom {
        return String::new();
    }

    let start_row = ay.max(msg_top);
    let end_row = fy.min(msg_bottom);

    let mut out = String::new();
    let mut first = true;
    let mut prev_row: Option<usize> = None;
    let mut prev_idx: Option<usize> = None;
    let mut prev_soft_wrap = false;

    for row in start_row..=end_row {
        let idx = scroll_top + (row - msg_top);
        if idx >= rendered_lines.len() {
            continue;
        }

        let line = &rendered_lines[idx];
        let line_cells = line.cells;

        let mut s_col = 1usize;
        let mut e_col = line_cells;
        if row == ay {
            s_col = ax;
        }
        if row == fy {
            e_col = fx;
        }

        s_col = s_col.max(1);
        e_col = e_col.min(line_cells);

        if !first {
            let contiguous =
                prev_row.is_some_and(|r| r + 1 == row) && prev_idx.is_some_and(|i| i + 1 == idx);
            if !(contiguous && prev_soft_wrap) {
                out.push('\n');
            }
        }
        first = false;

        if line_cells > 0 && s_col <= e_col && s_col <= line_cells {
            out.push_str(&slice_by_cells(&line.text, s_col - 1, e_col));
        }

        prev_row = Some(row);
        prev_idx = Some(idx);
        prev_soft_wrap = line.soft_wrap_to_next;
    }

    out
}

pub(super) fn normalize_selection_x(col0: usize) -> usize {
    if col0 >= MSG_CONTENT_X {
        col0 - MSG_CONTENT_X + 1
    } else {
        1
    }
}

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
