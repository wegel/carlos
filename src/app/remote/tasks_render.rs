//! Draw the shared task frame from one accepted header and bounded device-owned display fields.

use carlos_core::session::text::{safe_label, safe_text};
use carlos_proto::session::Backend;
use carlos_proto::tasks::{Task, TaskField, TaskKind};
use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::Device;
use crate::app::task_panel_view::{draw, TaskPanel};
use crate::app::view_models::TerminalSize;
use crate::theme::COLOR_PRIMARY;

impl Device {
    pub(super) fn draw_tasks(&self, buffer: &mut Buffer, size: TerminalSize) {
        let claude = self.task_backend() == Some(Backend::Claude);
        let selected = self.selected_task();
        let title = if claude {
            format!(
                "Tasks {}/{} · {} active",
                self.task_position().map_or(0, |index| index + 1),
                self.task_count(),
                self.task_facts().map_or(0, |facts| facts.counts.active)
            )
        } else if self.task_backend() == Some(Backend::Codex) {
            "Codex agents / processes".into()
        } else {
            "Native tasks unavailable".into()
        };
        let caption = self.task_caption(selected);
        let mut rows = self.tasks.paint.borrow_mut();
        if self.tasks.paint_dirty.replace(false) || rows.width() != size.width.max(1) {
            rows.rebuild(
                self.task_body(selected)
                    .into_iter()
                    .map(|line| safe_text(&line))
                    .collect(),
                size.width,
            );
        }
        let style = if claude {
            Style::default()
                .fg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_PRIMARY)
        };
        draw(
            buffer,
            TaskPanel {
                title: &title,
                title_style: style,
                selected: (!claude).then_some(caption.as_str()),
                body: &rows,
                scroll: self.tasks.view.scroll,
                footer: self.task_footer(size, selected),
                compact_help: (!claude).then_some("↑↓ select PgDn"),
            },
            size,
        );
    }

    fn task_caption(&self, selected: Option<&Task>) -> String {
        let Some(task) = selected else {
            return if !self.task_pages_settled() {
                "Reading native task page; selected id kept"
            } else if self.selected_task_id().is_some() {
                "Selected row unavailable; use arrows"
            } else {
                "No selected agent/process"
            }
            .into();
        };
        let label = if task.kind == TaskKind::Process {
            self.task_detail_field(TaskField::NativeId)
                .unwrap_or(&task.label)
        } else {
            &task.label
        };
        safe_label(&format!(
            "> {label}: {}",
            if task.kind == TaskKind::Process {
                "background process"
            } else {
                &task.state
            }
        ))
    }

    fn task_body(&self, selected: Option<&Task>) -> Vec<String> {
        let mut lines = self.task_notices();
        if let Some(notice) = selected
            .and_then(|task| self.task_stop_notice(task))
            .or_else(|| self.missing_task_stop_notice())
        {
            lines.push(notice);
        }
        let current = self.client.current_session_is_ready(&self.session)
            && (self.task_view_is_current() || self.task_window().is_some());
        if current {
            match selected {
                Some(task) => self.selected_task_body(task, &mut lines),
                None if self.task_pages_settled() => {
                    lines.push(if self.selected_task_id().is_some() {
                        "Selected row unavailable; use arrows to choose another".into()
                    } else {
                        "No native tasks reported".into()
                    })
                }
                None => lines.push("Reading native task page…".into()),
            }
        } else if !self.client.current_session_is_ready(&self.session) {
            lines.push("Waiting for accepted session facts; drafts and selected ids are kept. F5 reconnects".into());
        } else if matches!(self.task_backend(), Some(Backend::Codex | Backend::Claude)) {
            lines.push("Reading agents and background processes…".into());
        }
        if let Some(overview) = self.client.task_overview(&self.session) {
            lines.extend(super::tasks_text::overview(overview.entries()));
        }
        lines.extend(self.task_activity_lines());
        lines
    }

    fn selected_task_body(&self, task: &Task, lines: &mut Vec<String>) {
        if self
            .task_details()
            .and_then(|details| details.value())
            .is_some_and(|value| {
                value.task.id == task.id && Some(&value.revision) == self.task_page_revision()
            })
        {
            lines.extend(super::tasks_text::detail(
                self.task_details().expect("selected detail"),
                task.kind,
            ));
        } else {
            lines.push("Reading selected native details…".into());
        }
        if self
            .task_details()
            .is_some_and(|details| details.is_reading())
        {
            lines.push("Reading complete named task fields…".into());
        }
        if task.kind == TaskKind::Agent {
            if let Some(window) = self.task_window() {
                lines.push(format!(
                    "{} rows in this private native window; ↑↓ selects",
                    window.rows
                ));
                if window.older_available {
                    lines.push("Older native tasks exist".into());
                }
                return;
            }
            if let Some(facts) = self.task_facts() {
                lines.push(format!(
                    "{} agents / {} background processes; ↑↓ selects",
                    facts.counts.agents, facts.counts.processes
                ));
                if facts.native_older_available {
                    lines.push("Older native tasks exist".into());
                }
            }
        }
    }

    fn task_notices(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.task_window().is_some() {
            lines.push(if self.task_window_is_stale() {
                "Older native task window retired; displayed rows kept. r latest before controls"
            } else { "Private older native task window; l older, r latest" }.into());
        }
        if self.older_tasks_reading() {
            lines.push("Reading older tasks; current page kept…".into());
        }
        if let Some(error) = self
            .task_window()
            .and_then(|window| window.read_error.as_ref())
        {
            lines.push(error.clone());
        }
        for error in [
            self.tasks.view.error.as_deref(),
            self.client
                .task_panel(&self.session)
                .and_then(|panel| panel.read_error()),
            self.task_details().and_then(|details| details.read_error()),
            self.client
                .task_overview(&self.session)
                .and_then(|overview| overview.read_error()),
        ]
        .into_iter()
        .flatten()
        {
            lines.push(error.into());
        }
        if let Some(facts) = self.task_facts() {
            if let Some(error) = &facts.read_error {
                lines.push(error.clone());
            }
            if !facts.notice.is_empty() {
                lines.push(facts.notice.clone());
            }
            if facts.closed {
                lines.push("Native connection is closed; task controls are unavailable".into());
            }
        }
        if let Some(status) = self.disconnect_hint() {
            lines.push(status);
        }
        lines
    }

    fn task_footer(&self, size: TerminalSize, selected: Option<&Task>) -> &'static str {
        if self.tasks.activity.show {
            return if size.width < 24 {
                "Esc list F2 back"
            } else if size.width < 40 {
                "l older r new F2 back"
            } else {
                "↑↓ select · l older activity · r reload activity · x stop · PgDn · Esc list · F2 close"
            };
        }
        if self.task_backend() == Some(Backend::Claude) {
            let can_stop =
                selected.is_some() && self.task_detail_field(TaskField::CanStop) == Some("true");
            return match (size.width < 40, can_stop) {
                (true, true) => "↑↓ x stop F2 back",
                (true, false) => "↑↓ no stop F2 back",
                (false, true) => "↑↓ tasks · x stop · PgDn · F3 mode · F2/Esc close",
                (false, false) => "↑↓ tasks · stop unavailable · PgDn · F3 mode · F2/Esc close",
            };
        }
        if size.width < 24 {
            "l x F2 back"
        } else if size.width < 40 {
            "l older x F2 back"
        } else {
            "↑↓ select · Enter activity · x stop · l older · r latest · PgDn · F2/Esc close"
        }
    }
}
