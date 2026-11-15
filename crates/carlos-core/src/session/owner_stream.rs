//! Start a replacement owner's numbered stream after the prior owner's final published cursor.

use std::fmt;

use carlos_proto::ids::Sequence;

use super::state::SessionState;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OwnerStreamError {
    AlreadyPublished,
    SequenceExhausted,
}

impl SessionState {
    /// Continue a logical session once, forcing the replacement's first publication to be a reset.
    pub(crate) fn continue_owner_stream(
        &mut self,
        after: Sequence,
    ) -> Result<(), OwnerStreamError> {
        if self.events.seq != Sequence::default() || self.events.info.is_some() {
            return Err(OwnerStreamError::AlreadyPublished);
        }
        if after.checked_next().is_none() {
            return Err(OwnerStreamError::SequenceExhausted);
        }
        self.events.seq = after;
        self.messages.journal.reset = true;
        Ok(())
    }
}

impl fmt::Display for OwnerStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AlreadyPublished => {
                "owner already published; its numbered stream cannot be reset"
            }
            Self::SequenceExhausted => {
                "session event counter is exhausted; no replacement owner was started"
            }
        })
    }
}

impl std::error::Error for OwnerStreamError {}

#[cfg(test)]
#[path = "owner_stream_tests.rs"]
mod tests;
