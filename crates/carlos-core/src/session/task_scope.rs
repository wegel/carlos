//! Keep transcript-scoped Codex tasks and helper-scoped Claude tasks in their native lifetimes.

use carlos_proto::session::Backend;

use super::TaskRef;
use crate::backend::BackendKind;
use crate::session::state::SessionState;
use crate::session::task_digest::hash_text;

impl SessionState {
    pub(in crate::session) fn task_scope(&self) -> blake3::Hasher {
        let (name, kind) = match self.events.backend {
            Backend::Codex => ("codex", BackendKind::Codex),
            Backend::Claude => ("claude", BackendKind::Claude),
            Backend::Grok => ("grok", BackendKind::Grok),
        };
        let mut hash = blake3::Hasher::new();
        hash.update(b"carlos.task.v1");
        hash.update(self.events.owner.as_bytes());
        for text in [self.session_id().as_str(), self.thread_id(), name] {
            hash_text(&mut hash, text);
        }
        let (root, helper) = self.command_connection(kind);
        hash.update(&root.to_le_bytes());
        hash.update(&helper.to_le_bytes());
        hash.update(&[u8::from(self.events.backend_closed)]);
        hash
    }
}

impl TaskRef<'_> {
    pub(super) fn identity_scope(&self, session: &SessionState) -> blake3::Hasher {
        if !matches!(self, Self::Claude { .. }) {
            return session.task_scope();
        }
        // Native conversation reset preserves these process-level tasks. A fork/reopen
        // replaces the helper, so its generation still retires every old address.
        let mut hash = blake3::Hasher::new();
        hash.update(b"carlos.task.claude-process.v1");
        hash.update(session.events.owner.as_bytes());
        hash_text(&mut hash, session.session_id().as_str());
        hash.update(
            &session
                .command_connection(BackendKind::Claude)
                .1
                .to_le_bytes(),
        );
        hash.update(&[u8::from(session.events.backend_closed)]);
        hash
    }
}
