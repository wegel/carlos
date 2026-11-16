//! Fetch one earlier native window on a bounded worker while the owner handles live controls.

use std::time::{Duration, Instant};

use super::mailbox::Envelope;
use super::owner::{failure, Owner};
use super::target::Target;
use super::workers::Completion;
use super::{EngineCommand, EngineReply};
use crate::session::history_window::{older_window, HistoryCursor, HistoryPage};

/// Keep repeated windows of one read within its original time and byte allowance.
pub(super) struct ReadScope {
    deadline: Instant,
    bytes: usize,
}

/// Retain the mailbox permit, deadline and original owner while another worker runs.
pub(super) struct WaitingRead {
    envelope: Envelope,
    scope: ReadScope,
    target: Target,
}

impl Owner {
    pub(super) fn finish_waiting_history_reads(&mut self) {
        // The mailbox permit bounds this list by the existing read count and byte budgets.
        for _ in 0..self.history_waiters.len() {
            let waiting = self.history_waiters.pop_front().expect("waiting read");
            if waiting.scope.deadline <= Instant::now() {
                finish(
                    waiting.envelope,
                    EngineReply::Failure(failure("native history read timed out")),
                );
            } else if !waiting.target.matches(&self.session, self.client.kind()) {
                finish(
                    waiting.envelope,
                    EngineReply::Failure(failure(
                        "native history read retired with its original root or connection",
                    )),
                );
            } else if let Some(envelope) =
                self.defer_history_with_scope(waiting.envelope, waiting.scope)
            {
                // An earlier accepted window may already satisfy this reader's original anchor.
                let reply = match &envelope.command {
                    EngineCommand::History { before, limit } => self.history(before, *limit),
                    EngineCommand::Prompts(command) => self.read_prompt(command),
                    _ => unreachable!("waiting history read"),
                }
                .unwrap_or_else(EngineReply::Failure);
                finish(envelope, reply);
            }
        }
    }

    pub(super) fn defer_history_read(&mut self, envelope: Envelope) -> Option<Envelope> {
        self.defer_history_with_scope(
            envelope,
            ReadScope {
                deadline: Instant::now() + Duration::from_secs(20),
                bytes: 0,
            },
        )
    }

    fn defer_history_with_scope(
        &mut self,
        envelope: Envelope,
        scope: ReadScope,
    ) -> Option<Envelope> {
        let cursor = match &envelope.command {
            EngineCommand::History { before, limit } => self
                .session
                .history_window_cursor(before, *limit)
                .map_err(|error| failure(error.to_string())),
            EngineCommand::Prompts(command) => self.session.prompt_history_cursor(command),
            _ => return Some(envelope),
        };
        let cursor = match cursor {
            Ok(Some(cursor)) => cursor,
            Ok(None) => return Some(envelope),
            Err(error) => {
                finish(envelope, EngineReply::Failure(error));
                return None;
            }
        };
        self.queue_history_read(envelope, scope, cursor)
    }

    fn queue_history_read(
        &mut self,
        envelope: Envelope,
        scope: ReadScope,
        cursor: HistoryCursor,
    ) -> Option<Envelope> {
        if scope.deadline <= Instant::now() {
            finish(
                envelope,
                EngineReply::Failure(failure("native history read timed out")),
            );
            return None;
        }
        if self.history_busy || self.workers.actions_busy >= 2 {
            self.history_waiters.push_back(WaitingRead {
                envelope,
                scope,
                target: Target::capture(&self.session, self.client.kind()),
            });
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
            let result = scope
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| anyhow::anyhow!("native history read timed out"))
                .and_then(|remaining| older_window(&client, &cursor, remaining, scope.bytes))
                .map_err(|error| error.to_string());
            Completion::History {
                pending: Box::new(envelope),
                target,
                cursor,
                result,
                scope,
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
        mut scope: ReadScope,
    ) {
        self.history_busy = false;
        self.workers.actions_busy -= 1;
        let result = if target.matches(&self.session, self.client.kind()) {
            result.and_then(|page| {
                scope.bytes = page.read_bytes();
                self.session
                    .accept_history_window(&cursor, page)
                    .map_err(|error| error.to_string())
            })
        } else {
            Err("native history read retired with its original root or connection".into())
        };
        match result {
            Ok(added) => {
                self.publish();
                let pending = if added == 0 || matches!(pending.command, EngineCommand::Prompts(_))
                {
                    let Some(pending) = self.defer_history_with_scope(pending, scope) else {
                        return;
                    };
                    pending
                } else {
                    pending
                };
                let reply = match &pending.command {
                    EngineCommand::History { before, limit } => self.history(before, *limit),
                    EngineCommand::Prompts(command) => self.read_prompt(command),
                    _ => unreachable!("captured history or prompt read"),
                }
                .unwrap_or_else(EngineReply::Failure);
                finish(pending, reply);
            }
            Err(error) => finish(pending, EngineReply::Failure(failure(error))),
        };
        let _ = self.channels.notice.try_send(());
    }
}

fn finish(envelope: Envelope, reply: EngineReply) {
    let _ = envelope.reply.try_send(reply);
}
