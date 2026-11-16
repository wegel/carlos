//! Draw the existing native queue widget from one bounded preview page and independent editor.

use ratatui::buffer::Buffer;

use super::Device;
use crate::app::codex_queue_view::{cursor_for, draw_panel, editable};
use crate::app::view_models::TerminalSize;

impl Device {
    /// Render accepted facts beside only this terminal's selected id and local editor.
    pub(super) fn draw_queue(&self, buffer: &mut Buffer, size: TerminalSize) -> (usize, usize) {
        let editor = self
            .queue
            .editing
            .draft
            .as_ref()
            .map(|editor| &editor.editor);
        draw_panel(buffer, editor, self.queue_lines(size), size);
        cursor_for(editor, size)
    }

    fn queue_lines(&self, size: TerminalSize) -> Vec<String> {
        let facts = self.queue_facts();
        let current = self.client.queue_is_current(&self.session);
        let mut lines = vec!["Codex queue · Esc back".into()];
        if !editable(size) {
            lines.push("Resize to edit".into());
        }
        let status = if self.queue_panel().is_some_and(|panel| panel.is_reading())
            || facts.is_some_and(|facts| facts.reading)
            || !current && facts.is_some_and(|facts| facts.known)
        {
            "Refreshing"
        } else if current {
            "Current"
        } else {
            "Unknown"
        };
        let count = facts.map_or(0, |facts| facts.count);
        lines.push(format!("{status}: {count} queued"));
        lines.push(format!(
            "Composer mode: {}",
            if self.view.codex_queue.mode {
                "Queue"
            } else {
                "Steer"
            }
        ));
        lines.push(format!(
            "Selected {}/{}: {}",
            self.queue_panel()
                .and_then(|panel| panel.page().position())
                .filter(|_| current)
                .map_or(0, |index| index + 1),
            count,
            self.selected_queue_display_id()
        ));
        self.queue_notices(&mut lines);
        self.queue_selected_lines(&mut lines, current);
        lines
    }

    fn selected_queue_display_id(&self) -> String {
        let selected = self.view.codex_queue.selected.as_deref();
        self.queue_panel()
            .and_then(|panel| panel.page().selected(selected))
            .and_then(|entry| entry.display_id.as_deref())
            .map(str::to_owned)
            .or_else(|| selected.map(short_id))
            .unwrap_or_else(|| "none".into())
    }

    fn queue_notices(&self, lines: &mut Vec<String>) {
        let facts = self.queue_facts();
        if let Some(error) = self
            .view
            .codex_queue
            .error
            .as_deref()
            .or_else(|| self.queue_panel().and_then(|panel| panel.input_error()))
            .or_else(|| self.queue_panel().and_then(|panel| panel.read_error()))
            .or_else(|| facts.and_then(|facts| facts.read_error.as_deref()))
        {
            lines.push(error.into());
        } else if let Some(notice) = facts
            .map(|facts| facts.notice.as_str())
            .filter(|notice| !notice.is_empty())
        {
            lines.push(notice.into());
        }
        if self.queue_pending() {
            lines.push("Queue request pending; Codex has not acknowledged it".into());
        }
        if self
            .queue_panel()
            .is_some_and(|panel| panel.is_reading_input())
        {
            lines.push("Reading full selected input; the partial preview cannot be edited".into());
        }
    }

    fn queue_selected_lines(&self, lines: &mut Vec<String>, current: bool) {
        if current {
            if let Some(entry) = self
                .queue_panel()
                .and_then(|panel| panel.page().selected(panel.selection()))
            {
                lines.push(format!("Client: {}", entry.client_message));
                lines.push(entry.preview.clone());
                if !entry.preview_complete {
                    lines.push("[Preview only; full native input is retained]".into());
                }
            } else if self.view.codex_queue.selected.is_some() {
                lines.push("Selected entry left the queue; arrows choose another".into());
            }
        }
        if self.view.codex_queue.recovery.is_some() {
            lines
                .push("Unsent private draft retained; v recovers it into an empty composer".into());
        }
        if let Some(status) = self.disconnect_hint() {
            lines.insert(1, status);
        }
    }

    /// Explain the local Enter policy and compact native queue notices in the ordinary footer.
    pub(super) fn queue_status(&self, status: String) -> String {
        let mode = if self.view.codex_queue.mode {
            Some("Queue")
        } else if self.view.viewport.steer_notice_until.is_some() {
            Some("Steer")
        } else {
            None
        };
        let notice = if self.queue_pending() {
            Some("Queue pending".to_string())
        } else if self.view.codex_queue.recovery.is_some() {
            Some("Unsent draft".into())
        } else {
            self.queue_facts()
                .filter(|facts| facts.known && facts.count > 0)
                .map(|facts| format!("{} queued", facts.count))
        };
        let mut result = status;
        for text in mode.into_iter().map(str::to_string).chain(notice) {
            result.push_str(" · ");
            result.push_str(&text);
        }
        result
    }
}

fn short_id(id: &str) -> String {
    let prefix: String = id.chars().take(12).collect();
    if prefix.len() < id.len() {
        format!("{prefix}…")
    } else {
        prefix
    }
}
