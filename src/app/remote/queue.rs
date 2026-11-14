//! Keep native queue keys, selection and drafts local while the daemon owns queued work.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use carlos_proto::ids::{QueueEntryId, QueueRevision};
use carlos_proto::queue::QueueFacts;
use carlos_proto::session::Backend;
use carlos_proto::submissions::{ClientOperation, OperationState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::TextArea;

use super::queue_editor::Editors;
use super::queue_outcomes::OwnedQueue;
use super::queue_pages::Pages;
use super::queue_reads::Reads;
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
    pub(super) pages: Pages,
    pub(super) reads: Reads,
    pub(super) editing: Editors,
    pub(super) operations: HashMap<ClientOperation, OwnedQueue>,
    pub(super) refresh: bool,
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
    pub(super) fn open_queue(&mut self) {
        self.view.codex_queue.show = true;
        self.refresh_queue();
    }

    /// Ask for a new snapshot without discarding an independent editor.
    pub(super) fn refresh_queue(&mut self) {
        self.queue.refresh = true;
        self.queue.reads = Reads::default();
        self.view.codex_queue.error = None;
    }

    /// Drop only transport read tokens after EOF; editors and submitted commands remain private.
    pub(super) fn queue_disconnected(&mut self) {
        self.queue.refresh |= self.queue.reads.refreshing();
        self.queue.reads = Reads::default();
        self.queue.editing.loading = None;
    }

    /// Consume queue shortcuts and editor keys before approvals and ordinary chat.
    pub(super) fn queue_key(
        &mut self,
        link: Option<&mut Transport>,
        key: KeyEvent,
    ) -> Result<bool> {
        if !self.view.codex_queue.show {
            if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::ALT) {
                self.open_queue();
                return Ok(true);
            }
            if key.code == KeyCode::F(3) && key.modifiers.is_empty() && self.codex_queue_backend() {
                self.toggle_queue_mode();
                return Ok(true);
            }
            return Ok(false);
        }
        if key.code == KeyCode::F(3) && key.modifiers.is_empty() {
            self.toggle_queue_mode();
        } else if key.code == KeyCode::Esc {
            self.queue_back();
        } else if self.queue.editing.draft.is_some() {
            self.queue_editor_key(link, key)?;
        } else if self.queue.editing.loading.is_none() {
            self.queue_list_key(link, key)?;
        }
        Ok(true)
    }

    fn codex_queue_backend(&self) -> bool {
        self.client
            .session(&self.session)
            .is_some_and(|replica| replica.info().backend == Backend::Codex)
    }

    fn toggle_queue_mode(&mut self) {
        self.view.codex_queue.mode = !self.view.codex_queue.mode;
        if self.view.codex_queue.mode && self.queue_facts().is_none_or(|facts| !facts.known) {
            self.refresh_queue();
        }
        self.view.viewport.steer_notice_until =
            (!self.view.codex_queue.mode).then(|| Instant::now() + Duration::from_secs(2));
    }

    fn queue_back(&mut self) {
        if self.queue.editing.draft.take().is_none() && self.queue.editing.loading.take().is_none()
        {
            self.view.codex_queue.show = false;
        }
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
            (KeyCode::Up, KeyModifiers::ALT) => self.operate_queue(link, "up")?,
            (KeyCode::Down, KeyModifiers::ALT) => self.operate_queue(link, "down")?,
            (KeyCode::Up, _) => self.navigate_queue(-1),
            (KeyCode::Down, _) => self.navigate_queue(1),
            (KeyCode::Char('e'), _) => self.begin_queue_edit()?,
            (KeyCode::Char('d'), _) => self.operate_queue(link, "delete")?,
            (KeyCode::Char('s'), _) => self.operate_queue(link, "start")?,
            (KeyCode::Char('r'), _) => self.refresh_queue(),
            (KeyCode::Char('v'), _) => self.recover_queue_draft()?,
            _ => {}
        }
        Ok(())
    }

    fn navigate_queue(&mut self, direction: isize) {
        let count = self.queue_facts().map_or(0, |facts| facts.count as usize);
        self.queue
            .pages
            .navigate(direction, count, &mut self.view.codex_queue.selected);
        self.queue.reads.clear_failure();
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
