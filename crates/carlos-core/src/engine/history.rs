//! Fetch one earlier native window on a bounded worker while the owner handles live controls.

use std::time::Duration;

use super::mailbox::{full, Envelope};
use super::owner::{failure, Owner};
use super::target::Target;
use super::workers::Completion;
use super::{EngineCommand, EngineReply};
use crate::protocol_history::{older_window, HistoryCursor, HistoryPage};

impl Owner {
    pub(super) fn defer_history_read(&mut self, envelope: Envelope) -> Option<Envelope> {
        let EngineCommand::History { before, limit } = &envelope.command else {
            return Some(envelope);
        };
        let cursor = match self.session.older_native_cursor(before, *limit) {
            Ok(Some(cursor)) => cursor,
            Ok(None) => return Some(envelope),
            Err(error) => {
                finish(envelope, EngineReply::Failure(failure(error.to_string())));
                return None;
            }
        };
        if self.history_busy || self.workers.actions_busy >= 2 {
            finish(envelope, EngineReply::Failure(full()));
            return None;
        }
        let client = match self.client.capture_commands() {
            Ok(client) => client,
            Err(error) => {
                finish(envelope, EngineReply::Failure(failure(error.to_string())));
                return None;
            }
        };
        self.history_busy = true;
        self.workers.actions_busy += 1;
        let target = Target::capture(&self.session, self.client.kind());
        self.workers.actions.submit(move || {
            let result = older_window(&client, &cursor, Duration::from_secs(20))
                .map_err(|error| error.to_string());
            Completion::History {
                pending: Box::new(envelope),
                target,
                cursor,
                result,
            }
        });
        None
    }

    pub(super) fn finish_history_read(
        &mut self,
        pending: Envelope,
        target: Target,
        cursor: HistoryCursor,
        result: Result<HistoryPage, String>,
    ) {
        self.history_busy = false;
        self.workers.actions_busy -= 1;
        let result = if target.matches(&self.session, self.client.kind()) {
            result.and_then(|page| {
                self.session
                    .accept_older_native_history(&cursor, page)
                    .map_err(|error| error.to_string())
            })
        } else {
            Err("native history read retired with its original root or connection".into())
        };
        let reply = match result {
            Ok(_) => {
                self.publish();
                let EngineCommand::History { before, limit } = &pending.command else {
                    unreachable!("captured history read");
                };
                self.history(before, *limit)
                    .unwrap_or_else(EngineReply::Failure)
            }
            Err(error) => EngineReply::Failure(failure(error)),
        };
        finish(pending, reply);
        let _ = self.channels.notice.try_send(());
    }
}

fn finish(envelope: Envelope, reply: EngineReply) {
    let _ = envelope.reply.try_send(reply);
}
