//! Bound source and normalized output windows; platform code supplies bytes and performs I/O.

use std::collections::HashMap;

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::Command;
use carlos_proto::ids::{EntryId, Sequence, SessionId};
use carlos_proto::output::{OutputCursor, OutputRevision};
use carlos_proto::source::{Source, SourceCursor};

use crate::body_pieces::Pieces;
use crate::ClientError;

const MAX_WINDOWS: usize = 32;
pub(crate) const BODY_BYTES: usize = 8 * 1024 * 1024;

#[path = "body_storage.rs"]
mod storage;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Format {
    Source,
    Output,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct Key {
    session: SessionId,
    entry: EntryId,
    format: Format,
}

/// An explicit read lifetime; opening another window for the same address retires old callbacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyToken {
    key: Key,
    generation: u64,
    document: bool,
}

impl BodyToken {
    /// Borrow the logical session whose original source or normalized output this token reads.
    pub fn session(&self) -> &SessionId {
        &self.key.session
    }
    /// Borrow the stable row address without exposing its native backend identity.
    pub fn entry(&self) -> &EntryId {
        &self.key.entry
    }
    /// Distinguish original bytes from normalized display pieces.
    pub fn is_source(&self) -> bool {
        self.key.format == Format::Source
    }
    pub(crate) fn document_source(&self) -> bool {
        self.document
    }
}

/// Original bytes remain separate from normalized display text and its parsing cursor.
pub struct SourceWindow {
    pub(crate) metadata_known: bool,
    pub(crate) source: Source,
    pub(crate) pieces: Pieces,
    pub(crate) start: ByteOffset,
    pub(crate) end: ByteOffset,
    pub(crate) next: Option<SourceCursor>,
    pub(crate) seek: bool,
    pub(crate) seq: Sequence,
    pub(crate) has_more: bool,
    pub(crate) done: bool,
}

/// Display pieces preserve their kinds and cursor even when source controls produce no text.
pub struct OutputWindow {
    pub(crate) revision: OutputRevision,
    pub(crate) pieces: Pieces,
    pub(crate) next: Option<OutputCursor>,
    pub(crate) seq: Sequence,
    pub(crate) has_more: bool,
    pub(crate) done: bool,
    pub(crate) resumed: bool,
}

enum Body {
    Source(SourceWindow),
    Output(OutputWindow),
}
struct Window {
    generation: u64,
    document: bool,
    body: Body,
}

/// At most 32 windows share eight MiB of body data, without sockets, clocks or automatic retries.
#[derive(Default)]
pub struct Bodies {
    windows: HashMap<Key, Window>,
    generation: u64,
}

impl Bodies {
    /// Maximum simultaneously retained source and normalized-output windows together.
    pub const WINDOW_LIMIT: usize = MAX_WINDOWS;
    /// Maximum retained body-data bytes across both formats; other client facts are separate.
    pub const BYTE_LIMIT: usize = BODY_BYTES;

    pub(crate) fn contains(&self, token: &BodyToken) -> bool {
        self.window(token).is_some()
    }
    pub(crate) fn tokens(&self, session: &SessionId) -> Vec<BodyToken> {
        self.windows
            .iter()
            .filter(|(key, _)| &key.session == session)
            .map(|(key, window)| BodyToken {
                key: key.clone(),
                generation: window.generation,
                document: window.document,
            })
            .collect()
    }
    /// Begin at the original body's first byte; terminal controls may span returned pieces.
    pub fn open_source(
        &mut self,
        session: SessionId,
        entry: EntryId,
        revision: OutputRevision,
    ) -> Result<BodyToken, ClientError> {
        let token = self.source_window(
            session,
            entry,
            Source {
                revision,
                bytes: carlos_proto::bytes::ByteLength::new(0),
            },
            ByteOffset::new(0),
            false,
        )?;
        let Body::Source(window) = &mut self
            .windows
            .get_mut(&token.key)
            .expect("new source window")
            .body
        else {
            unreachable!()
        };
        window.metadata_known = false;
        window.has_more = true;
        Ok(token)
    }

    /// Revisit a raw byte position directly; the first reply may round down at most three bytes.
    pub fn seek_source(
        &mut self,
        session: SessionId,
        entry: EntryId,
        source: Source,
        offset: ByteOffset,
    ) -> Result<BodyToken, ClientError> {
        if offset.get() > source.bytes.get() {
            return Err(ClientError::InvalidBody);
        }
        self.source_window(session, entry, source, offset, true)
    }

    fn source_window(
        &mut self,
        session: SessionId,
        entry: EntryId,
        source: Source,
        offset: ByteOffset,
        seek: bool,
    ) -> Result<BodyToken, ClientError> {
        let has_more = offset.get() < source.bytes.get();
        self.insert(
            Key {
                session,
                entry,
                format: Format::Source,
            },
            Body::Source(SourceWindow {
                metadata_known: true,
                source,
                pieces: Pieces::default(),
                start: offset,
                end: offset,
                next: None,
                seek,
                seq: Sequence::default(),
                has_more,
                done: false,
            }),
        )
    }

