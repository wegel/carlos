//! Reuse the existing picker painter while daemon startup remains asynchronous.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::{Bootstrap, Phase};
use crate::app::picker_render::{compute_picker_layout, draw_picker_rows, PickerRow};
use crate::app::TerminalSize;
use crate::theme::{COLOR_DIM, COLOR_OVERLAY, COLOR_TEXT};

impl Bootstrap {
    pub(super) fn draw(&mut self, frame: &mut ratatui::Frame<'_>) {
        let hint = self.error.as_deref().unwrap_or(match self.phase {
            Phase::Greeting => "Connecting to Carlos daemon · Esc cancels",
            Phase::Listing => "Reading saved sessions · Esc cancels",
            Phase::Opening => "Opening session · Esc detaches",
            Phase::Archiving => "Archiving selected session · Esc detaches",
            _ => "h newer · l older · g newest · G oldest · r refresh",
        });
        let Some(picker) = &mut self.picker else {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "Carlos",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(hint),
                    Line::from(""),
                    Line::from(Span::styled("Esc exits", Style::default().fg(COLOR_DIM))),
                ])
                .style(Style::default().fg(COLOR_TEXT).bg(COLOR_OVERLAY))
                .block(Block::default().borders(Borders::ALL)),
                frame.area(),
            );
            return;
        };
        let layout = compute_picker_layout(TerminalSize {
            width: frame.area().width as usize,
            height: frame.area().height as usize,
        });
        let height = layout.list_h.saturating_sub(1).max(1);
        picker.top = picker.top.min(picker.selected);
        if picker.selected >= picker.top + height {
            picker.top = picker.selected + 1 - height;
        }
        let rows: Vec<_> = picker
            .page
            .rows
            .iter()
            .map(|row| PickerRow {
                id: row.choice.as_str(),
                name: row.name.as_deref(),
                preview: &row.preview,
                cwd: &row.cwd,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
            .collect();
        let target = picker.confirm.then(|| rows.get(picker.selected)).flatten();
        draw_picker_rows(
            frame,
            &rows,
            picker.selected,
            picker.top,
            picker.page.can_archive,
            target,
            Some(hint),
        );
    }
}
