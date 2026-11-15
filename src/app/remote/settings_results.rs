//! Settle original private Save results without retrying an uncertain native write.

use carlos_proto::submissions::{ClientOperation, OperationState};

use super::settings_intent::Step;
use super::Device;

impl Device {
    pub(super) fn apply_settings_operation(&mut self, operation: &ClientOperation) -> bool {
        if self.settings.operation.as_ref() != Some(operation) {
            return false;
        }
        let Some(record) = self.client.operations().get(operation) else {
            return true;
        };
        match record.state.clone() {
            OperationState::Accepted { notice, .. } => {
                let intent = self.settings.submitted.as_mut().expect("submitted save");
                intent.notice(notice);
                if let Some(Step::Context(choice)) = intent.step.take() {
                    self.context_saved(choice);
                }
            }
            OperationState::Rejected { message, .. } => {
                let intent = self.settings.submitted.as_mut().expect("submitted save");
                if matches!(intent.step.take(), Some(Step::Context(_))) {
                    self.settings.submitted = None;
                    self.settings.keep_draft = true;
                    self.view.status_hint = Some(message.clone());
                    self.settings.error = Some(message);
                } else {
                    intent.error.get_or_insert(message);
                }
            }
            OperationState::Uncertain { message, .. } => {
                self.settings.error = Some(format!(
                    "{message}. Draft kept; F6 refreshes facts without resending"
                ));
                self.view.status_hint.clone_from(&self.settings.error);
                return true;
            }
            OperationState::Pending | OperationState::Queued => return true,
        }
        self.settings.operation = None;
        self.client.forget_operation(operation);
        true
    }
}
