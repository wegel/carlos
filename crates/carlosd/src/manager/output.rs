//! Advance each expanded tool cursor only after its bounded chunk enters that peer's queue.

use carlos_core::engine::EngineReply;
use carlos_proto::commands::RequestId;
use carlos_proto::ids::{EntryId, SessionId};

use super::body::{Chunk, Cursor, Format, WatchKey};
use super::state::{OutputWatch, ReadKind};
use super::Manager;
use crate::transport::outbox::Frame;
use crate::transport::wire::{self, error};

impl Manager {
    pub(super) fn unwatch_body(
        &mut self,
        peer: u64,
        id: RequestId,
        session: SessionId,
        entry: EntryId,
        format: Format,
    ) {
        let connection = self.peers.get_mut(&peer).expect("peer");
        connection
            .outputs
            .remove(&(session.clone(), entry.clone(), format));
        connection
            .socket
            .outbox
            .discard(|kind| *kind == format.queue_kind(&session, &entry));
        self.reply(peer, id, ());
    }

    pub(super) fn watch_output(
        &mut self,
        peer: u64,
        id: RequestId,
        session: SessionId,
        entry: EntryId,
        cursor: Cursor,
    ) {
        let connection = self.peers.get_mut(&peer).expect("peer");
        let format = cursor.format();
        let key = (session.clone(), entry.clone(), format);
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
            .discard(|kind| *kind == format.queue_kind(&session, &entry));
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
        let command = cursor.request(entry.clone());
        if let Err(reason) = self.queue_read(
            peer,
            session,
            ReadKind::OutputFirst {
                id: id.clone(),
                format,
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
            for ((session, entry, format), watch) in &mut connection.outputs {
                if watch.dirty && !watch.reading {
                    watch.reading = true;
                    watch.dirty = false;
                    pending.push((
                        *peer,
                        session.clone(),
                        entry.clone(),
                        watch.cursor.clone(),
                        *format,
                        watch.generation,
                    ));
                }
            }
        }
        for (peer, session, entry, cursor, format, generation) in pending {
            let command = cursor.request(entry.clone());
            if let Err(reason) = self.queue_read(
                peer,
                session.clone(),
                ReadKind::Output {
                    format,
                    entry: entry.clone(),
                    generation,
                },
                command,
            ) {
                self.reset_output(peer, &(session, entry, format), &reason.message);
            }
        }
    }

    pub(super) fn finish_output(
        &mut self,
        peer: u64,
        key: WatchKey,
        generation: u64,
        id: Option<RequestId>,
        reply: EngineReply,
    ) {
        let (_, _, format) = &key;
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
        let chunk = match Chunk::read(reply, *format) {
            Ok(chunk) => chunk,
            Err(reason) => {
                if let Some(id) = id {
                    self.fail(peer, id, reason.clone());
                }
                self.reset_output(peer, &key, &reason.message);
                return;
            }
        };
        let changed = watch.cursor != chunk.cursor() || watch.done != chunk.done();
        let queued = self.queue_chunk(peer, &key, id, &chunk, changed);
        if !queued {
            self.reset_output(
                peer,
                &key,
                "output queue filled; explicitly start a fresh watch",
            );
            return;
        }
        self.advance_output(peer, &key, chunk);
    }

    fn advance_output(&mut self, peer: u64, key: &WatchKey, chunk: Chunk) {
        let watch = self
            .peers
            .get_mut(&peer)
            .expect("peer")
            .outputs
            .get_mut(key)
            .expect("watch");
        watch.cursor = chunk.cursor();
        watch.done = chunk.done();
        watch.reading = false;
        watch.dirty |= chunk.has_more();
        if watch.dirty {
            let _ = self.wake.try_send(());
        }
    }

    fn queue_chunk(
        &self,
        peer: u64,
        key: &WatchKey,
        id: Option<RequestId>,
        chunk: &Chunk,
        changed: bool,
    ) -> bool {
        let (session, entry, format) = key;
        if let Some(id) = id {
            self.reply(peer, id, chunk);
            return true;
        }
        if !changed {
            return true;
        }
        wire::notification(chunk.notification(session)).is_ok_and(|bytes| {
            self.peers[&peer].socket.outbox.push(Frame {
                bytes,
                kind: format.queue_kind(session, entry),
            })
        })
    }

    pub(super) fn reset_output(&mut self, peer: u64, key: &WatchKey, message: &str) {
        let (session, entry, format) = key;
        let connection = self.peers.get_mut(&peer).expect("peer");
        connection.outputs.remove(key);
        connection
            .socket
            .outbox
            .discard(|kind| *kind == format.queue_kind(session, entry));
        self.required(
            peer,
            wire::notification(format.reset(session.clone(), entry.clone(), message.into())),
        );
    }
}
