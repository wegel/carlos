//! Merge compact queue facts without duplicating a client's bounded selected page or editor.

use carlos_proto::queue::QueuePage;

use crate::{ChangeSet, Client, ClientError, Effects, Replica};

impl Replica {
    /// Reject late and future queue pages while allowing unrelated newer transcript facts.
    pub fn merge_queue(&mut self, page: &QueuePage) -> Result<ChangeSet, ClientError> {
        if page.session != self.info.id {
            return Err(ClientError::ForeignSession);
        }
        if self.stale || page.seq < self.queue_seq || page.seq > self.seq {
            return Err(ClientError::StaleQueue);
        }
        let mut changes = ChangeSet::default();
        self.replace_queue(Some(page.queue.clone()), page.seq, &mut changes);
        Ok(changes)
    }
}

impl Client {
    /// Accept a queue header through the same sequence fences and byte budget as numbered facts.
    pub fn merge_queue(&mut self, page: &QueuePage) -> Result<Effects, ClientError> {
        let session = page.session.clone();
        let replica = self
            .sessions
            .get_mut(&session)
            .and_then(|slot| slot.replica.as_mut())
            .ok_or(ClientError::ForeignSession)?;
        let changes = replica.merge_queue(page)?;
        let mut effects = Effects::default();
        self.changed(&session, changes, &mut effects)?;
        self.drive(&mut effects);
        Ok(effects)
    }
}
