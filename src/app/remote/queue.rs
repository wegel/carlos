//! Keep native queue keys, selection and drafts local while the daemon owns queued work.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use carlos_client::panels::{QueuePanel, QueueReadMode};
use carlos_client::Effects;
use carlos_proto::ids::{QueueEntryId, QueueRevision};
use carlos_proto::queue::{QueueDirection, QueueFacts};
use carlos_proto::session::Backend;
use carlos_proto::submissions::{ClientOperation, OperationState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::TextArea;

use super::queue_commands::QueueControl;
use super::queue_editor::Editors;
use super::queue_outcomes::OwnedQueue;
use super::transport::Transport;
use super::Device;
use crate::app::codex_queue_view::editable;
use crate::app::render_input::textarea_input_from_key;
use crate::app::view_models::TerminalSize;

/// Freeze the full plain text and native revision of one independent editor.
pub(super) struct QueueEditor {
    pub(super) entry: QueueEntryId,
    pub(super) expected: QueueRevision,
    pub(super) editor: TextArea<'static>,
    pub(super) version: u64,
}

/// Keep page reads and private operation ownership separate from the shared queue facts.
#[derive(Default)]
pub(super) struct Queue {
    pub(super) editing: Editors,
    pub(super) operations: HashMap<ClientOperation, OwnedQueue>,
    pub(super) size: Option<TerminalSize>,
}

impl Device {
    /// Borrow the one accepted compact queue state held by the shared Client.
    pub(super) fn queue_facts(&self) -> Option<&QueueFacts> {
        self.client.session(&self.session)?.queue()
    }

    /// Lock new queue changes only while a native or privately submitted change is pending.
    pub(super) fn queue_pending(&self) -> bool {
        self.queue.operations.keys().any(|id| {
            self.client.operations().get(id).is_some_and(|record| {
                matches!(
                    record.state,
                    OperationState::Pending | OperationState::Queued
                )
            })
        }) || self.queue_facts().is_some_and(|facts| facts.pending)
    }

    /// Preserve the composer and selected identity while opening a fresh native queue read.
    pub(super) fn open_queue(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.view.codex_queue.show = true;
        self.refresh_queue(link)
    }

    /// Ask for a new snapshot without discarding an independent editor.
    pub(super) fn refresh_queue(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let effects = if self.queue_panel().is_some() {
            self.client.refresh_queue_panel(&self.session)?
        } else {
            let selected = self
                .view
                .codex_queue
                .selected
                .as_deref()
                .map(str::parse)
                .transpose()?;
            self.client
                .open_queue_panel(&self.session, selected, QueueReadMode::Refresh)?
        };
        self.view.codex_queue.error = None;
        self.queue_effects(link, effects)
    }

    /// Borrow shared read state while keeping sheet visibility and editor drafts in this terminal.
    pub(super) fn queue_panel(&self) -> Option<&QueuePanel> {
        self.client.queue_panel(&self.session)
    }

    /// Open a reader after a private operation reveals its panel; read scheduling stays in Client.
    pub(super) fn open_visible_queue_reader(&mut self, link: &mut Transport) -> Result<()> {
        if (self.view.codex_queue.show || self.view.codex_queue.mode)
            && self.queue_panel().is_none()
            && self.client.wants_session(&self.session)
        {
            self.refresh_queue(Some(link))?;
        }
        Ok(())
    }

    fn queue_effects(&mut self, link: Option<&mut Transport>, effects: Effects) -> Result<()> {
        if let Some(link) = link {
            self.apply_effects(link, effects)?;
        } else {
            self.apply_queue_model(&effects)?;
        }
        Ok(())
    }

    pub(super) fn close_hidden_queue_reader(&mut self) {
        if !(self.view.codex_queue.show || self.view.codex_queue.mode) {
            let _ = self.client.close_queue_panel(&self.session);
        }
    }

    /// Expire the terminal's Steer notice even while disconnected.
    pub(super) fn expire_queue_mode_notice(&mut self) -> bool {
        if self
            .view
            .viewport
            .steer_notice_until
            .is_some_and(|until| until <= Instant::now())
        {
            self.view.viewport.steer_notice_until = None;
            return true;
        }
        false
    }

    /// Consume queue shortcuts and editor keys before approvals and ordinary chat.
    pub(super) fn queue_key(
        &mut self,
        link: Option<&mut Transport>,
        key: KeyEvent,
    ) -> Result<bool> {
        if !self.view.codex_queue.show {
            if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::ALT) {
                self.open_queue(link)?;
                return Ok(true);
            }
            if key.code == KeyCode::F(3) && key.modifiers.is_empty() && self.codex_queue_backend() {
                self.toggle_queue_mode(link)?;
                return Ok(true);
            }
            return Ok(false);
        }
        if key.code == KeyCode::F(3) && key.modifiers.is_empty() {
            self.toggle_queue_mode(link)?;
        } else if key.code == KeyCode::Esc {
            self.queue_back();
        } else if self.queue.editing.draft.is_some() {
            self.queue_editor_key(link, key)?;
        } else if !self.queue_panel().is_some_and(QueuePanel::is_reading_input) {
            self.queue_list_key(link, key)?;
        }
        Ok(true)
    }

    fn codex_queue_backend(&self) -> bool {
        self.client
            .session(&self.session)
            .is_some_and(|replica| replica.info().backend == Backend::Codex)
    }

    fn toggle_queue_mode(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.view.codex_queue.mode = !self.view.codex_queue.mode;
        if self.view.codex_queue.mode && self.queue_panel().is_none() {
            let selected = self
                .view
                .codex_queue
                .selected
                .as_deref()
                .map(str::parse)
                .transpose()?;
            let effects =
                self.client
                    .open_queue_panel(&self.session, selected, QueueReadMode::Cached)?;
            self.queue_effects(link, effects)?;
        }
        self.close_hidden_queue_reader();
        self.view.viewport.steer_notice_until =
            (!self.view.codex_queue.mode).then(|| Instant::now() + Duration::from_secs(2));
        Ok(())
    }

    fn queue_back(&mut self) {
        let loading = self.queue_panel().is_some_and(QueuePanel::is_reading_input);
        let _ = self.client.cancel_queue_input(&self.session);
        if self.queue.editing.draft.take().is_none() && !loading {
            self.view.codex_queue.show = false;
        }
        self.close_hidden_queue_reader();
    }

    fn queue_editor_key(&mut self, link: Option<&mut Transport>, key: KeyEvent) -> Result<()> {
        if self.queue_pending() || !self.queue.size.is_some_and(editable) {
            return Ok(());
        }
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.save_queue_edit(link)?;
        } else if let Some(editor) = self.queue.editing.draft.as_mut() {
            editor.editor.input(textarea_input_from_key(key));
        }
        Ok(())
    }

    fn queue_list_key(&mut self, link: Option<&mut Transport>, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Up, KeyModifiers::ALT) => {
                self.operate_queue(link, QueueControl::Move(QueueDirection::Up))?
            }
            (KeyCode::Down, KeyModifiers::ALT) => {
                self.operate_queue(link, QueueControl::Move(QueueDirection::Down))?
            }
            (KeyCode::Up, _) => self.navigate_queue(link, -1)?,
            (KeyCode::Down, _) => self.navigate_queue(link, 1)?,
            (KeyCode::Char('e'), _) => self.begin_queue_edit(link)?,
            (KeyCode::Char('d'), _) => self.operate_queue(link, QueueControl::Delete)?,
            (KeyCode::Char('s'), _) => self.operate_queue(link, QueueControl::Start)?,
            (KeyCode::Char('r'), _) => self.refresh_queue(link)?,
            (KeyCode::Char('v'), _) => self.recover_queue_draft()?,
            _ => {}
        }
        Ok(())
    }

    fn navigate_queue(&mut self, link: Option<&mut Transport>, direction: isize) -> Result<()> {
        let effects = self.client.move_queue_panel(&self.session, direction)?;
        self.queue_effects(link, effects)
    }

    /// Consume queue paste without writing to the composer or a hidden editor.
    pub(super) fn queue_paste(&mut self, text: &str) {
        if self.queue_pending() || !self.queue.size.is_some_and(editable) {
            return;
        }
        if let Some(editor) = self.queue.editing.draft.as_mut() {
            editor.editor.insert_str(text);
        }
    }
}

#[cfg(test)]
#[path = "queue_contract_tests.rs"]
mod contract_tests;
