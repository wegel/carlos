//! Check body revisions, explicit cursors and byte positions before changing cached pieces.

use carlos_proto::output::{OutputChunk, OutputSegment};
use carlos_proto::session::OutputKind;
use carlos_proto::source::SourceChunk;

use crate::bodies::{OutputWindow, SourceWindow};
use crate::ClientError;

impl SourceWindow {
    pub(crate) fn apply(&mut self, chunk: SourceChunk) -> Result<bool, ClientError> {
        let end = chunk
            .offset
            .get()
            .checked_add(chunk.text.len() as u64)
            .ok_or(ClientError::InvalidBody)?;
        if chunk.source.revision != self.source.revision
            || chunk.text.len() > 65536
            || end > chunk.source.bytes.get()
            || chunk.has_more != (end < chunk.source.bytes.get())
            || chunk.done && chunk.has_more
        {
            return Err(ClientError::InvalidBody);
        }
        if chunk.seq < self.seq
            || self.next.as_ref() == Some(&chunk.next)
                && end == self.end.get()
                && chunk.seq == self.seq
        {
            return Ok(false);
        }
        let valid_start = if self.seek {
            chunk.from.is_some()
                && chunk.offset.get() <= self.end.get()
                && self.end.get() - chunk.offset.get() <= 3
        } else {
            chunk.from == self.next && chunk.offset == self.end
        };
        if !valid_start || chunk.has_more && chunk.from.as_ref() == Some(&chunk.next) {
            return Err(ClientError::InvalidBody);
        }
        let changed = self.changed(&chunk, end);
        if self.seek {
            self.start = chunk.offset;
        }
        self.seek = false;
        self.metadata_known = true;
        let bytes = self.source.bytes.get().max(chunk.source.bytes.get());
        self.source.bytes = carlos_proto::bytes::ByteLength::new(bytes);
        self.pieces.push(OutputSegment {
            kind: OutputKind::Plain,
            text: chunk.text,
        });
        self.end = carlos_proto::bytes::ByteOffset::new(end);
        self.next = Some(chunk.next);
        self.seq = chunk.seq;
        self.has_more = end < bytes;
        self.done = chunk.done && !self.has_more;
        Ok(changed)
    }

    fn changed(&self, chunk: &SourceChunk, end: u64) -> bool {
        let bytes = self.source.bytes.get().max(chunk.source.bytes.get());
        self.seek
            || !chunk.text.is_empty()
            || self.next.as_ref() != Some(&chunk.next)
            || self.source.bytes.get() != bytes
            || self.has_more != (end < bytes)
            || self.done != (chunk.done && end == bytes)
    }
}

impl OutputWindow {
    pub(crate) fn apply(&mut self, chunk: OutputChunk) -> Result<bool, ClientError> {
        if chunk.segments.len() > 256 {
            return Err(ClientError::InvalidBody);
        }
        let bytes: usize = chunk.segments.iter().map(|piece| piece.text.len()).sum();
        // Native reads can finish a small pending diff prefix from their preceding source chunk.
        if chunk.revision != self.revision || bytes > 65536 + 256 || chunk.done && chunk.has_more {
            return Err(ClientError::InvalidBody);
        }
        if chunk.seq < self.seq || self.next.as_ref() == Some(&chunk.next) && chunk.seq == self.seq
        {
            return Ok(false);
        }
        if chunk.from != self.next || chunk.has_more && chunk.from.as_ref() == Some(&chunk.next) {
            return Err(ClientError::InvalidBody);
        }
        let changed = bytes > 0
            || self.next.as_ref() != Some(&chunk.next)
            || self.has_more != chunk.has_more
            || self.done != chunk.done;
        for piece in chunk.segments {
            self.pieces.push(piece);
        }
        self.next = Some(chunk.next);
        self.seq = chunk.seq;
        self.has_more = chunk.has_more;
        self.done = chunk.done;
        Ok(changed)
    }
}
