//! Retain private results until the manager collects them, keeping admission charged meanwhile.

use std::collections::HashMap;

use carlos_proto::ids::SessionId;
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState, SubmittedText};

use super::mailbox::Permit;

struct Record {
    result: OperationResult,
    dirty: bool,
    _permit: Permit,
}

#[derive(Default)]
pub(super) struct Results {
    records: HashMap<ClientOperation, Record>,
}

impl Results {
    pub fn admit(
        &mut self,
        session: &SessionId,
        operation: ClientOperation,
        permit: Permit,
    ) -> OperationResult {
        let result = OperationResult {
            operation: operation.clone(),
            session: Some(session.clone()),
            state: OperationState::Pending,
        };
        self.records.insert(
            operation,
            Record {
                result: result.clone(),
                dirty: true,
                _permit: permit,
            },
        );
        result
    }

    pub fn contains(&self, operation: &ClientOperation) -> bool {
        self.records.contains_key(operation)
    }

    pub fn update(&mut self, operation: &ClientOperation, mut state: OperationState) {
        match &mut state {
            OperationState::Rejected { message, .. }
            | OperationState::Uncertain { message, .. } => {
                *message = crate::protocol_events::bounded_diagnostic(message)
            }
            OperationState::Accepted {
                notice: Some(notice),
                ..
            } => *notice = crate::protocol_events::bounded_diagnostic(notice),
            OperationState::Pending
            | OperationState::Queued
            | OperationState::Accepted { notice: None, .. } => {}
        }
        if let Some(record) = self.records.get_mut(operation) {
            record.result.state = state;
            record.dirty = true;
        }
    }

    pub fn collect(&mut self) -> Vec<OperationResult> {
        let results = self
            .records
            .values_mut()
            .filter(|record| record.dirty)
            .map(|record| {
                record.dirty = false;
                record.result.clone()
            })
            .collect();
        self.records.retain(|_, record| {
            matches!(
                record.result.state,
                OperationState::Pending | OperationState::Queued
            )
        });
        results
    }

    pub fn recover(&mut self, draft: SubmittedText, message: impl Into<String>, uncertain: bool) {
        if let Some(operation) = draft.operation.clone() {
            let message = message.into();
            let recovery = Some(draft);
            self.update(
                &operation,
                if uncertain {
                    OperationState::Uncertain { message, recovery }
                } else {
                    OperationState::Rejected { message, recovery }
                },
            );
        }
    }
}
