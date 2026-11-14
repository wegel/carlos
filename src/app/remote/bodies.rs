//! Hold explicit source read lifetimes and local failures for current terminal body windows.

use std::collections::HashMap;

use anyhow::{bail, Result};
use carlos_client::{BodyToken, Client};
use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::output::OutputRevision;

use super::transport::Transport;

struct Read {
    token: BodyToken,
    revision: OutputRevision,
    failed: bool,
}

#[derive(Default)]
pub(super) struct DocumentReads {
    reads: HashMap<EntryId, Read>,
}

impl DocumentReads {
    pub fn token(&self, id: &EntryId) -> Option<&BodyToken> {
        self.reads
            .get(id)
            .filter(|read| !read.failed)
            .map(|read| &read.token)
    }

    pub fn observe(
        &mut self,
        client: &mut Client,
        transport: &mut Transport,
        session: &SessionId,
        id: &EntryId,
    ) -> Result<()> {
        let Some(revision) = revision(client, session, id) else {
            return Ok(());
        };
        if self.reads.get(id).is_some_and(|read| {
            read.revision == revision
                && (read.failed || client.bodies().source(&read.token).is_some())
        }) {
            return Ok(());
        }
        if self.reads.len() >= 32 && !self.reads.contains_key(id) {
            bail!("terminal source windows are full");
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
            Read {
                token,
                revision,
                failed: false,
            },
        );
        transport.send(outgoing)?;
        Ok(())
    }

    pub fn prune(&mut self, client: &Client, session: &SessionId) {
        self.reads.retain(|id, _| {
            client
                .session(session)
                .is_some_and(|replica| replica.entry(id).is_some())
        });
    }

    pub fn changed(&mut self, client: &Client, session: &SessionId, token: &BodyToken) {
        if client.bodies().source(token).is_some() {
            if let Some(revision) = revision(client, session, token.entry()) {
                self.reads.insert(
                    token.entry().clone(),
                    Read {
                        token: token.clone(),
                        revision,
                        failed: false,
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
