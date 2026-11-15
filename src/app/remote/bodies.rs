//! Hold explicit source read lifetimes and local failures for current terminal body windows.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use carlos_client::{Bodies, BodyToken, Client};
use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::output::OutputRevision;

use super::formatting::Formatting;
use super::selection_copy::SelectionCopy;
use super::source_copy::CopySource;
use super::transport::Transport;

struct SourceRead {
    token: BodyToken,
    revision: OutputRevision,
    failed: bool,
    watched: bool,
    pending: Option<carlos_proto::commands::RequestId>,
    used: u64,
}

#[derive(Default)]
pub(super) struct DocumentReads {
    reads: HashMap<EntryId, SourceRead>,
    clock: u64,
    pub(super) formatting: Formatting,
    pub(super) copy: Option<CopySource>,
    pub(super) selection: Option<SelectionCopy>,
}

impl DocumentReads {
    /// Keep an explicit source failure distinct from a window that has not opened yet.
    pub fn failed(&self, id: &EntryId) -> bool {
        self.reads.get(id).is_some_and(|read| read.failed)
    }

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
            if !self.make_room(client, transport, id, &needed)? {
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
        let token = client
            .bodies_mut()
            .open_source(session.clone(), id.clone(), revision)?;
        let outgoing = match client.read_body(transport.connection, &token) {
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
                watched: false,
                pending: Some(outgoing.id()),
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
        id: &EntryId,
        needed: &HashSet<&EntryId>,
    ) -> Result<bool> {
        loop {
            let live: Vec<_> = self
                .reads
                .iter()
                .filter(|(key, read)| *key != id && client.bodies().source(&read.token).is_some())
                .collect();
            if live.len() < Bodies::WINDOW_LIMIT {
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

    /// Reopen an evicted prefix explicitly before giving it to a new owner.
    pub fn restart(
        &mut self,
        client: &mut Client,
        transport: &mut Transport,
        session: &SessionId,
        id: &EntryId,
    ) -> Result<()> {
        if let Some(revision) = revision(client, session, id) {
            self.observe(client, transport, session, id, revision)?;
        }
        Ok(())
    }

    fn release(
        &mut self,
        client: &mut Client,
        transport: &mut Transport,
        id: &EntryId,
    ) -> Result<()> {
        self.formatting.remove(id);
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

    /// Latch a formatter failure until the user explicitly retries this source.
    pub fn fail(&mut self, id: &EntryId) -> Option<BodyToken> {
        self.formatting.remove(id);
        let read = self.reads.get_mut(id)?;
        read.failed = true;
        Some(read.token.clone())
    }

    /// A replaced greeting has already discarded the client's old windows and watches.
    pub fn replaced(&mut self) {
        self.reads.clear();
        self.copy = None;
        self.selection = None;
        self.formatting.clear();
    }

    /// Retry failed reads only after an explicit user action, never on each live append.
    pub fn retry_failed(&mut self) {
        self.reads.retain(|_, read| !read.failed);
    }

    pub fn changed(&mut self, client: &Client, session: &SessionId, token: &BodyToken) {
        if client.bodies().source(token).is_some() {
            if let Some(revision) = revision(client, session, token.entry()) {
                let old = self.reads.get(token.entry());
                let watched = old.is_some_and(|read| read.watched);
                self.reads.insert(
                    token.entry().clone(),
                    SourceRead {
                        token: token.clone(),
                        revision,
                        failed: false,
                        watched,
                        pending: None,
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

#[path = "source_reads.rs"]
mod source_reads;
