//! Reject worker replies after their logical conversation or native generation changes.

use carlos_proto::ids::SessionId;

use crate::backend::BackendKind;
use crate::session::state::SessionState;

pub(super) struct Target {
    session: SessionId,
    thread: String,
    kind: BackendKind,
    connection: (u64, u64),
}

impl Target {
    pub fn capture(session: &SessionState, kind: BackendKind) -> Self {
        Self {
            session: session.session_id().clone(),
            thread: session.thread_id().clone(),
            kind,
            connection: session.command_connection(kind),
        }
    }

    pub fn matches(&self, session: &SessionState, kind: BackendKind) -> bool {
        self.same_helper(session, kind)
            && self.thread == *session.thread_id()
            && self.connection.0 == session.command_connection(kind).0
    }

    pub fn same_helper(&self, session: &SessionState, kind: BackendKind) -> bool {
        &self.session == session.session_id()
            && self.kind == kind
            && self.connection.1 == session.command_connection(kind).1
    }
}
