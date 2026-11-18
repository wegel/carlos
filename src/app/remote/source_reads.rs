//! Pull source prefixes at the consumer's pace, then watch only their accepted tail.

use anyhow::Result;
use carlos_client::codec::Incoming;
use carlos_client::{BodyToken, Client};
use carlos_proto::commands::Command;
use carlos_proto::source::INLINE_SOURCE_BYTES;

use super::DocumentReads;
use crate::app::remote::transport::Transport;

const SOURCE_PIECE_BYTES: u64 = 64 * 1024;

impl DocumentReads {
    /// Clear only the exact outstanding read before the client checks its reply.
    pub(in crate::app::remote) fn received(
        &mut self,
        incoming: &Incoming,
        client: &Client,
    ) -> Vec<BodyToken> {
        let (id, failed) = match incoming {
            Incoming::Success { id, .. } => (id, false),
            Incoming::Failure { id: Some(id), .. } => (id, true),
            _ => return Vec::new(),
        };
        let mut retired = Vec::new();
        for read in self.reads.values_mut() {
            if read.pending.as_ref() == Some(id) {
                read.pending = None;
                if failed && client.bodies().source(&read.token).is_some() {
                    read.failed = true;
                    retired.push(read.token.clone());
                }
            }
        }
        retired
    }

    /// A new socket retires old pending read ids; accepted cursors remain available.
    pub(in crate::app::remote) fn disconnected(&mut self) {
        for read in self.reads.values_mut() {
            read.pending = None;
        }
    }

    /// Admit one bounded read per window; never let a watch push an entire large prefix.
    pub(in crate::app::remote) fn drive_reads(
        &mut self,
        client: &mut Client,
        link: &mut Transport,
        active: Option<&BodyToken>,
        accepting: bool,
    ) -> Result<()> {
        for read in self.reads.values_mut().filter(|read| !read.failed) {
            let Some(window) = client.bodies().source(&read.token) else {
                continue;
            };
            let more = window.has_more();
            let backlog = window.descriptor().is_some_and(|source| {
                source.bytes.get().saturating_sub(window.end().get()) > SOURCE_PIECE_BYTES
            });
            let large = window
                .descriptor()
                .is_some_and(|source| source.bytes.get() > INLINE_SOURCE_BYTES as u64);
            let blocked = large
                && (backlog && active != Some(&read.token)
                    || active == Some(&read.token) && !accepting);
            if read.watched && (backlog || blocked) {
                link.send(client.read(
                    link.connection,
                    Command::SourceUnsubscribe {
                        session: read.token.session().clone(),
                        entry: read.token.entry().clone(),
                    },
                )?)?;
                read.watched = false;
                read.pending = None;
            }
            if read.pending.is_some() || blocked || read.watched {
                continue;
            }
            let outgoing = if more {
                client.read_body(link.connection, &read.token)?
            } else {
                read.watched = true;
                client.watch_body(link.connection, &read.token)?
            };
            read.pending = Some(outgoing.id());
            link.send(outgoing)?;
        }
        Ok(())
    }
}
