//! Keep each terminal's task panel, selected opaque id and mode independent of daemon facts.

use std::cell::{Cell, RefCell};

use anyhow::Result;
use carlos_proto::commands::SendPolicy;
use carlos_proto::ids::{Sequence, TaskId};
use carlos_proto::session::Backend;
use carlos_proto::task_facts::TaskFacts;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::tasks_details::Details;
use super::tasks_overview::Overview;
use super::tasks_pages::Pages;
use super::tasks_reads::Reads;
use super::tasks_stops::Stops;
use super::transport::Transport;
use super::Device;
use crate::app::task_panel_rows::PanelRows;

#[derive(Default)]
pub(super) struct TaskView {
    pub(super) show: bool,
    pub(super) selected: Option<TaskId>,
    pub(super) scroll: usize,
    pub(super) error: Option<String>,
    pub(super) queue_mode: bool,
}

/// A device retains only bounded display data and the private operations it initiated.
#[derive(Default)]
pub(super) struct Tasks {
    pub(super) view: TaskView,
    pub(super) pages: Pages,
    pub(super) reads: Reads,
    pub(super) details: Details,
    pub(super) overview: Overview,
    pub(super) stops: Stops,
    pub(super) refresh: bool,
    pub(super) paint: RefCell<PanelRows>,
    pub(super) paint_dirty: Cell<bool>,
}

impl Device {
    /// Borrow task headers from the shared Client's single accepted session copy.
    pub(super) fn task_facts(&self) -> Option<&TaskFacts> {
        self.client.session(&self.session)?.tasks()
    }

    pub(super) fn task_sequence(&self) -> Sequence {
        self.client
            .session(&self.session)
            .map_or(Sequence::default(), |replica| replica.task_sequence())
    }

    pub(super) fn task_backend(&self) -> Option<Backend> {
        Some(self.client.session(&self.session)?.info().backend)
    }

    pub(super) fn task_count(&self) -> usize {
        self.task_facts().map_or(0, |facts| {
            (facts.counts.agents + facts.counts.processes + facts.counts.tasks) as usize
        })
    }

    /// Open explicit native task interest while preserving any underlying composer or editor.
    pub(super) fn open_tasks(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.tasks.paint_dirty.set(true);
        self.tasks.view.show = true;
        self.tasks.view.scroll = 0;
        self.tasks.view.error = None;
        self.tasks.reads.header_error = None;
        let effects = self.client.watch_tasks(&self.session)?;
        self.tasks.reads.header = self.client.pending_task_read(&self.session);
        if let Some(link) = link {
            for outgoing in effects.outgoing {
                link.send(outgoing)?;
            }
        }
        Ok(())
    }

    fn close_tasks(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.tasks.view.show = false;
        self.tasks.reads.close();
        let effects = self.client.close_tasks(&self.session);
        if let Some(link) = link {
            for outgoing in effects.outgoing {
                link.send(outgoing)?;
            }
        }
        Ok(())
    }

    /// Retire only unfinished read rights; selected rows and private stop results survive EOF.
    pub(super) fn tasks_disconnected(&mut self) {
        self.tasks.paint_dirty.set(true);
        self.tasks.reads.disconnected();
        self.tasks.details.disconnected();
        self.tasks.overview.disconnected();
        if self.tasks.stops.disconnected() {
            self.tasks.view.error =
                Some("Native run read interrupted; x explicitly rereads before stopping".into());
        }
    }

    /// Route F2 and panel keys before any draft or callback widget can consume them.
    pub(super) fn task_key(&mut self, link: Option<&mut Transport>, key: KeyEvent) -> Result<bool> {
        if key.code == KeyCode::F(2) && key.modifiers.is_empty() {
            if self.tasks.view.show {
                self.close_tasks(link)?;
            } else {
                self.open_tasks(link)?;
            }
            return Ok(true);
        }
        if key.code == KeyCode::F(3) && key.modifiers.is_empty() {
            if self.task_backend() == Some(Backend::Claude) {
                self.tasks.view.queue_mode = !self.tasks.view.queue_mode;
                return Ok(true);
            }
            if self.tasks.view.show {
                return self.queue_key(link, key);
            }
        }
        if !self.tasks.view.show {
            return Ok(false);
        }
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => self.close_tasks(link)?,
            (KeyCode::Up, _) => self.select_task(-1),
            (KeyCode::Down, _) => self.select_task(1),
            (KeyCode::PageUp, _) => {
                self.tasks.view.scroll = self.tasks.view.scroll.saturating_sub(5)
            }
            (KeyCode::PageDown, _) => {
                self.tasks.view.scroll = self.tasks.view.scroll.saturating_add(5)
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => self.refresh_tasks(),
            (KeyCode::Char('x'), KeyModifiers::NONE) => self.begin_task_stop(link)?,
            _ => {}
        }
        Ok(true)
    }

    fn select_task(&mut self, direction: isize) {
        self.tasks.paint_dirty.set(true);
        let count = self.task_count();
        self.tasks.reads.page_changed();
        self.tasks
            .pages
            .navigate(direction, count, &mut self.tasks.view.selected);
        self.tasks.view.scroll = 0;
        self.tasks.view.error = None;
        self.clear_task_details();
    }

    fn refresh_tasks(&mut self) {
        self.tasks.paint_dirty.set(true);
        self.tasks.refresh = true;
        self.tasks.reads.clear();
        self.clear_task_details();
        let retired = self.tasks.overview.clear();
        self.tasks.reads.retire(retired);
        self.tasks.view.error = None;
    }

    /// Preserve Claude's explicit native Queue mode independently of the Codex queue editor.
    pub(super) fn task_send_policy(&self) -> SendPolicy {
        if self.task_backend() == Some(Backend::Claude) && self.tasks.view.queue_mode {
            SendPolicy::Queue
        } else {
            SendPolicy::Steer
        }
    }

    pub(super) fn task_status(&self, status: String) -> String {
        let active = self.task_facts().map_or(0, |facts| facts.counts.active);
        let status = if active > 0 {
            format!(
                "{status} · {active} task{}",
                if active == 1 { "" } else { "s" }
            )
        } else {
            status
        };
        if self.task_send_policy() == SendPolicy::Queue {
            format!("{status} · Queue (F3)")
        } else {
            status
        }
    }
}