    /// Begin or resume normalized output independently of any original-source window.
    pub fn open_output(
        &mut self,
        session: SessionId,
        entry: EntryId,
        revision: OutputRevision,
        cursor: Option<OutputCursor>,
    ) -> Result<BodyToken, ClientError> {
        let resumed = cursor.is_some();
        self.insert(
            Key {
                session,
                entry,
                format: Format::Output,
            },
            Body::Output(OutputWindow {
                revision,
                next: cursor,
                resumed,
                pieces: Pieces::default(),
                seq: Sequence::default(),
                has_more: true,
                done: false,
            }),
        )
    }

    /// Borrow exact source pieces and their retained byte interval without concatenating them.
    pub fn source(&self, token: &BodyToken) -> Option<&SourceWindow> {
        match &self.window(token)?.body {
            Body::Source(window) => Some(window),
            _ => None,
        }
    }

    /// Borrow normalized pieces without treating them as original source.
    pub fn output(&self, token: &BodyToken) -> Option<&OutputWindow> {
        match &self.window(token)?.body {
            Body::Output(window) => Some(window),
            _ => None,
        }
    }

    /// Construct the next explicit read; platforms decide when to write it.
    pub fn request(&self, token: &BodyToken) -> Result<Command, ClientError> {
        let key = &token.key;
        match &self.window(token).ok_or(ClientError::RetiredBody)?.body {
            Body::Source(window) if window.seek => Ok(Command::SourceSeek {
                session: key.session.clone(),
                entry: key.entry.clone(),
                revision: window.source.revision,
                offset: window.end,
                limit: Some(65536),
            }),
            Body::Source(window) => Ok(Command::Source {
                session: key.session.clone(),
                entry: key.entry.clone(),
                cursor: window.next.clone(),
                limit: Some(65536),
            }),
            Body::Output(window) => Ok(Command::Output {
                session: key.session.clone(),
                entry: key.entry.clone(),
                cursor: window.next.clone(),
                limit: Some(65536),
            }),
        }
    }

    /// Retire both formats after a row replacement, watch reset or explicit collapse.
    pub fn forget(&mut self, session: &SessionId, entry: &EntryId) {
        self.windows
            .retain(|key, _| &key.session != session || &key.entry != entry);
    }

    /// Release all body formats when a session is detached or replaced.
    pub fn forget_session(&mut self, session: &SessionId) {
        self.windows.retain(|key, _| &key.session != session);
    }

    /// Apply a shared body descriptor without discarding a valid append-only cached prefix.
    pub fn observe_source(&mut self, session: &SessionId, entry: &EntryId, source: &Source) {
        self.observe_revision(session, entry, source.revision);
        let key = Key {
            session: session.clone(),
            entry: entry.clone(),
            format: Format::Source,
        };
        if let Some(Window {
            body: Body::Source(window),
            ..
        }) = self.windows.get_mut(&key)
        {
            if window.metadata_known && source.bytes.get() < window.source.bytes.get() {
                return;
            }
            window.metadata_known = true;
            window.source.bytes = source.bytes;
            window.has_more = window.end.get() < source.bytes.get();
            window.done &= !window.has_more;
        }
    }

    /// Retire both formats when a numbered fact announces a different original body generation.
    pub fn observe_revision(
        &mut self,
        session: &SessionId,
        entry: &EntryId,
        revision: OutputRevision,
    ) {
        self.windows.retain(|key, window| {
            if &key.session != session || &key.entry != entry {
                return true;
            }
            let current = match &window.body {
                Body::Source(window) => window.source.revision,
                Body::Output(window) => window.revision,
            };
            current == revision
        });
    }

    pub(crate) fn retain_entries(&mut self, session: &SessionId, keep: impl Fn(&EntryId) -> bool) {
        self.windows
            .retain(|key, _| &key.session != session || keep(&key.entry));
    }

    /// Release only the supplied window; an old callback cannot close its replacement.
    pub fn close(&mut self, token: &BodyToken) {
        if self.window(token).is_some() {
            self.windows.remove(&token.key);
        }
    }

    fn insert(&mut self, key: Key, body: Body) -> Result<BodyToken, ClientError> {
        if !self.windows.contains_key(&key) && self.windows.len() >= MAX_WINDOWS {
            return Err(ClientError::Capacity(
                "body windows are full; close an unused window first",
            ));
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(ClientError::Capacity("body read counter exhausted"))?;
        self.windows.insert(
            key.clone(),
            Window {
                generation: self.generation,
                body,
                document: false,
            },
        );
        Ok(BodyToken {
            key,
            generation: self.generation,
            document: false,
        })
    }

    fn window(&self, token: &BodyToken) -> Option<&Window> {
        self.windows
            .get(&token.key)
            .filter(|window| window.generation == token.generation)
    }

    fn window_mut(&mut self, token: &BodyToken) -> Option<&mut Window> {
        self.windows
            .get_mut(&token.key)
            .filter(|window| window.generation == token.generation)
    }
}
