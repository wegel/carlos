//! Route shared notifications without moving another client's editor or inventing a mutation retry.

use carlos_proto::events::{Notification, SessionChange};

use crate::client::{Client, Effects, RequestFailure};
use crate::ClientError;

impl Client {
    pub(crate) fn notification(
        &mut self,
        notification: Notification,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        match notification {
            Notification::SessionEvent(event) => self.event(event, effects)?,
            Notification::Snapshot { snapshot } => self.snapshot(*snapshot, effects)?,
            Notification::SessionsChanged(change) => {
                match change {
                    SessionChange::Updated { info } => self.catalog.update(info)?,
                    SessionChange::Removed { session } => self.catalog.remove(&session),
                }
                effects.catalog = true;
            }
            Notification::Operation(result) => {
                if Some(&result.operation.client) != self.connection.client() {
                    return Err(ClientError::InvalidConnection);
                }
                let operation = result.operation.clone();
                if self.operations.apply(result)? {
                    effects.operations.push(operation);
                }
            }
            Notification::Source(update) => self.source_watch(update, effects),
            Notification::Output(update) => self.output_watch(update, effects),
            Notification::SyncFailed { session, error } => {
                if let Some(slot) = self.sessions.get_mut(&session) {
                    slot.refresh = false;
                    if let Some(replica) = &mut slot.replica {
                        replica.stale = true;
                    }
                }
                effects.failures.push(RequestFailure { id: None, error });
            }
            Notification::SourceReset {
                session,
                entry,
                message,
            } => self.reset_body_watch(session, entry, true, message, effects),
            Notification::OutputReset {
                session,
                entry,
                message,
            } => self.reset_body_watch(session, entry, false, message, effects),
        }
        Ok(())
    }
}
