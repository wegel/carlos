//! Release handled private outcomes while retaining unconfirmed text and newer editor drafts.

use carlos_proto::submissions::{ClientOperation, OperationState};

use super::Device;

impl Device {
    pub(super) fn apply_operations(&mut self, operations: &[ClientOperation]) {
        for id in operations {
            if self.apply_settings_operation(id)
                || self.apply_task_operation(id)
                || self.apply_queue_operation(id)
                || self.apply_goal_operation(id)
            {
                continue;
            }
            let Some(record) = self.client.operations().get(id) else {
                continue;
            };
            let handled = match &record.state {
                OperationState::Accepted { .. } => {
                    record.text().is_none() || record.entry.is_some()
                }
                OperationState::Rejected { message, .. } => {
                    self.view.status_hint = Some(message.clone());
                    if let Some(text) = record.text() {
                        if self.view.input.is_empty() {
                            self.view.input.insert_str(text);
                            true
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                }
                OperationState::Uncertain { message, .. } => {
                    self.view.status_hint = Some(format!("{message}; submitted text retained"));
                    false
                }
                OperationState::Pending | OperationState::Queued => false,
            };
            if handled {
                self.client.forget_operation(id);
            }
        }
        if !operations.is_empty() {
            let length = self
                .client
                .session(&self.session)
                .map_or(0, |replica| replica.entry_count());
            self.dirty_from(length);
        }
    }
}
