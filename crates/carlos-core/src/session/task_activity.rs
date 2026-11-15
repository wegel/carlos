//! Address independent child activity interests without changing the selected task or native root.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use carlos_proto::ids::{Sequence, TaskActivityId, TaskId, TaskRevision};

use super::state::SessionState;
use super::task_window_state::Scope;

/// The daemon binds each socket/session reader; an activity id alone grants no read right.
#[derive(Clone, Debug)]
pub struct ActivityContext {
    pub activity: TaskActivityId,
    pub reader: Arc<AtomicBool>,
}

/// Native requests remain behind typed child reads and the existing bounded owner mailbox.
pub enum ActivityCommand {
    Open {
        reader: Arc<AtomicBool>,
        task: TaskId,
        expected: TaskRevision,
        window: Option<super::task_windows::WindowContext>,
    },
    Read {
        context: ActivityContext,
        since: Option<Sequence>,
    },
    Page {
        context: ActivityContext,
        expected: TaskRevision,
        after: Option<carlos_proto::ids::TaskActivityItemId>,
        limit: usize,
    },
    Advance {
        context: ActivityContext,
        older: bool,
    },
    Close {
        reader: Arc<AtomicBool>,
        activity: Option<TaskActivityId>,
    },
}

/// One interest owns its cursors and counters while all native activity data share the existing pool.
#[derive(Debug)]
pub(in crate::session) struct ActivityOwner {
    pub context: ActivityContext,
    pub task: TaskId,
    pub scope: Scope,
    pub revision: u64,
    pub change: u64,
    pub known: bool,
    pub retired: bool,
}

impl ActivityOwner {
    pub fn available(&self, session: &SessionState, context: &ActivityContext) -> bool {
        self.context.activity == context.activity
            && Arc::ptr_eq(&self.context.reader, &context.reader)
            && self.context.reader.load(Ordering::Acquire)
            && !self.retired
            && self.scope == Scope::capture(session)
    }

    pub fn changed(&mut self, items: bool) {
        if self.change == u64::MAX || items && self.revision == u64::MAX {
            self.retired = true;
            return;
        }
        self.change += 1;
        self.revision += u64::from(items);
    }

    pub fn revision(&self) -> TaskRevision {
        let mut digest = blake3::Hasher::new();
        digest.update(b"carlos.child-activity.items.v1");
        digest.update(self.context.activity.as_str().as_bytes());
        digest.update(&self.revision.to_le_bytes());
        digest
            .finalize()
            .to_hex()
            .as_str()
            .parse()
            .expect("activity revision")
    }
}
