//! Hold explicit source read lifetimes and local failures for current terminal body windows.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use carlos_client::{Bodies, BodyToken, Client};
use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::output::OutputRevision;

use super::transport::Transport;

struct SourceRead {
    token: BodyToken,
    revision: OutputRevision,
    failed: bool,
    used: u64,
}

#[derive(Default)]
pub(super) struct DocumentReads {
    reads: HashMap<EntryId, SourceRead>,
    clock: u64,
}

impl DocumentReads {
    pub fn token(&self, id: &EntryId) -> Option<&BodyToken> {
        self.reads
            .get(id)
            .filter(|read| !read.failed)
            .map(|read| &read.token)
    }

    /// Open only needed visible documents, evicting unused sources before reserving a slot or bytes.
    pub fn request_visible(
        &mut self,
        client: &mut Client,
        transport: &mut Transport,
        session: &SessionId,
        wanted: &[EntryId],
        visible: &[EntryId],
    ) -> Result<()> {
        self.clock = self.clock.saturating_add(1);
        for id in visible {
            if let Some(read) = self.reads.get_mut(id) {
                read.used = self.clock;
            }
        }
        let needed: HashSet<_> = wanted.iter().collect();
        for id in wanted {
            if let Some(read) = self.reads.get_mut(id) {
                read.used = self.clock;
            }
            let Some(revision) = revision(client, session, id) else {
                continue;
            };
            if self.reads.get(id).is_some_and(|read| {
                read.revision == revision
                    && (read.failed || client.bodies().source(&read.token).is_some())
            }) {
                continue;
            }
            if !self.make_room(
                client,
                transport,
                session,
                (id, source_bytes(client, session, id)),
                &needed,
            )? {
                break;
            }
            self.observe(client, transport, session, id, revision)?;
        }
        Ok(())
    }

    fn observe(
        &mut self,
        client: &mut Client,
        transport: &mut Transport,
        session: &SessionId,
        id: &EntryId,
        revision: OutputRevision,
    ) -> Result<()> {
        if self.reads.contains_key(id) {
            self.release(client, transport, id)?;
        }
        let token =
            client
                .bodies_mut()
                .open_source_document(session.clone(), id.clone(), revision)?;
        let outgoing = match client.watch_body(transport.connection, &token) {
            Ok(outgoing) => outgoing,
            Err(error) => {
                client.bodies_mut().close(&token);
                return Err(error.into());
            }
        };
        self.reads.insert(
            id.clone(),
            SourceRead {
                token,
                revision,
                failed: false,
                used: self.clock,
            },
        );
        transport.send(outgoing)?;
        Ok(())
    }

    fn make_room(
        &mut self,
        client: &mut Client,
        transport: &mut Transport,
        session: &SessionId,
        target: (&EntryId, u64),
        needed: &HashSet<&EntryId>,
    ) -> Result<bool> {
        let (id, incoming_bytes) = target;
        loop {
            let live: Vec<_> = self
                .reads
                .iter()
                .filter(|(key, read)| *key != id && client.bodies().source(&read.token).is_some())
                .collect();
            let bytes: u64 = live
                .iter()
                .map(|(id, read)| {
                    let cached = client
                        .bodies()
                        .source(&read.token)
                        .and_then(|window| window.descriptor())
                        .map_or(0, |source| source.bytes.get());
                    cached.max(source_bytes(client, session, id))
                })
                .sum();
            // An individually oversized document still gets one attempt and a latched failure.
            let reserve = incoming_bytes.min(Bodies::BYTE_LIMIT as u64);
            if live.len() < Bodies::WINDOW_LIMIT && bytes + reserve <= Bodies::BYTE_LIMIT as u64 {
                return Ok(true);
            }
            let victim = live
                .iter()
                .filter(|(id, _)| !needed.contains(id))
                .min_by_key(|(_, read)| read.used)
                .map(|(id, _)| (*id).clone());
            let Some(victim) = victim else {
                return Ok(false);
            };
            self.release(client, transport, &victim)?;
        }
    }

    /// Reserve a visible append using its announced byte length without accepting its contents.
    pub fn reserve_growth(
        &mut self,
        client: &mut Client,
        transport: &mut Transport,
        session: &SessionId,
        id: &EntryId,
        bytes: u64,
        visible: &[EntryId],
    ) -> Result<()> {
        if self
            .token(id)
            .and_then(|token| client.bodies().source(token))
            .is_some()
        {
            let needed = visible.iter().collect();
            self.make_room(client, transport, session, (id, bytes), &needed)?;
        }
        Ok(())
    }

    fn release(
        &mut self,
        client: &mut Client,
        transport: &mut Transport,
        id: &EntryId,
    ) -> Result<()> {
        if let Some(read) = self.reads.remove(id) {
            for outgoing in client.close_body(&read.token).outgoing {
                transport.send(outgoing)?;
            }
        }
        Ok(())
    }

    /// Retire removed row watches as well as their local token addresses.
    pub fn prune(
        &mut self,
        client: &mut Client,
        transport: &mut Transport,
        session: &SessionId,
    ) -> Result<()> {
        let removed: Vec<_> = self
            .reads
            .keys()
            .filter(|id| {
                client
                    .session(session)
                    .is_none_or(|replica| replica.entry(id).is_none())
            })
            .cloned()
            .collect();
        for id in removed {
            self.release(client, transport, &id)?;
        }
        Ok(())
    }

    /// Retry failed reads only after an explicit user action, never on each live append.
    pub fn retry_failed(&mut self) {
        self.reads.retain(|_, read| !read.failed);
    }

    pub fn changed(&mut self, client: &Client, session: &SessionId, token: &BodyToken) {
        if client.bodies().source(token).is_some() {
            if let Some(revision) = revision(client, session, token.entry()) {
                self.reads.insert(
                    token.entry().clone(),
                    SourceRead {
                        token: token.clone(),
                        revision,
                        failed: false,
                        used: self.clock,
                    },
                );
            }
        } else if let Some(read) = self
            .reads
            .get_mut(token.entry())
            .filter(|read| read.token == *token)
        {
            read.failed = true;
        }
    }
}

fn revision(client: &Client, session: &SessionId, id: &EntryId) -> Option<OutputRevision> {
    let entry = client.session(session)?.entry(id)?;
    entry
        .source
        .as_ref()
        .map(|source| source.revision)
        .or_else(|| entry.tool.as_ref().map(|tool| tool.output_revision))
}

fn source_bytes(client: &Client, session: &SessionId, id: &EntryId) -> u64 {
    client
        .session(session)
        .and_then(|replica| replica.entry(id))
        .and_then(|entry| entry.source.as_ref())
        .map_or(0, |source| source.bytes.get())
}
