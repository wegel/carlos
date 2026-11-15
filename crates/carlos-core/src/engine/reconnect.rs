//! Start the existing nonblocking Codex rebuild only for the current closed owner generation.

use carlos_proto::submissions::{ClientOperation, OperationState};

use super::dispatch::rejected;
use super::owner::Owner;
use crate::backend::BackendKind;

impl Owner {
    pub(super) fn finish_native_reconnect(&mut self) {
        if self.client.kind() == BackendKind::Codex
            && !self.session.codex_scope().closed()
            && !self.session.codex_connection().rebuilding()
            && !self.session.events.backend_closed
            && self.client.connection_generation()
                == Some(self.session.codex_connection().generation())
        {
            self.native.closed = false;
        }
    }

    pub(super) fn reconnect_codex(&mut self, operation: ClientOperation) {
        let generation = self.session.codex_connection().generation();
        let valid = self.client.kind() == BackendKind::Codex
            && self.session.codex_scope().closed()
            && !self.session.codex_connection().rebuilding()
            && self.client.connection_generation() == Some(generation);
        let state = if !valid {
            rejected(
                "Codex reconnect requires the current closed connection with no rebuild pending",
            )
        } else if self.session.reconnect_codex(self.client.as_ref()).is_some() {
            self.session.events.backend_closed = false;
            OperationState::Accepted {
                entry: None,
                notice: Some("Codex reconnect started; unconfirmed input was not resent".into()),
            }
        } else {
            rejected("Codex reconnect did not start; the existing session is kept")
        };
        self.results.update(&operation, state);
        self.notice();
    }
}
