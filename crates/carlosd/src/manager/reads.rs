//! Finish owner reads only for the socket interest or watch generation that requested them.

use std::sync::mpsc;

use carlos_core::engine::{EngineChanges, EngineCommand, EngineReply};
use carlos_proto::commands::{RequestId, SubscribeResult};
use carlos_proto::events::{Notification, NotificationMessage};
use carlos_proto::ids::SessionId;

use super::state::{Read, ReadKind};
use super::updates::changes_sequence;
use super::Manager;
use crate::transport::wire::{self, error};

impl Manager {
    pub(super) fn drain_reads(&mut self) {
        let mut pending = Vec::new();
        for read in std::mem::take(&mut self.reads) {
            if !self.peers.contains_key(&read.peer) {
                continue;
            }
            match read.reply.try_recv() {
                Ok(reply) => {
                    self.read_bytes -= read.reserved;
                    self.finish_read(read, reply);
                }
                Err(mpsc::TryRecvError::Empty) => pending.push(read),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.read_bytes -= read.reserved;
                    self.finish_read(
                        read,
                        EngineReply::Failure(error(-32603, "session owner stopped during read")),
                    );
                }
            }
        }
        self.reads.extend(pending);
    }

    fn finish_read(&mut self, read: Read, reply: EngineReply) {
        let Read {
            peer,
            session,
            kind,
            ..
        } = read;
        match kind {
            ReadKind::Rpc(id) => match reply {
                EngineReply::Queue(queue) => self.reply(peer, id, queue),
                EngineReply::QueueText(text) => self.reply(peer, id, text),
                EngineReply::Goal(goal) => self.reply(peer, id, goal),
                EngineReply::History(history) => self.reply(peer, id, history),
                EngineReply::Output(chunk) => self.reply(peer, id, chunk),
                EngineReply::Source(chunk) => self.reply(peer, id, chunk),
                EngineReply::Failure(reason) => self.fail(peer, id, reason),
                _ => self.fail(peer, id, error(-32603, "unexpected owner reply")),
            },
            ReadKind::Subscribe { id, generation } => {
                self.subscription_reply(peer, id, session, generation, reply)
            }
            ReadKind::Snapshot { generation } => {
                if !self.valid_interest(peer, &session, generation) {
                    return;
                }
                match reply {
                    EngineReply::Snapshot(snapshot) => {
                        self.deliver_changes(peer, &session, &EngineChanges::Snapshot(snapshot))
                    }
                    EngineReply::Changes(changes) => self.deliver_changes(peer, &session, &changes),
                    EngineReply::Failure(reason) => self.sync_failed(&session, reason),
                    _ => self.peers[&peer].socket.close(),
                }
            }
            ReadKind::Output {
                entry,
                format,
                generation,
            } => self.finish_output(peer, (session, entry, format), generation, None, reply),
            ReadKind::OutputFirst {
                id,
                format,
                entry,
                generation,
            } => self.finish_output(peer, (session, entry, format), generation, Some(id), reply),
        }
    }

    fn subscription_reply(
        &mut self,
        peer: u64,
        id: RequestId,
        session: SessionId,
        generation: u64,
        reply: EngineReply,
    ) {
        if !self.valid_interest(peer, &session, generation) {
            self.fail(peer, id, error(-32012, "subscription was superseded"));
            return;
        }
        match reply {
            EngineReply::Changes(changes) => {
                self.subscribed(peer, id, session, generation, changes)
            }
            EngineReply::Snapshot(snapshot) => self.subscribed(
                peer,
                id,
                session,
                generation,
                EngineChanges::Snapshot(snapshot),
            ),
            EngineReply::Failure(reason) => {
                self.peers
                    .get_mut(&peer)
                    .expect("peer")
                    .sessions
                    .remove(&session);
                self.fail(peer, id, reason);
            }
            _ => self.fail(peer, id, error(-32603, "unexpected subscription reply")),
        }
    }

    fn subscribed(
        &mut self,
        peer: u64,
        id: RequestId,
        session: SessionId,
        generation: u64,
        changes: EngineChanges,
    ) {
        let seq = changes_sequence(&changes);
        let result = match changes {
            EngineChanges::Snapshot(snapshot) => SubscribeResult::Snapshot { snapshot },
            EngineChanges::Replay { frames, .. } => {
                let bytes: usize = frames.iter().map(|frame| frame.bytes.len()).sum();
                if bytes > crate::limits::FRAME_BYTES - 1024 {
                    if let Err(reason) = self.queue_read(
                        peer,
                        session,
                        ReadKind::Subscribe {
                            id: id.clone(),
                            generation,
                        },
                        EngineCommand::Snapshot { limit: 100 },
                    ) {
                        self.fail(peer, id, reason);
                    }
                    return;
                }
                let Some(events) = replay_events(&frames) else {
                    self.peers[&peer].socket.close();
                    return;
                };
                SubscribeResult::Replay { events, seq }
            }
        };
        match wire::success(id.clone(), result) {
            Ok(bytes) => {
                self.required(peer, Ok(bytes));
                let connection = self.peers.get_mut(&peer).expect("peer");
                let interest = connection.sessions.get_mut(&session).expect("interest");
                interest.seq = seq;
                interest.syncing = false;
            }
            Err(_) => {
                self.peers
                    .get_mut(&peer)
                    .expect("peer")
                    .sessions
                    .remove(&session);
                self.fail(
                    peer,
                    id,
                    error(-32011, "subscription reply exceeds frame budget"),
                );
            }
        }
    }
}

fn replay_events(
    frames: &[carlos_core::engine::EngineFrame],
) -> Option<Vec<carlos_proto::events::SessionEvent>> {
    frames
        .iter()
        .map(|frame| {
            let message = serde_json::from_slice::<NotificationMessage>(&frame.bytes).ok()?;
            if let Notification::SessionEvent(event) = message.notification {
                Some(event)
            } else {
                None
            }
        })
        .collect()
}
