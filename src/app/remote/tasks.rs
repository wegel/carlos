//! Keep each terminal's task panel, selected opaque id and mode independent of daemon facts.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use anyhow::Result;
use carlos_client::Effects;
use carlos_proto::commands::SendPolicy;

use carlos_proto::session::Backend;
use carlos_proto::task_facts::TaskFacts;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::tasks_stops::Stops;
use super::transport::Transport;
use super::Device;
use crate::app::task_panel_rows::PanelRows;

#[derive(Default)]
pub(super) struct TaskView {
    pub(super) show: bool,
    pub(super) scroll: usize,
    pub(super) error: Option<String>,
    pub(super) queue_mode: bool,
    pub(super) overview_next_draw: Option<Instant>,
}

/// A device retains only bounded display data and the private operations it initiated.
#[derive(Default)]
pub(super) struct Tasks {
    pub(super) activity: super::tasks_activity::ChildActivity,
    pub(super) view: TaskView,
    pub(super) stops: Stops,
    pub(super) effects: Vec<Effects>,
    pub(super) paint: RefCell<PanelRows>,
    pub(super) paint_dirty: Cell<bool>,
}

impl Device {
    /// Borrow task headers from the shared Client's single accepted session copy.
    pub(super) fn task_facts(&self) -> Option<&TaskFacts> {
        self.client.session(&self.session)?.tasks()
    }

    pub(super) fn task_backend(&self) -> Option<Backend> {
        Some(self.client.session(&self.session)?.info().backend)
    }

    pub(super) fn task_count(&self) -> usize {
        self.client
            .task_panel(&self.session)
            .map_or(0, |panel| panel.count())
    }

    /// Open explicit native task interest while preserving any underlying composer or editor.
    pub(super) fn open_tasks(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.tasks.paint_dirty.set(true);
        self.tasks.view.show = true;
        self.tasks.view.scroll = 0;
        self.tasks.view.error = None;
        let effects = self.client.open_task_panel(&self.session, None)?;
        self.apply_or_queue_task_effects(link, effects)?;
        Ok(())
    }

    fn close_tasks(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.close_task_activity(false)?;
        self.tasks.view.show = false;
        if self.task_window_id().is_none() {
            self.clear_task_details();
        }
        let _ = self.client.close_task_overview(&self.session);
        let effects = self.client.close_task_panel(&self.session);
        self.apply_or_queue_task_effects(link, effects)?;
        Ok(())
    }

    /// Retire only unfinished read rights; selected rows and private stop results survive EOF.
    pub(super) fn tasks_disconnected(&mut self) {
        self.tasks.paint_dirty.set(true);
        self.tasks.effects.clear();
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
                self.toggle_claude_queue_mode();
                return Ok(true);
            }
            if self.tasks.view.show {
                return self.queue_key(link, key);
            }
        }
        if !self.tasks.view.show {
            return Ok(false);
        }
        if key.code == KeyCode::F(4) {
            self.close_tasks(link)?;
            self.next_question();
            return Ok(true);
        }
        self.task_panel_key(link, key)
    }

    fn task_panel_key(&mut self, link: Option<&mut Transport>, key: KeyEvent) -> Result<bool> {
        match (key.code, key.modifiers) {
            (KeyCode::Enter, _) => self.open_task_activity(link)?,
            (KeyCode::Esc, _) if self.tasks.activity.show => self.close_task_activity(true)?,
            (KeyCode::Esc, _) => self.close_tasks(link)?,
            (KeyCode::Up, _) => {
                if self.tasks.activity.show {
                    self.close_task_activity(true)?;
                }
                self.select_task(-1)
            }
            (KeyCode::Down, _) => {
                if self.tasks.activity.show {
                    self.close_task_activity(true)?;
                }
                self.select_task(1)
            }
            (KeyCode::PageUp, _) => {
                self.tasks.view.scroll = self.tasks.view.scroll.saturating_sub(5)
            }
            (KeyCode::PageDown, _) => {
                self.tasks.view.scroll = self.tasks.view.scroll.saturating_add(5)
            }
            (KeyCode::Char('r' | 'l'), KeyModifiers::NONE) if self.tasks.activity.show => {
                let link = link.ok_or_else(|| {
                    anyhow::anyhow!("Reconnect before reading native child history")
                })?;
                self.advance_task_activity(link, key.code == KeyCode::Char('l'))?;
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => self.refresh_tasks(),
            (KeyCode::Char('l'), KeyModifiers::NONE) => self.open_older_tasks(link)?,
            (KeyCode::Char('x'), KeyModifiers::NONE) => self.begin_task_stop(link)?,
            _ => {}
        }
        Ok(true)
    }

    fn select_task(&mut self, direction: isize) {
        self.tasks.paint_dirty.set(true);
        match self.client.move_task_panel(&self.session, direction) {
            Ok(effects) => self.tasks.effects.push(effects),
            Err(error) => {
                self.tasks.view.error = Some(error.to_string());
                return;
            }
        }
        self.tasks.view.scroll = 0;
        self.tasks.view.error = None;
        self.clear_task_details();
    }

    fn refresh_tasks(&mut self) {
        self.tasks.paint_dirty.set(true);
        match self.client.refresh_task_panel(&self.session) {
            Ok(effects) => self.tasks.effects.push(effects),
            Err(error) => {
                self.tasks.view.error = Some(error.to_string());
                return;
            }
        }
        self.clear_task_details();
        let _ = self.client.close_task_overview(&self.session);
        self.tasks.view.overview_next_draw = None;
        self.tasks.view.error = None;
    }

    /// Preserve Claude's explicit native Queue mode independently of the Codex queue editor.
    fn toggle_claude_queue_mode(&mut self) {
        self.tasks.view.queue_mode = !self.tasks.view.queue_mode;
        self.view.viewport.steer_notice_until =
            (!self.tasks.view.queue_mode).then(|| Instant::now() + Duration::from_secs(2));
    }

    /// Choose the captured Enter policy without changing another device's queue mode.
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
