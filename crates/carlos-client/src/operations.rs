//! Keep submitted text and private outcomes under their exact client operation, never an editor slot.

use carlos_proto::ids::{ClientId, EntryId, SessionId};
use carlos_proto::session::Entry;
use carlos_proto::submissions::{
    ClientOperation, OperationResult, OperationState, RecoveryKind, SubmittedText,
};

use crate::ClientError;

const MAX_OPERATIONS: usize = 128;
const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// Submitted text can be a new prompt, an earlier-prompt edit or a queued draft.
pub struct LocalText {
    pub kind: RecoveryKind,
    pub text: String,
}

/// One local submission; an attached shared row suppresses only this operation's optimistic echo.
pub struct LocalOperation {
    pub operation: ClientOperation,
    pub session: Option<SessionId>,
    pub state: OperationState,
    pub entry: Option<EntryId>,
    text: Option<LocalText>,
}

impl LocalOperation {
    /// Borrow original submitted text without copying it into another client's editor.
    pub fn text(&self) -> Option<&str> {
        self.text.as_ref().map(|draft| draft.text.as_str())
    }

    /// Show an optimistic prompt only until its shared row or a private failure arrives.
    pub fn needs_echo(&self) -> bool {
        self.text
            .as_ref()
            .is_some_and(|draft| draft.kind == RecoveryKind::Prompt)
            && self.entry.is_none()
            && matches!(
                self.state,
                OperationState::Pending | OperationState::Queued | OperationState::Accepted { .. }
            )
    }
}

/// Bound outstanding submissions; platforms explicitly forget handled records rather than lose drafts.
#[derive(Default)]
pub struct Operations {
    records: Vec<LocalOperation>,
    bytes: usize,
}

impl Operations {
    /// Retain text before any write; a full store rejects new work without evicting another draft.
    pub fn begin(
        &mut self,
        operation: ClientOperation,
        session: Option<SessionId>,
        text: Option<LocalText>,
    ) -> Result<(), ClientError> {
        if self.get(&operation).is_some() {
            return Err(ClientError::InvalidPayload(
                "operation already tracks submitted work".into(),
            ));
        }
        // Reserve an equal recovery copy because a native failure can arrive before the echo.
        let reserved = text
            .as_ref()
            .map_or(0, |draft| draft.text.len())
            .saturating_mul(2);
        if self.records.len() >= MAX_OPERATIONS
            || reserved > MAX_TEXT_BYTES.saturating_sub(self.bytes)
        {
            return Err(ClientError::Capacity(
                "private client operations are full; retain or handle existing drafts first",
            ));
        }
        self.bytes += reserved;
        self.records.push(LocalOperation {
            operation,
            session,
            state: OperationState::Pending,
            entry: None,
            text,
        });
        Ok(())
    }

    /// Borrow a record by exact origin, including the client id rather than only its counter.
    pub fn get(&self, operation: &ClientOperation) -> Option<&LocalOperation> {
        self.records
            .iter()
            .find(|record| &record.operation == operation)
    }

    /// Iterate in submission order; the platform chooses which session's echoes to render.
    pub fn iter(&self) -> impl Iterator<Item = &LocalOperation> {
        self.records.iter()
    }

    /// Release a record only after the platform consumed its outcome or retained its recovery text.
    pub fn forget(&mut self, operation: &ClientOperation) {
        if let Some(index) = self
            .records
            .iter()
            .position(|record| &record.operation == operation)
        {
            let record = self.records.remove(index);
            self.bytes -= record.text.as_ref().map_or(0, |draft| draft.text.len()) * 2;
        }
    }

    /// Adopt a private result without letting a late admission reply overwrite a newer outcome.
    pub fn apply(&mut self, result: OperationResult) -> Result<bool, ClientError> {
        let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.operation == result.operation)
        else {
            return Ok(false); // Already handled records can still have replies in flight.
        };
        validate_result(record, &result)?;
        if (terminal(&record.state) && !terminal(&result.state))
            || (matches!(record.state, OperationState::Queued)
                && matches!(result.state, OperationState::Pending))
        {
            return Ok(false);
        }
        if record.state == result.state && record.session.as_ref() == Some(&result.session) {
            return Ok(false);
        }
        record.session = Some(result.session);
        record.state = result.state;
        Ok(true)
    }

    /// Bind an authoritative row, even when it arrives before the corresponding private reply.
    pub fn observe_entry(
        &mut self,
        session: &SessionId,
        entry: &Entry,
    ) -> Result<bool, ClientError> {
        let Some(operation) = &entry.operation else {
            return Ok(false);
        };
        let Some(record) = self
            .records
            .iter_mut()
            .find(|record| &record.operation == operation)
        else {
            return Ok(false);
        };
        if record
            .session
            .as_ref()
            .is_some_and(|target| target != session)
        {
            return Err(ClientError::ForeignSession);
        }
        if record.entry.as_ref().is_some_and(|old| old != &entry.id) {
            return Err(ClientError::InvalidPayload(
                "one operation names several prompt entries".into(),
            ));
        }
        let changed = record.entry.is_none();
        record.session = Some(session.clone());
        record.entry = Some(entry.id.clone());
        Ok(changed)
    }

    /// Losing a lease makes unfinished work uncertain; preserve its text and never create a resend.
    pub fn lose_rights(&mut self, client: &ClientId) {
        for record in &mut self.records {
            if &record.operation.client != client || terminal(&record.state) {
                continue;
            }
            record.state = OperationState::Uncertain {
                message: "Daemon retry rights expired; inspect the conversation before submitting this text again".into(),
                recovery: record.text.as_ref().map(|draft| SubmittedText {
                    operation: Some(record.operation.clone()), text: draft.text.clone(),
                }),
            };
        }
    }
}

fn validate_result(record: &LocalOperation, result: &OperationResult) -> Result<(), ClientError> {
    if record
        .session
        .as_ref()
        .is_some_and(|session| session != &result.session)
    {
        return Err(ClientError::ForeignSession);
    }
    if let OperationState::Rejected { message, recovery }
    | OperationState::Uncertain { message, recovery } = &result.state
    {
        if message.len() > 4096
            || recovery.as_ref().is_some_and(|draft| {
                draft.operation.as_ref() != Some(&record.operation)
                    || record.text() != Some(&draft.text)
            })
        {
            return Err(ClientError::InvalidPayload(
                "private recovery differs from the originating submission".into(),
            ));
        }
    }
    Ok(())
}

fn terminal(state: &OperationState) -> bool {
    matches!(
        state,
        OperationState::Accepted { .. }
            | OperationState::Rejected { .. }
            | OperationState::Uncertain { .. }
    )
}
