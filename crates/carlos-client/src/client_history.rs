//! Hold bounded history pages until the numbered stream reaches the cursor at which they were read.

use std::collections::VecDeque;

use carlos_proto::commands::{RequestId, RpcError};
use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::session::History;

use crate::client::{Client, Effects, RequestFailure};
use crate::{ClientError, Replica};

const MAX_BYTES: usize = 8 * 1024 * 1024;

struct Waiting {
    serial: i64,
    session: SessionId,
    before: EntryId,
    page: History,
    bytes: usize,
}

#[derive(Default)]
pub(crate) struct WaitingHistory {
    pages: VecDeque<Waiting>,
    bytes: usize,
}

impl Client {
    pub(crate) fn history_reply(
        &mut self,
        serial: i64,
        session: SessionId,
        before: EntryId,
        page: History,
        effects: &mut Effects,
    ) {
        let Some(replica) = self.session(&session) else {
            return;
        };
        let bytes = page.entries.iter().map(crate::rows::weight).sum();
        let waiting = Waiting {
            serial,
            session,
            before,
            page,
            bytes,
        };
        if !replica.is_stale() && waiting.page.seq <= replica.sequence() {
            self.apply_history(waiting, effects);
        } else if self.history.pages.len() >= 32
            || bytes > MAX_BYTES.saturating_sub(self.history.bytes)
        {
            fail(
                serial,
                "pending history pages are full; read again after the numbered stream catches up",
                effects,
            );
        } else {
            self.history.bytes += bytes;
            self.history.pages.push_back(waiting);
        }
    }

    pub(crate) fn drain_history(&mut self, effects: &mut Effects) {
        for _ in 0..self.history.pages.len() {
            let waiting = self
                .history
                .pages
                .pop_front()
                .expect("counted waiting page");
            self.history.bytes -= waiting.bytes;
            let Some(replica) = self.session(&waiting.session) else {
                continue;
            };
            if !replica.is_stale() && waiting.page.seq <= replica.sequence() {
                self.apply_history(waiting, effects);
            } else {
                self.history.bytes += waiting.bytes;
                self.history.pages.push_back(waiting);
            }
        }
    }

    fn apply_history(&mut self, waiting: Waiting, effects: &mut Effects) {
        let Waiting {
            serial,
            session,
            before,
            page,
            ..
        } = waiting;
        let Some(replica) = self
            .sessions
            .get_mut(&session)
            .and_then(|slot| slot.replica.as_mut())
        else {
            return;
        };
        let result = replica
            .merge_history(&before, page)
            .and_then(|changes| self.changed(&session, changes, effects));
        if let Err(error) = result {
            fail(serial, &error.to_string(), effects);
        }
    }

    pub(crate) fn require_history_window(&self, session: &SessionId) -> Result<(), ClientError> {
        if self.session(session).is_none_or(Replica::is_stale) {
            return Err(ClientError::InvalidPayload(
                "subscribe to current session facts before reading its history".into(),
            ));
        }
        Ok(())
    }
}

fn fail(serial: i64, message: &str, effects: &mut Effects) {
    effects.failures.push(RequestFailure {
        id: Some(RequestId::Integer(serial)),
        error: RpcError {
            code: -32012,
            message: message.chars().take(256).collect(),
        },
    });
}
