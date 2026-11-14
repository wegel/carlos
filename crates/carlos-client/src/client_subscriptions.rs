//! Adopt numbered subscription facts and request one complete snapshot after a gap.

use carlos_proto::commands::SubscribeResult;
use carlos_proto::events::SessionEvent;
use carlos_proto::ids::SessionId;
use carlos_proto::session::Snapshot;

use crate::client::{Client, Effects};
use crate::{ChangeSet, ClientError, Replica};

impl Client {
    pub(crate) fn subscribed(
        &mut self,
        serial: i64,
        session: SessionId,
        reply: SubscribeResult,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        if !self
            .sessions
            .get(&session)
            .is_some_and(|slot| slot.syncing == Some(serial))
        {
            return Ok(());
        }
        self.sessions
            .get_mut(&session)
            .expect("active subscription")
            .syncing = None;
        match reply {
            SubscribeResult::Snapshot { snapshot } => {
                if snapshot.info.id != session {
                    return Err(ClientError::ForeignSession);
                }
                self.snapshot(*snapshot, effects)?;
            }
            SubscribeResult::Replay { events, seq } => {
                for event in events {
                    if event.session != session {
                        return Err(ClientError::ForeignSession);
                    }
                    self.event(event, effects)?;
                }
                if self
                    .session(&session)
                    .is_none_or(|replica| replica.sequence() != seq || replica.is_stale())
                {
                    self.refresh(&session);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn snapshot(
        &mut self,
        snapshot: Snapshot,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        let session = snapshot.info.id.clone();
        let Some(slot) = self.sessions.get_mut(&session).filter(|slot| slot.wanted) else {
            return Ok(());
        };
        let changes = if let Some(replica) = &mut slot.replica {
            replica.replace(snapshot)?
        } else {
            slot.replica = Some(Replica::new(snapshot)?);
            ChangeSet {
                reset: true,
                info: true,
                approvals: true,
                goal: true,
                queue: true,
                ..Default::default()
            }
        };
        let replica = slot.replica.as_ref().expect("adopted snapshot");
        slot.refresh = replica.is_stale();
        for entry in replica.entries() {
            self.operations.observe_entry(&session, entry)?;
        }
        self.changed(&session, changes, effects)?;
        Ok(())
    }

    pub(crate) fn event(
        &mut self,
        event: SessionEvent,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        let session = event.session.clone();
        let echo = match &event.event {
            carlos_proto::events::Event::EntryAdded { entry }
            | carlos_proto::events::Event::EntryUpdated { entry } => {
                Some((entry.id.clone(), entry.operation.clone()))
            }
            _ => None,
        };
        let Some(slot) = self.sessions.get_mut(&session).filter(|slot| slot.wanted) else {
            return Ok(());
        };
        let Some(replica) = &mut slot.replica else {
            self.refresh(&session);
            return Ok(());
        };
        let was_new = event.seq > replica.sequence();
        let changes = match replica.apply(event) {
            Ok(changes) => changes,
            Err(ClientError::SequenceGap { .. }) => {
                self.refresh(&session);
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        if let Some((entry, Some(operation))) = echo.filter(|_| was_new) {
            if self
                .operations
                .observe_identity(&session, &entry, &operation)?
            {
                effects.operations.push(operation);
            }
        }
        self.changed(&session, changes, effects)?;
        Ok(())
    }

    pub(crate) fn refresh(&mut self, session: &SessionId) {
        if let Some(slot) = self.sessions.get_mut(session) {
            slot.refresh = true;
            slot.since = None;
            if let Some(replica) = &mut slot.replica {
                replica.stale = true;
            }
        }
    }
}
