//! Keep device cursors separate from session ownership and schedule bounded owner reads.

use carlos_core::engine::EngineCommand;
use carlos_proto::commands::{Command, RequestId, RpcError};
use carlos_proto::ids::{Sequence, SessionId};
use carlos_proto::session::Availability;

use super::state::{Interest, Read, ReadKind, Waiting};
use super::Manager;
use crate::transport::outbox::Kind;
use crate::transport::wire::error;

impl Manager {
    pub(super) fn read_command(&mut self, peer: u64, id: RequestId, command: Command) {
        match command {
            command @ (Command::QueueRead { .. }
            | Command::QueueText { .. }
            | Command::GoalRead { .. }) => self.native_read_command(peer, id, command),
            Command::Subscribe { session, since } => self.subscribe(peer, id, session, since),
            Command::Unsubscribe { session } => {
                let connection = self.peers.get_mut(&peer).expect("peer");
                connection.sessions.remove(&session);
                connection
                    .outputs
                    .retain(|(target, _, _), _| target != &session);
                connection.socket.outbox.discard(|kind| matches!(kind,
                    Kind::Shared(target) | Kind::Snapshot(target) | Kind::Output(target, _) | Kind::Source(target, _) if target == &session));
                self.reply(peer, id, ());
            }
            Command::History {
                session,
                before,
                limit,
            } => self.rpc_read(
                peer,
                id,
                session,
                EngineCommand::History {
                    before,
                    limit: usize::from(limit),
                },
            ),
            Command::SourceSeek {
                session,
                entry,
                revision,
                offset,
                limit,
            } => self.rpc_read(
                peer,
                id,
                session,
                EngineCommand::SourceAt {
                    entry,
                    revision,
                    offset,
                    limit: limit.unwrap_or(65536).min(65536) as usize,
                },
            ),
            body @ (Command::Output { .. }
            | Command::OutputSubscribe { .. }
            | Command::OutputUnsubscribe { .. }
            | Command::Source { .. }
            | Command::SourceSubscribe { .. }
            | Command::SourceUnsubscribe { .. }) => self.body_command(peer, id, body),
            _ => unreachable!("authenticated read route"),
        }
    }

    fn native_read_command(&mut self, peer: u64, id: RequestId, command: Command) {
        match command {
            Command::QueueRead {
                session,
                expected,
                after,
                limit,
                refresh,
            } => self.rpc_read(
                peer,
                id,
                session,
                EngineCommand::QueueRead {
                    expected,
                    after,
                    limit: usize::from(limit.unwrap_or(32)),
                    refresh,
                },
            ),
            Command::QueueText {
                session,
                expected,
                entry,
                offset,
                limit,
            } => self.rpc_read(
                peer,
                id,
                session,
                EngineCommand::QueueText {
                    expected,
                    entry,
                    offset,
                    limit: limit.unwrap_or(65536).min(65536) as usize,
                },
            ),
            Command::GoalRead { session, refresh } => {
                self.rpc_read(peer, id, session, EngineCommand::GoalRead { refresh })
            }
            _ => unreachable!("typed native read"),
        }
    }

    fn subscribe(&mut self, peer: u64, id: RequestId, session: SessionId, since: Option<Sequence>) {
        if let Err(reason) = self.check_subscription(&session) {
            self.fail(peer, id, reason);
            return;
        }
        let connection = self.peers.get_mut(&peer).expect("peer");
        if !connection.sessions.contains_key(&session)
            && connection.sessions.len() == crate::limits::INTERESTS
        {
            self.fail(peer, id, error(-32010, "session interests are full"));
            return;
        }
        let Some(generation) = connection.generation.checked_add(1) else {
            connection.socket.close();
            return;
        };
        connection.generation = generation;
        let since = since.filter(|_| connection.replay_allowed);
        connection.sessions.insert(
            session.clone(),
            Interest {
                seq: Sequence::default(),
                syncing: true,
                generation,
            },
        );
        connection.socket.outbox.discard(|kind| matches!(kind, Kind::Shared(target) | Kind::Snapshot(target) if target == &session));
        self.waiting.push(Waiting {
            peer,
            id,
            session: session.clone(),
            since,
            generation,
        });
        if self.sessions[&session].live.is_none() && self.sessions[&session].launch.is_none() {
            self.sessions
                .get_mut(&session)
                .expect("known session")
                .info
                .availability = Availability::Restoring;
            self.start_launch(&session, true);
            self.list_changed(&session);
        }
        self.resume_subscriptions(&session);
    }

