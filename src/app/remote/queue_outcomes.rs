//! Apply private queue outcomes without replacing another editor or discarding retained source.

use carlos_proto::submissions::{ClientOperation, OperationState};

use super::Device;

/// Remember which local source a queued command may clear or recover.
#[derive(Clone, Copy)]
pub(super) enum QueueOwner {
    Add { cleared: bool },
    Edit(u64),
    Control,
}

/// Keep only local editor rights; the shared Client retains exact submitted text.
pub(super) struct OwnedQueue {
    pub(super) owner: QueueOwner,
    pub(super) recovered: bool,
    pub(super) clear_editor: bool,
}

impl Device {
    /// Settle only this terminal's queue operations; no shared event can clear an editor.
    pub(super) fn apply_queue_operation(&mut self, id: &ClientOperation) -> bool {
        let Some(owned) = self.queue.operations.get(id) else {
            return false;
        };
        let owner = owned.owner;
        let clear_editor = owned.clear_editor;
        let Some(record) = self.client.operations().get(id) else {
            return true;
        };
        match &record.state {
            OperationState::Pending | OperationState::Queued => return true,
            OperationState::Accepted { .. } => self.queue_accepted(owner, clear_editor),
            OperationState::Rejected { message, .. }
            | OperationState::Uncertain { message, .. } => {
                let uncertain = matches!(record.state, OperationState::Uncertain { .. });
                let message = message.clone();
                if !self.queue_failed(id, owner, message, uncertain) {
                    return true;
                }
            }
        }
        self.queue.operations.remove(id);
        self.client.forget_operation(id);
        true
    }

    fn queue_accepted(&mut self, owner: QueueOwner, clear_editor: bool) {
        if let QueueOwner::Edit(version) = owner {
            if clear_editor
                && self
                    .queue
                    .editing
                    .draft
                    .as_ref()
                    .is_some_and(|edit| edit.version == version)
            {
                self.queue.editing.draft = None;
            }
        }
        self.view.codex_queue.error = None;
    }

    fn queue_failed(
        &mut self,
        id: &ClientOperation,
        owner: QueueOwner,
        message: String,
        uncertain: bool,
    ) -> bool {
        self.view.codex_queue.error = Some(message);
        self.view.codex_queue.show = true;
        let owned = self
            .queue
            .operations
            .get_mut(id)
            .expect("owned queue operation");
        if uncertain {
            owned.clear_editor = false;
        }
        let recovered = owned.recovered;
        if !recovered && !self.recover_queue_operation(id, owner) {
            return false;
        }
        if uncertain {
            self.queue
                .operations
                .get_mut(id)
                .expect("retained queue operation")
                .recovered = true;
        }
        !uncertain
    }

    /// Fill the next empty recovery slot from an owned failure after explicit user recovery.
    pub(super) fn refill_queue_recovery(&mut self) {
        let candidate = self.client.operations().iter().find_map(|record| {
            let owned = self.queue.operations.get(&record.operation)?;
            (!owned.recovered
                && matches!(
                    record.state,
                    OperationState::Rejected { .. } | OperationState::Uncertain { .. }
                ))
            .then(|| (record.operation.clone(), owned.owner))
        });
        let Some((id, owner)) = candidate else {
            return;
        };
        if !self.recover_queue_operation(&id, owner) {
            return;
        }
        if self
            .client
            .operations()
            .get(&id)
            .is_some_and(|record| matches!(record.state, OperationState::Uncertain { .. }))
        {
            let owned = self
                .queue
                .operations
                .get_mut(&id)
                .expect("retained queue operation");
            owned.recovered = true;
            owned.clear_editor = false;
        } else {
            self.queue.operations.remove(&id);
            self.client.forget_operation(&id);
        }
    }

    fn recover_queue_operation(&mut self, id: &ClientOperation, owner: QueueOwner) -> bool {
        if matches!(
            owner,
            QueueOwner::Control | QueueOwner::Add { cleared: false }
        ) {
            return true;
        }
        if matches!(owner, QueueOwner::Edit(version) if self.queue.editing.draft.as_ref().is_some_and(|edit| edit.version == version))
        {
            return true;
        }
        let Some(text) = self
            .client
            .operations()
            .get(id)
            .and_then(|record| record.text())
        else {
            return true;
        };
        if self.view.input.is_empty() {
            self.view.input.insert_str(text);
        } else if self.view.codex_queue.recovery.is_none() {
            self.view.codex_queue.recovery = Some(text.into());
        } else {
            return false;
        }
        true
    }
}
