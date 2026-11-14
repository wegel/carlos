//! Aggregate visible task readers and keep typed native metadata reads separate from device focus.

use carlos_core::engine::{EngineCommand, EngineReply};
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::SessionId;
use carlos_proto::session::Backend;

use super::state::ReadKind;
use super::Manager;
use crate::transport::wire::error;

impl Manager {
    pub(super) fn subscribe_tasks(&mut self, peer: u64, id: RequestId, session: SessionId) {
        if !self.peers[&peer]
            .sessions
            .get(&session)
            .is_some_and(|interest| !interest.syncing)
        {
            self.fail(
                peer,
                id,
                error(
                    -32012,
                    "finish subscribing to the session before reading its tasks",
                ),
            );
            return;
        }
        if self
            .sessions
            .get(&session)
            .is_none_or(|state| state.info.backend == Backend::Grok)
        {
            self.fail(
                peer,
                id,
                error(-32012, "this backend does not support native task views"),
            );
            return;
        }
        let connection = self.peers.get_mut(&peer).expect("task reader");
        let Some(generation) = connection.generation.checked_add(1) else {
            connection.socket.close();
            return;
        };
        connection.generation = generation;
        match self.queue_read(
            peer,
            session.clone(),
            ReadKind::TaskSubscribe {
                id: id.clone(),
                generation,
            },
            EngineCommand::TaskSnapshot,
        ) {
            Ok(()) => {
                self.peers
                    .get_mut(&peer)
                    .expect("task reader")
                    .tasks
                    .insert(session, generation);
            }
            Err(reason) => self.fail(peer, id, reason),
        }
    }

    pub(super) fn unsubscribe_tasks(&mut self, peer: u64, id: RequestId, session: SessionId) {
        self.peers
            .get_mut(&peer)
            .expect("task reader")
            .tasks
            .remove(&session);
        self.reply(peer, id, ());
    }

    /// Release only the failed original reader; a newer subscription retains its native timer.
    pub(super) fn finish_task_subscription(
        &mut self,
        peer: u64,
        id: RequestId,
        session: SessionId,
        generation: u64,
        reply: EngineReply,
    ) {
        if self.peers[&peer].tasks.get(&session) != Some(&generation) {
            self.fail(peer, id, error(-32012, "task subscription was superseded"));
            return;
        }
        match reply {
            EngineReply::TaskSnapshot(snapshot) => self.reply(peer, id, snapshot),
            EngineReply::Failure(reason) => {
                self.peers
                    .get_mut(&peer)
                    .expect("task reader")
                    .tasks
                    .remove(&session);
                self.fail(peer, id, reason);
            }
            _ => {
                self.peers
                    .get_mut(&peer)
                    .expect("task reader")
                    .tasks
                    .remove(&session);
                self.fail(
                    peer,
                    id,
                    error(-32603, "unexpected task subscription reply"),
                );
            }
        }
    }

    /// Connected readers share only native polling; every device keeps its own pages and targets.
    pub(super) fn refresh_task_interests(&mut self) {
        for peer in self.peers.values_mut() {
            peer.tasks
                .retain(|session, _| peer.sessions.contains_key(session));
        }
        for (session, state) in &self.sessions {
            if let Some(live) = &state.live {
                live.handle.set_task_interest(
                    self.peers
                        .values()
                        .any(|peer| peer.tasks.contains_key(session)),
                );
            }
        }
    }

    pub(super) fn task_read_command(&mut self, peer: u64, id: RequestId, command: Command) {
        let (session, command) = match command {
            Command::TaskRun {
                session,
                task,
                read,
                refresh,
            } => (
                session,
                EngineCommand::TaskRun {
                    task,
                    read,
                    refresh,
                },
            ),
            page @ (Command::TaskList { .. } | Command::TaskOverview { .. }) => page_command(page),
            value => value_command(value),
        };
        self.rpc_read(peer, id, session, command);
    }
}

fn page_command(command: Command) -> (SessionId, EngineCommand) {
    match command {
        Command::TaskList {
            session,
            expected,
            after,
            limit,
            refresh,
        } => (
            session,
            EngineCommand::TaskList {
                expected,
                after,
                limit: usize::from(limit.unwrap_or(32)),
                refresh,
            },
        ),
        Command::TaskOverview {
            session,
            expected,
            after,
            limit,
        } => (
            session,
            EngineCommand::TaskOverview {
                expected,
                after,
                limit: usize::from(limit.unwrap_or(32)),
            },
        ),
        _ => unreachable!("typed task page"),
    }
}

fn value_command(command: Command) -> (SessionId, EngineCommand) {
    match command {
        Command::TaskDetail {
            session,
            expected,
            task,
        } => (session, EngineCommand::TaskDetail { expected, task }),
        Command::TaskText {
            session,
            expected,
            task,
            field,
            offset,
            limit,
        } => (
            session,
            EngineCommand::TaskText {
                expected,
                task,
                field,
                offset,
                limit: limit.unwrap_or(65536).min(65536) as usize,
            },
        ),
        Command::TaskOverviewText {
            session,
            expected,
            entry,
            field,
            offset,
            limit,
        } => (
            session,
            EngineCommand::TaskOverviewText {
                expected,
                entry,
                field,
                offset,
                limit: limit.unwrap_or(65536).min(65536) as usize,
            },
        ),
        _ => unreachable!("typed task scalar"),
    }
}
