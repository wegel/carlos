//! Submit only explicit revision-checked queue actions and recover their originating private drafts.

use anyhow::{bail, Result};
use carlos_proto::commands::Command;
use carlos_proto::ids::QueueRevision;
use carlos_proto::queue::{QueueChange, QueueDirection, MAX_TEXT_BYTES};
use carlos_proto::submissions::ClientOperation;

use super::queue_outcomes::{OwnedQueue, QueueOwner};
use super::transport::Transport;
use super::Device;
use crate::app::input::make_input_area;

/// Map only explicit selected-row controls to their native queue command.
pub(super) enum QueueControl {
    Move(QueueDirection),
    Start,
    Delete,
}

impl Device {
    /// Require a complete current preview before changing a selected entry.
    pub(super) fn require_current_queue(&self) -> Result<()> {
        let revision = self.queue_revision()?;
        if self.queue.refresh
            || self.queue.reads.busy()
            || !self.queue.pages.settled
            || self.queue.pages.revision.as_ref() != Some(&revision)
        {
            bail!("Select a current queued message after the queue finishes refreshing");
        }
        Ok(())
    }

    fn queue_revision(&self) -> Result<QueueRevision> {
        if self.queue_pending() {
            bail!("Finish or recover the previous queue request before changing another");
        }
        self.queue_facts()
            .filter(|facts| facts.known)
            .map(|facts| facts.revision.clone())
            .ok_or_else(|| {
                anyhow::anyhow!("Queue changed or is unknown; refresh before submitting this draft")
            })
    }

    /// Add the composer to the native queue without dispatching or echoing an ordinary chat prompt.
    pub(super) fn submit_queue(&mut self, link: &mut Transport, text: String) -> Result<()> {
        if self.view.codex_queue.recovery.is_some() {
            bail!("Finish or recover the previous queue request before adding another");
        }
        check_text(&text)?;
        let expected = self.queue_revision()?;
        let operation = self.send_queue_change(
            link,
            expected,
            QueueChange::Add { text },
            QueueOwner::Add { cleared: false },
        )?;
        self.view.input = make_input_area();
        self.queue
            .operations
            .get_mut(&operation)
            .expect("submitted queue input")
            .owner = QueueOwner::Add { cleared: true };
        Ok(())
    }

    /// Save only the exact native revision from which this independent full-text editor was opened.
    pub(super) fn save_queue_edit(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.require_current_queue()?;
        let Some(editor) = self.queue.editing.draft.as_ref() else {
            return Ok(());
        };
        if Some(&editor.expected) != self.queue_facts().map(|facts| &facts.revision) {
            bail!("Queue changed; this draft is preserved. Refresh and reselect before saving");
        }
        let text = editor.editor.lines().join("\n");
        check_text(&text)?;
        let change = QueueChange::Edit {
            entry: editor.entry.clone(),
            text,
        };
        let expected = editor.expected.clone();
        let owner = QueueOwner::Edit(editor.version);
        let link = link.ok_or_else(|| anyhow::anyhow!("Disconnected; F5 reconnects"))?;
        self.send_queue_change(link, expected, change, owner)?;
        Ok(())
    }

    /// Start, drop or move only the visibly selected current opaque entry id.
    pub(super) fn operate_queue(
        &mut self,
        link: Option<&mut Transport>,
        action: QueueControl,
    ) -> Result<()> {
        self.require_current_queue()?;
        let entry = self
            .queue
            .pages
            .selected(self.view.codex_queue.selected.as_deref())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Select a current queued message after the queue finishes refreshing"
                )
            })?
            .id
            .clone();
        let change = match action {
            QueueControl::Move(direction) => QueueChange::Move { entry, direction },
            QueueControl::Start => QueueChange::Start { entry },
            QueueControl::Delete => QueueChange::Delete { entry },
        };
        let expected = self.queue_revision()?;
        let link = link.ok_or_else(|| anyhow::anyhow!("Disconnected; F5 reconnects"))?;
        self.send_queue_change(link, expected, change, QueueOwner::Control)?;
        Ok(())
    }

    fn send_queue_change(
        &mut self,
        link: &mut Transport,
        expected: QueueRevision,
        change: QueueChange,
        owner: QueueOwner,
    ) -> Result<ClientOperation> {
        let session = self.session.clone();
        if !self.client.session_is_ready(link.connection, &session) {
            bail!("The daemon is refreshing this conversation; your queue draft is kept");
        }
        let mut identity = None;
        let outgoing = self.client.submit(link.connection, |operation| {
            identity = Some(operation.clone());
            Command::QueueChange {
                session,
                operation,
                expected,
                change,
            }
        })?;
        let operation = identity.expect("allocated queue operation");
        self.queue.operations.insert(
            operation.clone(),
            OwnedQueue {
                owner,
                recovered: false,
                clear_editor: true,
            },
        );
        self.view.codex_queue.error = None;
        let id = outgoing.id();
        if let Err(error) = link.send(outgoing) {
            let effects = self
                .client
                .write_failed(link.connection, id, &error.to_string(), false);
            self.apply_operations(&effects.operations);
            return Err(error);
        }
        Ok(operation)
    }

    /// Move one retained failed input only into an empty composer after explicit recovery.
    pub(super) fn recover_queue_draft(&mut self) -> Result<()> {
        if !self.view.input.is_empty() {
            bail!("Clear or save the main draft before recovering another");
        }
        if let Some(text) = self.view.codex_queue.recovery.take() {
            self.view.input.insert_str(&text);
        }
        self.refill_queue_recovery();
        self.view.codex_queue.show = false;
        Ok(())
    }
}

fn check_text(text: &str) -> Result<()> {
    if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES {
        bail!("Queued text must be nonempty and at most 512 KiB; the draft is kept");
    }
    Ok(())
}
