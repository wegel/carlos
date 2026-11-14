//! Move a terminal viewport and choose selection endpoints from pointer gestures.

use super::selection::{decide_mouse_drag_mode, shift_selection_focus, MouseDragMode, Selection};
use super::viewport_state::ViewportState;

/// Describe a pointer position against the current wrapped transcript.
pub(super) struct MouseHitContext {
    pub(super) in_messages: bool,
    pub(super) norm_x: usize,
    pub(super) clamped_y: usize,
    pub(super) clamped_line_idx: usize,
    pub(super) max_scroll: usize,
    pub(super) last_line: usize,
}

/// Move three rows and keep a dragging selection at the same screen position.
pub(super) fn wheel(viewport: &mut ViewportState, hit: &MouseHitContext, is_up: bool) -> bool {
    let prev_scroll = viewport.scroll_top;
    let prev_follow = viewport.auto_follow_bottom;
    let natural_up = is_up != viewport.scroll_inverted;
    if natural_up {
        viewport.scroll_top = viewport.scroll_top.saturating_sub(3);
    } else {
        viewport.scroll_top = (viewport.scroll_top.saturating_add(3)).min(hit.max_scroll);
    }
    viewport.sync_auto_follow_bottom(hit.max_scroll);
    let scroll_delta = viewport.scroll_top as isize - prev_scroll as isize;
    if scroll_delta != 0 && viewport.mouse_drag_mode != MouseDragMode::Scroll {
        let max_line_idx = hit.last_line;
        if let Some(sel) = viewport.selection.as_mut() {
            if sel.dragging {
                shift_selection_focus(sel, scroll_delta, max_line_idx);
            }
        }
    }
    viewport.scroll_top != prev_scroll || viewport.auto_follow_bottom != prev_follow
}

/// Start a candidate selection inside the transcript.
pub(super) fn start(viewport: &mut ViewportState, hit: &MouseHitContext) -> bool {
    viewport.mouse_drag_mode = MouseDragMode::Undecided;
    viewport.mouse_drag_last_row = hit.clamped_y;
    if hit.in_messages {
        viewport.selection = Some(Selection {
            anchor_x: hit.norm_x,
            anchor_line_idx: hit.clamped_line_idx,
            focus_x: hit.norm_x,
            focus_line_idx: hit.clamped_line_idx,
            dragging: true,
        });
        true
    } else {
        false
    }
}

/// Choose a text selection or touch scroll from the existing gesture thresholds.
pub(super) fn drag(viewport: &mut ViewportState, hit: &MouseHitContext) -> bool {
    let Some(sel) = viewport.selection.as_mut() else {
        return false;
    };
    if !sel.dragging {
        return false;
    }
    if viewport.mouse_drag_mode == MouseDragMode::Undecided {
        viewport.mouse_drag_mode = decide_mouse_drag_mode(
            sel.anchor_x,
            viewport.mouse_drag_last_row,
            hit.norm_x,
            hit.clamped_y,
        );
    }

    match viewport.mouse_drag_mode {
        MouseDragMode::Scroll => {
            let prev_scroll = viewport.scroll_top;
            let prev_follow = viewport.auto_follow_bottom;
            if hit.clamped_y > viewport.mouse_drag_last_row {
                viewport.scroll_top = viewport
                    .scroll_top
                    .saturating_sub(hit.clamped_y - viewport.mouse_drag_last_row);
            } else if hit.clamped_y < viewport.mouse_drag_last_row {
                viewport.scroll_top = viewport
                    .scroll_top
                    .saturating_add(viewport.mouse_drag_last_row - hit.clamped_y);
            }
            viewport.scroll_top = viewport.scroll_top.min(hit.max_scroll);
            viewport.sync_auto_follow_bottom(hit.max_scroll);
            viewport.mouse_drag_last_row = hit.clamped_y;
            viewport.scroll_top != prev_scroll || viewport.auto_follow_bottom != prev_follow
        }
        MouseDragMode::Select | MouseDragMode::Undecided => {
            let prev_focus_x = sel.focus_x;
            let prev_focus_idx = sel.focus_line_idx;
            sel.focus_x = hit.norm_x;
            sel.focus_line_idx = hit.clamped_line_idx;
            sel.focus_x != prev_focus_x || sel.focus_line_idx != prev_focus_idx
        }
    }
}

/// End a drag and return only the selection that the caller should copy.
pub(super) fn finish(
    viewport: &mut ViewportState,
    hit: &MouseHitContext,
) -> (bool, Option<Selection>) {
    let mut copied = None;
    let dragging = viewport
        .selection
        .is_some_and(|selection| selection.dragging);
    if let Some(selection) = viewport
        .selection
        .as_mut()
        .filter(|selection| selection.dragging)
    {
        selection.focus_x = hit.norm_x;
        selection.focus_line_idx = hit.clamped_line_idx;
        selection.dragging = false;
        if viewport.mouse_drag_mode == MouseDragMode::Scroll {
            viewport.selection = None;
        } else {
            copied = Some(*selection);
        }
    }
    viewport.mouse_drag_mode = MouseDragMode::Undecided;
    (dragging, copied)
}
