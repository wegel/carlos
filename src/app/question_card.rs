//! Paint borrowed callback cards with the same bounded answer fields and caret on every terminal route.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::{
    compute_overlay_box, draw_rounded_border, fill_box_background, fill_fullscreen_overlay,
    OverlayBox,
};
use crate::app::claude_question_state::NativeDraft;
use crate::app::claude_question_view::{
    editor_line, footer_lines, wrap_borrowed_lines, QuestionControls,
};
use crate::app::render::draw_str;
use crate::app::TerminalSize;
use crate::theme::{COLOR_DIM, COLOR_PRIMARY, COLOR_TEXT};

struct QuestionLayout {
    bounds: OverlayBox,
    width: usize,
    footer: Vec<String>,
    editor: Option<(String, usize)>,
    editor_y: usize,
    body_height: usize,
}

/// Paint a callback using borrowed labels and the shared local answer editor.
pub(in crate::app) struct QuestionCard<'a> {
    pub(in crate::app) title: &'a str,
    pub(in crate::app) caller: &'a str,
    pub(in crate::app) body: &'a [String],
    pub(in crate::app) count: usize,
    pub(in crate::app) controls: QuestionControls,
}

/// Draw a native callback while keeping answer controls within the terminal.
pub(in crate::app) fn draw(
    buf: &mut Buffer,
    size: TerminalSize,
    card: QuestionCard<'_>,
    draft: &NativeDraft,
) {
    fill_fullscreen_overlay(buf, size);
    let Some(layout) = question_layout(size, draft, card.controls) else {
        draw_small(buf, size, &card);
        return;
    };
    let ob = &layout.bounds;
    fill_box_background(buf, ob, ob.bottom - ob.top + 1);
    draw_rounded_border(buf, ob);
    draw_str(
        buf,
        ob.left + 2,
        ob.top + 1,
        &format!("[{} waiting] {}", card.count, card.title),
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        layout.width,
    );
    draw_str(
        buf,
        ob.left + 2,
        ob.top + 2,
        card.caller,
        Style::default().fg(COLOR_DIM),
        layout.width,
    );
    draw_body(buf, &layout, card.body, draft);
    if let Some((text, _)) = &layout.editor {
        draw_str(
            buf,
            ob.left + 2,
            layout.editor_y,
            text,
            Style::default().fg(COLOR_TEXT),
            layout.width,
        );
    }
    for (i, line) in layout.footer.iter().enumerate() {
        draw_str(
            buf,
            ob.left + 2,
            ob.bottom - layout.footer.len() + i,
            line,
            Style::default().fg(COLOR_DIM),
            layout.width,
        );
    }
}

/// Put the terminal caret in the native answer editor, including after a resize.
pub(in crate::app) fn cursor(
    size: TerminalSize,
    draft: &NativeDraft,
    controls: QuestionControls,
) -> Option<(usize, usize)> {
    let layout = question_layout(size, draft, controls)?;
    layout
        .editor
        .map(|(_, column)| (layout.bounds.left + 2 + column, layout.editor_y))
}

fn question_layout(
    size: TerminalSize,
    draft: &NativeDraft,
    controls: QuestionControls,
) -> Option<QuestionLayout> {
    if !crate::app::claude_question_state::can_edit_question(size) {
        return None;
    }
    let width = (if size.width < 40 {
        size.width
    } else {
        size.width - 2
    })
    .min(100);
    let height = if size.height < 18 {
        size.height
    } else {
        size.height - 2
    };
    let bounds = compute_overlay_box(size, width, height);
    let footer = footer_lines(draft, controls, width - 4, height < 18 || width < 40);
    let editor = editor_line(draft, width - 4);
    let editor_y = bounds.bottom - footer.len() - 1;
    let body_height = height.saturating_sub(footer.len() + 4 + usize::from(editor.is_some()));
    Some(QuestionLayout {
        bounds,
        width: width - 4,
        footer,
        editor,
        editor_y,
        body_height,
    })
}

fn draw_body(buf: &mut Buffer, layout: &QuestionLayout, body: &[String], draft: &NativeDraft) {
    let lines = wrap_borrowed_lines(body, layout.width);
    let max_scroll = lines.len().saturating_sub(layout.body_height);
    draft.viewport.max_scroll.set(max_scroll);
    let mut scroll = draft.viewport.scroll.get().min(max_scroll);
    if draft.viewport.follow_focus.get() && draft.error.is_none() && !draft.review {
        if let Some(focus) = lines.iter().position(|line| line.starts_with("> [")) {
            if focus >= scroll + layout.body_height {
                scroll = focus + 1 - layout.body_height;
            }
            if focus < scroll {
                scroll = focus;
            }
        } else {
            scroll = 0;
        }
    }
    draft.viewport.scroll.set(scroll);
    for (i, line) in lines
        .iter()
        .skip(scroll)
        .take(layout.body_height)
        .enumerate()
    {
        draw_str(
            buf,
            layout.bounds.left + 2,
            layout.bounds.top + 3 + i,
            line,
            Style::default().fg(COLOR_TEXT),
            layout.width,
        );
    }
    if max_scroll > 0 {
        draw_str(
            buf,
            layout.bounds.right.saturating_sub(16),
            layout.bounds.bottom,
            &format!(" {}/{} PgUp/PgDn ", scroll + 1, max_scroll + 1),
            Style::default().fg(COLOR_DIM),
            16,
        );
    }
}

fn draw_small(buf: &mut Buffer, size: TerminalSize, card: &QuestionCard<'_>) {
    let question = card.controls.blocking.is_some();
    let mut rows = vec![
        if card.controls.claude {
            "Claude question"
        } else {
            "Codex question"
        },
        card.title,
        "Resize to edit",
    ];
    if card.controls.decline {
        rows.push(if question {
            "Alt+N skip"
        } else {
            "Alt+N decline"
        });
    }
    if card.controls.cancel {
        rows.push(if question {
            "Alt+C interrupt"
        } else {
            "Alt+C cancel"
        });
    }
    rows.push("F10 interrupt");
    for (row, text) in rows.into_iter().take(size.height).enumerate() {
        draw_str(
            buf,
            0,
            row,
            text,
            Style::default().fg(COLOR_TEXT),
            size.width,
        );
    }
}
