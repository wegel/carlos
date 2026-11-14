//! Keep one accepted task header while devices own bounded pages, details and private stop intent.

use carlos_proto::task_facts::TaskSnapshot;
use carlos_proto::tasks::TaskPage;

use crate::{ChangeSet, Client, ClientError, Effects, Replica};

impl Replica {
    /// Reject replies from a retired root, an older task header or an unreached session cursor.
    pub fn merge_tasks(&mut self, snapshot: &TaskSnapshot) -> Result<ChangeSet, ClientError> {
        if snapshot.session != self.info.id {
            return Err(ClientError::ForeignSession);
        }
        if self.stale || snapshot.seq < self.tasks_seq || snapshot.seq > self.seq {
            return Err(ClientError::StaleTasks);
        }
        let mut changes = ChangeSet::default();
        self.replace_tasks(Some(snapshot.tasks.clone()), snapshot.seq, &mut changes);
        Ok(changes)
    }

    /// A metadata page is a private display window, never a second authoritative task header.
    pub fn check_task_page(&self, page: &TaskPage) -> Result<(), ClientError> {
        if page.session != self.info.id {
            return Err(ClientError::ForeignSession);
        }
        let Some(facts) = &self.tasks else {
            return Err(ClientError::StaleTasks);
        };
        if self.stale
            || page.seq < self.tasks_seq
            || page.seq > self.seq
            || page.revision != facts.revision
            || page.known != facts.known
            || page.reading != facts.reading
            || page.closed != facts.closed
        {
            return Err(ClientError::StaleTasks);
        }
        Ok(())
    }
}

impl Client {
    /// Merge task subscription replies through the same changed-fact and combined byte budget path.
    pub fn merge_tasks(&mut self, snapshot: &TaskSnapshot) -> Result<Effects, ClientError> {
        let session = snapshot.session.clone();
        let replica = self
            .sessions
            .get_mut(&session)
            .and_then(|slot| slot.replica.as_mut())
            .ok_or(ClientError::ForeignSession)?;
        let changes = replica.merge_tasks(snapshot)?;
        let mut effects = Effects::default();
        self.changed(&session, changes, &mut effects)?;
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Check a private metadata page against this client's one accepted current session copy.
    pub fn check_task_page(&self, page: &TaskPage) -> Result<(), ClientError> {
        self.session(&page.session)
            .ok_or(ClientError::ForeignSession)?
            .check_task_page(page)
    }
}