    fn check_subscription(&self, id: &SessionId) -> Result<(), RpcError> {
        let state = self
            .sessions
            .get(id)
            .ok_or_else(|| error(-32004, "unknown session"))?;
        if self.reads.len() + self.waiting.len() >= 256 {
            return Err(error(-32010, "pending reads are full"));
        }
        if state.live.is_none() && state.launch.is_none() {
            if state.info.availability != Availability::Dormant
                || state.saved.native_thread.is_none()
            {
                return Err(error(
                    -32012,
                    "saved native session is unavailable; no replacement was created",
                ));
            }
            if self.owner_count() == crate::limits::OWNERS {
                return Err(error(-32010, "live session capacity is full"));
            }
        }
        Ok(())
    }

    pub(super) fn resume_subscriptions(&mut self, session: &SessionId) {
        let mut remaining = Vec::new();
        for waiting in std::mem::take(&mut self.waiting) {
            if &waiting.session != session {
                remaining.push(waiting);
                continue;
            }
            if !self.valid_interest(waiting.peer, session, waiting.generation) {
                self.fail(
                    waiting.peer,
                    waiting.id,
                    error(-32012, "subscription was superseded"),
                );
                continue;
            }
            let state = &self.sessions[session];
            if state.live.is_none() && state.launch.is_some() {
                remaining.push(waiting);
                continue;
            }
            let kind = ReadKind::Subscribe {
                id: waiting.id.clone(),
                generation: waiting.generation,
            };
            if let Err(reason) = self.queue_read(
                waiting.peer,
                session.clone(),
                kind,
                EngineCommand::Changes {
                    since: waiting.since,
                },
            ) {
                self.fail(waiting.peer, waiting.id, reason);
                self.peers
                    .get_mut(&waiting.peer)
                    .expect("interested peer")
                    .sessions
                    .remove(session);
            }
        }
        self.waiting = remaining;
    }

    pub(super) fn rpc_read(
        &mut self,
        peer: u64,
        id: RequestId,
        session: SessionId,
        command: EngineCommand,
    ) {
        if let Err(reason) = self.queue_read(peer, session, ReadKind::Rpc(id.clone()), command) {
            self.fail(peer, id, reason);
        }
    }

    pub(super) fn queue_read(
        &mut self,
        peer: u64,
        session: SessionId,
        kind: ReadKind,
        command: EngineCommand,
    ) -> Result<(), RpcError> {
        if self.reads.len() + self.waiting.len() >= 256 {
            return Err(error(-32010, "pending reads are full"));
        }
        let reserved = match command {
            EngineCommand::QueueRead { .. } | EngineCommand::QueueText { .. } => 512 * 1024,
            EngineCommand::GoalRead { .. } => 1024 * 1024,
            EngineCommand::Changes { .. } => 16 * 1024 * 1024,
            EngineCommand::Output { .. }
            | EngineCommand::Source { .. }
            | EngineCommand::SourceAt { .. } => 512 * 1024,
            _ => crate::limits::FRAME_BYTES,
        };
        if reserved > crate::limits::READ_BYTES.saturating_sub(self.read_bytes) {
            return Err(error(
                -32010,
                "pending reply bytes are full; retry this read later",
            ));
        }
        let live = self
            .sessions
            .get(&session)
            .and_then(|state| state.live.as_ref())
            .ok_or_else(|| error(-32012, "session has no live owner"))?;
        let reply = live.handle.request(command)?;
        self.read_bytes += reserved;
        self.reads.push(Read {
            peer,
            session,
            kind,
            reply,
            reserved,
        });
        Ok(())
    }

    pub(super) fn valid_interest(&self, peer: u64, session: &SessionId, generation: u64) -> bool {
        self.peers
            .get(&peer)
            .and_then(|connection| connection.sessions.get(session))
            .is_some_and(|interest| interest.generation == generation)
    }
}
