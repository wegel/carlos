//! Rendering for the picker's delete-confirmation dialog overlay.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::render::{draw_str, fill_rect};
use super::{TerminalSize, ThreadSummary};
use crate::theme::{COLOR_DIFF_REMOVE, COLOR_DIM, COLOR_STEP2, COLOR_TEXT};

pub(super) fn draw_picker_delete_dialog(
    buf: &mut Buffer,
    size: TerminalSize,
    target: &ThreadSummary,
) {
    if size.width < 24 || size.height < 8 {
        return;
    }

    let dialog_w = size.width.saturating_sub(8).min(72).max(24);
    let dialog_h = 8usize;
    let left = (size.width.saturating_sub(dialog_w)) / 2;
    let top = (size.height.saturating_sub(dialog_h)) / 2;

    draw_delete_dialog_border(buf, left, top, dialog_w, dialog_h);
    draw_delete_dialog_content(buf, left, top, dialog_w, target);
}

fn draw_delete_dialog_border(
    buf: &mut Buffer,
    left: usize,
    top: usize,
    dialog_w: usize,
    dialog_h: usize,
) {
    let right = left + dialog_w.saturating_sub(1);
    let bottom = top + dialog_h.saturating_sub(1);
    let border = Style::default().fg(COLOR_DIFF_REMOVE);

    fill_rect(buf, left, top, dialog_w, dialog_h, Style::default().bg(COLOR_STEP2));

    // corners
    draw_str(buf, left, top, "┏", border, 1);
    draw_str(buf, right, top, "┓", border, 1);
    draw_str(buf, left, bottom, "┗", border, 1);
    draw_str(buf, right, bottom, "┛", border, 1);

    // horizontal edges
    for x in (left + 1)..right {
        draw_str(buf, x, top, "─", border, 1);
        draw_str(buf, x, bottom, "─", border, 1);
    }

    // vertical edges
    for y in (top + 1)..bottom {
        draw_str(buf, left, y, "┃", border, 1);
        draw_str(buf, right, y, "┃", border, 1);
    }
}

fn draw_delete_dialog_content(
    buf: &mut Buffer,
    left: usize,
    top: usize,
    dialog_w: usize,
    target: &ThreadSummary,
) {
    let text_w = dialog_w.saturating_sub(4);
    draw_str(
        buf,
        left + 2,
        top + 1,
        "Delete session?",
        Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
        text_w,
    );
    draw_str(
        buf,
        left + 2,
        top + 3,
        target.name.as_deref().unwrap_or(&target.preview),
        Style::default().fg(COLOR_TEXT),
        text_w,
    );
    draw_str(
        buf,
        left + 2,
        top + 4,
        &target.id,
        Style::default().fg(COLOR_DIM),
        text_w,
    );
    draw_str(
        buf,
        left + 2,
        top + 6,
        "y/Enter delete  n/Esc cancel",
        Style::default().fg(COLOR_DIM),
        text_w,
    );
}
