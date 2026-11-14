//! Advance each expanded tool cursor only after its bounded chunk enters that peer's queue.

use carlos_core::engine::{EngineCommand, EngineReply};
use carlos_proto::commands::RequestId;
use carlos_proto::events::Notification;
use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::output::{OutputChunk, OutputCursor, OutputUpdate};

use super::state::{OutputWatch, ReadKind};
use super::Manager;
use crate::transport::outbox::{Frame, Kind};
use crate::transport::wire::{self, error};

impl Manager {
    pub(super) fn watch_output(
        &mut self,
        peer: u64,
        id: RequestId,
        session: SessionId,
        entry: EntryId,
        cursor: Option<OutputCursor>,
    ) {
        let connection = self.peers.get_mut(&peer).expect("peer");
        let key = (session.clone(), entry.clone());
        if !connection.outputs.contains_key(&key)
            && connection.outputs.len() == crate::limits::WATCHES
        {
            self.fail(peer, id, error(-32010, "expanded output watches are full"));
            return;
        }
        let Some(generation) = connection.generation.checked_add(1) else {
            connection.socket.close();
            return;
        };
        connection.generation = generation;
        connection
            .socket
            .outbox
            .discard(|kind| *kind == Kind::Output(session.clone(), entry.clone()));
        connection.outputs.insert(
            key.clone(),
            OutputWatch {
                cursor: cursor.clone(),
                generation,
                reading: true,
                dirty: false,
                done: false,
            },
        );
        let command = EngineCommand::Output {
            entry: entry.clone(),
            cursor,
            limit: 65536,
        };
        if let Err(reason) = self.queue_read(
            peer,
            session,
            ReadKind::OutputFirst {
                id: id.clone(),
                entry,
                generation,
            },
            command,
        ) {
            self.peers
                .get_mut(&peer)
                .expect("peer")
                .outputs
                .remove(&key);
            self.fail(peer, id, reason);
        }
    }

    pub(super) fn schedule_outputs(&mut self) {
        let mut pending = Vec::new();
        for (peer, connection) in &mut self.peers {
            for ((session, entry), watch) in &mut connection.outputs {
                if watch.dirty && !watch.reading {
                    watch.reading = true;
                    watch.dirty = false;
                    pending.push((
                        *peer,
                        session.clone(),
                        entry.clone(),
                        watch.cursor.clone(),
                        watch.generation,
                    ));
                }
            }
        }
        for (peer, session, entry, cursor, generation) in pending {
            let command = EngineCommand::Output {
                entry: entry.clone(),
                cursor,
                limit: 65536,
            };
            if let Err(reason) = self.queue_read(
                peer,
                session.clone(),
                ReadKind::Output {
                    entry: entry.clone(),
                    generation,
                },
                command,
            ) {
                self.reset_output(peer, &session, &entry, &reason.message);
            }
        }
    }

    pub(super) fn finish_output(
        &mut self,
        peer: u64,
        session: SessionId,
        entry: EntryId,
        generation: u64,
        id: Option<RequestId>,
        reply: EngineReply,
    ) {
        let key = (session.clone(), entry.clone());
        let Some(watch) = self
            .peers
            .get(&peer)
            .and_then(|peer| peer.outputs.get(&key))
            .filter(|watch| watch.generation == generation)
        else {
            if let Some(id) = id {
                self.fail(peer, id, error(-32012, "output watch was superseded"));
            }
            return;
        };
        let chunk = match reply {
            EngineReply::Output(chunk) => chunk,
            EngineReply::Failure(reason) => {
                if let Some(id) = id {
                    self.fail(peer, id, reason.clone());
                }
                self.reset_output(peer, &session, &entry, &reason.message);
                return;
            }
            _ => {
                self.reset_output(peer, &session, &entry, "unexpected output reply");
                return;
            }
        };
        let changed = watch.cursor.as_ref() != Some(&chunk.next)
            || !chunk.segments.is_empty()
            || watch.done != chunk.done;
        let queued = self.queue_chunk(peer, &session, &entry, id, &chunk, changed);
        if !queued {
            self.reset_output(
                peer,
                &session,
                &entry,
                "output queue filled; explicitly start a fresh watch",
            );
            return;
        }
        self.advance_output(peer, &key, chunk);
    }

    fn advance_output(&mut self, peer: u64, key: &(SessionId, EntryId), chunk: OutputChunk) {
        let watch = self
            .peers
            .get_mut(&peer)
            .expect("peer")
            .outputs
            .get_mut(key)
            .expect("watch");
        watch.cursor = Some(chunk.next);
        watch.done = chunk.done;
        watch.reading = false;
        watch.dirty |= chunk.has_more;
        if watch.dirty {
            let _ = self.wake.try_send(());
        }
    }

    fn queue_chunk(
        &self,
        peer: u64,
        session: &SessionId,
        entry: &EntryId,
        id: Option<RequestId>,
        chunk: &OutputChunk,
        changed: bool,
    ) -> bool {
        if let Some(id) = id {
            self.reply(peer, id, chunk);
            return true;
        }
        if !changed {
            return true;
        }
        wire::notification(Notification::Output(OutputUpdate {
            session: session.clone(),
            chunk: chunk.clone(),
        }))
        .is_ok_and(|bytes| {
            self.peers[&peer].socket.outbox.push(Frame {
                bytes,
                kind: Kind::Output(session.clone(), entry.clone()),
            })
        })
    }

    fn reset_output(&mut self, peer: u64, session: &SessionId, entry: &EntryId, message: &str) {
        let connection = self.peers.get_mut(&peer).expect("peer");
        connection.outputs.remove(&(session.clone(), entry.clone()));
        connection
            .socket
            .outbox
            .discard(|kind| *kind == Kind::Output(session.clone(), entry.clone()));
        self.required(
            peer,
            wire::notification(Notification::OutputReset {
                session: session.clone(),
                entry: entry.clone(),
                message: message.into(),
            }),
        );
    }
}
