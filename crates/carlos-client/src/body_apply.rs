//! Check body revisions, explicit cursors and byte positions before changing cached pieces.

use carlos_proto::output::{OutputChunk, OutputSegment};
use carlos_proto::session::OutputKind;
use carlos_proto::source::SourceChunk;

use crate::bodies::{OutputWindow, SourceWindow, BODY_BYTES};
use crate::text_ranges::valid_utf8_start;
use crate::ClientError;

impl SourceWindow {
    pub(crate) fn apply(
        &mut self,
        chunk: SourceChunk,
        available: usize,
        document: bool,
    ) -> Result<bool, ClientError> {
        let end = source_bounds(&chunk)?;
        if chunk.source.revision != self.source.revision {
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
            chunk.from.is_some() && valid_utf8_start(self.end, chunk.offset, &chunk.text)
        } else {
            chunk.from == self.next && chunk.offset == self.end
        };
        if !valid_start || chunk.has_more && chunk.from.as_ref() == Some(&chunk.next) {
            return Err(ClientError::InvalidBody);
        }
        self.check_capacity(&chunk, available, document)?;
        let changed = self.changed(&chunk, end);
        if self.seek {
            self.start = chunk.offset;
        }
        self.seek = false;
        self.metadata_known = true;
        let bytes = self.source.bytes.get().max(chunk.source.bytes.get());
        self.source.bytes = carlos_proto::bytes::ByteLength::new(bytes);
        if document {
            self.pieces.push_document(chunk.text);
        } else {
            self.pieces.push(OutputSegment {
                kind: OutputKind::Plain,
                text: chunk.text,
            });
        }
        self.end = carlos_proto::bytes::ByteOffset::new(end);
        self.next = Some(chunk.next);
        self.seq = chunk.seq;
        self.has_more = end < bytes;
        self.done = chunk.done && !self.has_more;
        Ok(changed)
    }

    fn check_capacity(
        &self,
        chunk: &SourceChunk,
        available: usize,
        document: bool,
    ) -> Result<(), ClientError> {
        let retained = if document {
            if chunk.source.bytes.get() > BODY_BYTES as u64 {
                return Err(ClientError::Capacity(
                    "source document exceeds the body cache",
                ));
            }
            self.pieces.bytes + chunk.text.len()
        } else {
            self.pieces
                .projected_bytes(std::iter::once((OutputKind::Plain, chunk.text.len())))
        };
        check_capacity(retained, available)
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
    pub(crate) fn apply(
        &mut self,
        chunk: OutputChunk,
        available: usize,
    ) -> Result<bool, ClientError> {
        let bytes = output_bounds(&chunk)?;
        if chunk.revision != self.revision {
            return Err(ClientError::InvalidBody);
        }
        if chunk.seq < self.seq || self.next.as_ref() == Some(&chunk.next) && chunk.seq == self.seq
        {
            return Ok(false);
        }
        if chunk.from != self.next || chunk.has_more && chunk.from.as_ref() == Some(&chunk.next) {
            return Err(ClientError::InvalidBody);
        }
        let retained = self.pieces.projected_bytes(
            chunk
                .segments
                .iter()
                .map(|piece| (piece.kind, piece.text.len())),
        );
        check_capacity(retained, available)?;
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

pub(crate) fn source_bounds(chunk: &SourceChunk) -> Result<u64, ClientError> {
    let end = chunk
        .offset
        .get()
        .checked_add(chunk.text.len() as u64)
        .ok_or(ClientError::InvalidBody)?;
    if chunk.text.len() > 65536
        || end > chunk.source.bytes.get()
        || chunk.has_more != (end < chunk.source.bytes.get())
        || chunk.done && chunk.has_more
        || chunk.has_more && chunk.text.is_empty()
    {
        return Err(ClientError::InvalidBody);
    }
    Ok(end)
}

pub(crate) fn output_bounds(chunk: &OutputChunk) -> Result<usize, ClientError> {
    if chunk.segments.len() > 256 {
        return Err(ClientError::InvalidBody);
    }
    let bytes: usize = chunk.segments.iter().map(|piece| piece.text.len()).sum();
    // A read can finish the small diff prefix carried from its preceding source chunk.
    if bytes > 65536 + 256 || chunk.done && chunk.has_more {
        return Err(ClientError::InvalidBody);
    }
    Ok(bytes)
}

fn check_capacity(retained: usize, available: usize) -> Result<(), ClientError> {
    if retained > available {
        Err(ClientError::Capacity(
            "body data is full; close an unused window first",
        ))
    } else {
        Ok(())
    }
}
