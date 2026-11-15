//! Settle unconsumed commands from their retained exact text after the owner joins.

use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState, SubmittedText};

use super::{terminal, RetryStore};

impl RetryStore {
    /// Return private uncertainty without changing accepted results or lifecycle waiters.
    pub fn stopped(
        &self,
        session: &SessionId,
        waiters: &[ClientOperation],
    ) -> Vec<OperationResult> {
        self.records
            .values()
            .filter(|record| {
                record.result.session.as_ref() == Some(session)
                    && !terminal(&record.result.state)
                    && !waiters.contains(&record.result.operation)
            })
            .map(|record| {
                let text = match serde_json::from_slice::<Command>(&record.canonical) {
                    Ok(Command::Send { text, .. } | Command::Rewind { text, .. }) => Some(text),
                    Ok(Command::QueueChange { change, .. }) => change.text().map(str::to_owned),
                    _ => None,
                };
                let recovery = text.map(|text| SubmittedText {
                    text,
                    operation: Some(record.result.operation.clone()),
                });
                OperationResult {
                    state: OperationState::Uncertain {
                        message: "Session owner stopped before this command could be confirmed. Check the conversation before explicitly retrying.".into(),
                        recovery,
                    },
                    ..record.result.clone()
                }
            })
            .collect()
    }
}
