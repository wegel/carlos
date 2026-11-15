//! Reuse the owner's encoded stream and recover slow subscribers with explicit snapshots.

use std::sync::{mpsc, Arc};

use carlos_core::engine::{EngineChanges, EngineCommand, EngineReply, EngineUpdate};
use carlos_proto::commands::RpcError;
use carlos_proto::events::Notification;
use carlos_proto::ids::{Sequence, SessionId};
use carlos_proto::session::Availability;
use carlos_proto::submissions::OperationState;

use super::state::ReadKind;
use super::Manager;
use crate::registry::SavedSession;
use crate::transport::outbox::{Frame, Kind};
use crate::transport::wire::{self, error};

impl Manager {
    pub(super) fn schedule_updates(&mut self) {
        for state in self.sessions.values_mut() {
            let Some(live) = &mut state.live else {
                continue;
            };
            live.dirty |= live.handle.take_updates();
            if live.dirty && live.updates.is_none() {
                if let Ok(reply) = live
                    .handle
                    .request(EngineCommand::Updates { since: live.seq })
                {
                    live.updates = Some(reply);
                    live.dirty = false;
                }
            }
        }
    }

    pub(super) fn drain_updates(&mut self) {
        let mut finished = Vec::new();
        for (id, session) in &mut self.sessions {
            let Some(live) = &mut session.live else {
                continue;
            };
            let Some(reply) = &live.updates else {
                continue;
            };
            match reply.try_recv() {
                Ok(EngineReply::Updates(update)) => {
                    live.updates = None;
                    finished.push((id.clone(), Ok(update)));
                }
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {
                    live.updates = None;
                    finished.push((
                        id.clone(),
                        Err(error(
                            -32603,
                            "session owner stopped while collecting facts",
                        )),
                    ));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        for (id, result) in finished {
            match result {
                Ok(update) => self.apply_update(&id, *update),
                Err(reason) => {
                    let session = self.sessions.get_mut(&id).expect("known session");
                    session.info.availability = Availability::Closed;
                    for mut result in std::mem::take(&mut session.create) {
                        result.state = OperationState::Uncertain {
                            message: reason.message.clone(),
                            recovery: None,
                        };
                        self.result(result);
                    }
                    self.sync_failed(&id, reason);
                    self.list_changed(&id);
                }
            }
        }
    }

    fn apply_update(&mut self, id: &SessionId, update: EngineUpdate) {
        let session = self.sessions.get_mut(id).expect("known session");
        let changed = session.observe_owner_info(update.info, &update.native_thread);
        if matches!(
            session.info.availability,
            Availability::Closed | Availability::Unavailable
        ) {
            for mut result in std::mem::take(&mut session.create) {
                result.state = OperationState::Uncertain {
                    message: "backend closed before creation could be confirmed and saved".into(),
                    recovery: None,
                };
                self.result(result);
            }
        }
        let session = self.sessions.get_mut(id).expect("known session");
        if session.info.availability == Availability::Ready {
            match SavedSession::from_info(&session.info, update.native_thread) {
                Ok(saved) => session.saved = saved,
                Err(reason) => {
                    session.save_failed = true;
                    eprintln!("carlosd: invalid live catalogue fields: {reason}");
                }
            }
        }
        if changed {
            self.list_changed(id);
        }
        // Private outcomes are retained even if the shared transcript cannot fit a wire page.
        for result in update.outcomes {
            self.result(result);
        }
        match update.changes {
            Ok(changes) => {
                let seq = changes_sequence(&changes);
                self.sessions
                    .get_mut(id)
                    .expect("known session")
                    .live
                    .as_mut()
                    .expect("owner")
                    .seq = seq;
                self.broadcast(id, &changes);
            }
            Err(reason) => self.sync_failed(id, reason),
        }
        for peer in self.peers.values_mut() {
            for ((session, _, _), watch) in &mut peer.outputs {
                if session == id {
                    watch.dirty = true;
                }
            }
        }
        self.finish_create(id);
    }

    pub(super) fn finish_create(&mut self, id: &SessionId) {
        let session = self.sessions.get_mut(id).expect("known session");
        if session.info.availability == Availability::Ready
            && session.durable.as_ref() == Some(&session.saved)
        {
            for mut result in std::mem::take(&mut session.create) {
                result.state = OperationState::Accepted {
                    entry: None,
                    notice: None,
                };
                self.result(result);
            }
        }
    }

    fn broadcast(&mut self, session: &SessionId, changes: &EngineChanges) {
        let peers: Vec<_> = self
            .peers
            .iter()
            .filter(|(_, peer)| {
                peer.sessions
                    .get(session)
                    .is_some_and(|interest| !interest.syncing)
            })
            .map(|(id, _)| *id)
            .collect();
        for peer in peers {
            self.deliver_changes(peer, session, changes);
        }
    }

    pub(super) fn deliver_changes(
        &mut self,
        peer: u64,
        session: &SessionId,
        changes: &EngineChanges,
    ) {
        match changes {
            EngineChanges::Snapshot(snapshot) => {
                let connection = self.peers.get_mut(&peer).expect("interested peer");
                connection.socket.outbox.discard(|kind| matches!(kind, Kind::Shared(target) | Kind::Snapshot(target) if target == session));
                let bytes = wire::notification(Notification::Snapshot {
                    snapshot: snapshot.clone(),
                });
                if !bytes.is_ok_and(|bytes| {
                    connection.socket.outbox.push(Frame {
                        bytes,
                        kind: Kind::Snapshot(session.clone()),
                    })
                }) {
                    connection.socket.close();
                    return;
                }
                let interest = connection.sessions.get_mut(session).expect("interest");
                interest.seq = snapshot.seq;
                interest.syncing = false;
            }
            EngineChanges::Replay { frames, seq } => {
                for frame in frames {
                    let connection = self.peers.get_mut(&peer).expect("interested peer");
                    let interest = connection.sessions.get_mut(session).expect("interest");
                    if frame.seq <= interest.seq {
                        continue;
                    }
                    if interest.seq.checked_next() != Some(frame.seq)
                        || !connection.socket.outbox.push(Frame {
                            bytes: Arc::clone(&frame.bytes),
                            kind: Kind::Shared(session.clone()),
                        })
                    {
                        self.replace_snapshot(peer, session);
                        return;
                    }
                    interest.seq = frame.seq;
                }
                let interest = self
                    .peers
                    .get_mut(&peer)
                    .expect("peer")
                    .sessions
                    .get_mut(session)
                    .expect("interest");
                interest.syncing = false;
                if interest.seq != *seq {
                    self.replace_snapshot(peer, session);
                }
            }
        }
    }

    pub(super) fn replace_snapshot(&mut self, peer: u64, session: &SessionId) {
        let connection = self.peers.get_mut(&peer).expect("peer");
        let interest = connection.sessions.get_mut(session).expect("interest");
        interest.syncing = true;
        let generation = interest.generation;
        connection.socket.outbox.discard(|kind| matches!(kind, Kind::Shared(target) | Kind::Snapshot(target) if target == session));
        if self
            .queue_read(
                peer,
                session.clone(),
                ReadKind::Snapshot { generation },
                EngineCommand::Snapshot { limit: 100 },
            )
            .is_err()
        {
            self.peers[&peer].socket.close();
        }
    }

    pub(super) fn schedule_catchups(&mut self) {
        let mut targets = Vec::new();
        for (peer, connection) in &self.peers {
            for (id, interest) in &connection.sessions {
                if !interest.syncing
                    && self.sessions[id]
                        .live
                        .as_ref()
                        .is_some_and(|live| live.seq > interest.seq)
                {
                    targets.push((*peer, id.clone(), interest.seq, interest.generation));
                }
            }
        }
        for (peer, session, since, generation) in targets {
            self.peers
                .get_mut(&peer)
                .expect("peer")
                .sessions
                .get_mut(&session)
                .expect("interest")
                .syncing = true;
            if self
                .queue_read(
                    peer,
                    session,
                    ReadKind::Snapshot { generation },
                    EngineCommand::Changes { since: Some(since) },
                )
                .is_err()
            {
                self.peers[&peer].socket.close();
            }
        }
    }

    pub(super) fn sync_failed(&mut self, session: &SessionId, reason: RpcError) {
        let peers: Vec<_> = self
            .peers
            .iter()
            .filter(|(_, peer)| peer.sessions.contains_key(session))
            .map(|(id, _)| *id)
            .collect();
        for peer in peers {
            self.peers
                .get_mut(&peer)
                .expect("peer")
                .sessions
                .remove(session);
            self.required(
                peer,
                wire::notification(Notification::SyncFailed {
                    session: session.clone(),
                    error: reason.clone(),
                }),
            );
        }
    }
}

pub(super) fn changes_sequence(changes: &EngineChanges) -> Sequence {
    match changes {
        EngineChanges::Snapshot(snapshot) => snapshot.seq,
        EngineChanges::Replay { seq, .. } => *seq,
    }
}
