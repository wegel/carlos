//! Recover only the originating terminal's edit and distinguish its adopted fork from a failed fork.

use anyhow::{bail, Result};
use carlos_proto::commands::Command;
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::transport::Transport;
use super::Device;

impl Device {
    pub(super) fn submit_rewind(
        &mut self,
        link: Option<&mut Transport>,
        text: String,
    ) -> Result<()> {
        if self.view.codex_queue.recovery.is_some() {
            bail!("Finish or recover the Codex queue request before editing earlier history. This edited prompt is preserved.");
        }
        if self.history.operation.is_some() {
            bail!("Preparing edited branch; newer composer draft kept");
        }
        if self.history.pending.is_some() {
            bail!("Reading the selected prompt; edited draft kept");
        }
        let link =
            link.ok_or_else(|| anyhow::anyhow!("Disconnected; edited draft kept; F5 reconnects"))?;
        if !self.client.session_is_ready(link.connection, &self.session) {
            bail!("Waiting for current ready session facts; edited draft kept");
        }
        let entry = self
            .history
            .selection
            .as_ref()
            .and_then(|selection| selection.record.as_ref())
            .map(|record| record.id.clone())
            .ok_or_else(|| {
                anyhow::anyhow!("Choose a saved prompt with Up; F4 selects unknown sources")
            })?;
        let session = self.session.clone();
        let mut submitted = None;
        let outgoing = self.client.submit(link.connection, |operation| {
            submitted = Some(operation.clone());
            Command::Rewind {
                session,
                operation,
                entry,
                text,
            }
        })?;
        self.history.operation = submitted;
        self.history.failed = false;
        self.view.status_hint = None;
        self.view.viewport.auto_follow_bottom = true;
        self.reading.anchor = None;
        link.send(outgoing)?;
        self.set_composer("");
        Ok(())
    }

    pub(super) fn apply_history_operation(&mut self, id: &ClientOperation) -> bool {
        if self.history.operation.as_ref() != Some(id) {
            return false;
        }
        let Some(record) = self.client.operations().get(id) else {
            return true;
        };
        let rewound = record.rewound;
        let failed = match &record.state {
            OperationState::Pending | OperationState::Queued => return true,
            OperationState::Accepted { .. } => false,
            OperationState::Rejected { .. } | OperationState::Uncertain { .. } => true,
        };
        self.history.operation = None;
        if !failed || rewound {
            self.history.rewind_draft = None;
            self.history.reset_navigation();
            self.view.viewport.auto_follow_bottom = true;
        }
        self.history.failed = failed;
        if failed {
            self.view.status_hint = None;
            self.retain_failed_text(id);
        } else {
            self.client.forget_operation(id);
        }
        true
    }

    pub(super) fn retain_failed_text(&mut self, id: &ClientOperation) {
        let Some(text) = self
            .client
            .operations()
            .get(id)
            .and_then(|record| record.text())
        else {
            return;
        };
        if self.view.input.is_empty() {
            self.view.input.insert_str(text);
            self.client.forget_operation(id);
        } else if !self.history.recoveries.contains(id) {
            self.history.recoveries.push_back(id.clone());
        }
    }

    pub(super) fn apply_failed_draft(&mut self, id: &ClientOperation) -> bool {
        let Some(record) = self.client.operations().get(id) else {
            return false;
        };
        let message = match &record.state {
            OperationState::Rejected { message, .. }
            | OperationState::Uncertain { message, .. }
                if record.text().is_some() =>
            {
                message.clone()
            }
            _ => return false,
        };
        self.view.status_hint = Some(message);
        self.retain_failed_text(id);
        true
    }

    pub(super) fn recover_saved_draft(&mut self) -> bool {
        if self.recover_rename() {
            return true;
        }
        if !self.view.input.is_empty() {
            if self.history.recoveries.is_empty() {
                return false;
            }
            self.view.status_hint =
                Some("Copy or clear the current draft before F5 loads a saved draft".into());
            return true;
        }
        while let Some(id) = self.history.recoveries.pop_front() {
            if let Some(text) = self
                .client
                .operations()
                .get(&id)
                .and_then(|record| record.text())
            {
                self.view.input.insert_str(text);
                self.client.forget_operation(&id);
                return true;
            }
        }
        false
    }

    pub(super) fn history_hint(&self, width: usize) -> Option<&'static str> {
        if self.history.operation.is_some() {
            Some(if width < 30 {
                "Editing…"
            } else {
                "Preparing edited branch…"
            })
        } else if self.history.failed {
            Some(if width < 30 {
                "Edit failed"
            } else {
                "Edit failed · draft kept"
            })
        } else if !self.history.recoveries.is_empty() {
            Some("F5: saved drafts")
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "history_recovery_contract_tests.rs"]
mod recovery_tests;
