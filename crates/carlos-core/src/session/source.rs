//! Slice the single original entry body without parsing controls or cloning its previous prefix.

use carlos_proto::bytes::{ByteLength, ByteOffset};
use carlos_proto::ids::EntryId as WireEntryId;
use carlos_proto::output::OutputRevision;
use carlos_proto::source::{Source, SourceChunk, SourceCursor};

use super::entry_id::EntryId;
use super::events::Published;
use super::models::Message;
use super::output::{OutputReadError, MAX_OUTPUT_BYTES};
use super::state::SessionState;

impl SessionState {
    pub(super) fn export_source(&self, message: &Message) -> Source {
        Source {
            revision: OutputRevision::new(self.messages.output_revision(message.id)),
            bytes: ByteLength::new(message.text.len() as u64),
        }
    }
}

impl Published<'_> {
    /// Seek directly in a checked body; round down at most three bytes to a UTF-8 boundary.
    pub fn source_at(
        &self,
        entry: &WireEntryId,
        revision: OutputRevision,
        offset: ByteOffset,
        limit: usize,
    ) -> Result<SourceChunk, OutputReadError> {
        let id = EntryId::from_wire(entry);
        let message = self
            .session
            .messages
            .entry(id)
            .ok_or(OutputReadError::EntryNotFound)?;
        if self.session.messages.output_revision(id) != revision.get() {
            return Err(OutputReadError::Replaced);
        }
        let mut offset =
            usize::try_from(offset.get()).map_err(|_| OutputReadError::InvalidCursor)?;
        if offset > message.text.len() {
            return Err(OutputReadError::InvalidCursor);
        }
        while !message.text.is_char_boundary(offset) {
            offset -= 1;
        }
        self.source_chunk(entry, Some(&encode(entry, revision.get(), offset)), limit)
    }

    /// Read exact original text from any entry, including large plain rows and terminal escapes.
    pub fn source_chunk(
        &self,
        entry: &WireEntryId,
        from: Option<&SourceCursor>,
        limit: usize,
    ) -> Result<SourceChunk, OutputReadError> {
        let id = EntryId::from_wire(entry);
        let message = self
            .session
            .messages
            .entry(id)
            .ok_or(OutputReadError::EntryNotFound)?;
        let source = self.session.export_source(message);
        let offset = from
            .map(|token| decode(token, entry, source.revision.get()))
            .transpose()?
            .unwrap_or(0);
        if offset > message.text.len() || !message.text.is_char_boundary(offset) {
            return Err(OutputReadError::InvalidCursor);
        }
        let mut end = offset
            .saturating_add(limit.clamp(4, MAX_OUTPUT_BYTES))
            .min(message.text.len());
        while !message.text.is_char_boundary(end) {
            end -= 1;
        }
        let has_more = end < message.text.len();
        Ok(SourceChunk {
            entry: entry.clone(),
            offset: ByteOffset::new(offset as u64),
            next: encode(entry, source.revision.get(), end),
            source,
            from: from.cloned(),
            text: message.text[offset..end].to_owned(),
            has_more,
            done: !has_more && self.session.messages.is_complete(id),
            seq: self.sequence(),
        })
    }
}

fn decode(
    token: &SourceCursor,
    entry: &WireEntryId,
    revision: u64,
) -> Result<usize, OutputReadError> {
    let parts: Vec<_> = token.as_str().split(':').collect();
    if parts.len() != 4 || parts[0] != "s1" || parts[1] != entry.as_str() {
        return Err(OutputReadError::InvalidCursor);
    }
    let generation = hex(parts[2])?;
    if generation != revision {
        return Err(OutputReadError::Replaced);
    }
    usize::try_from(hex(parts[3])?).map_err(|_| OutputReadError::InvalidCursor)
}

fn encode(entry: &WireEntryId, revision: u64, offset: usize) -> SourceCursor {
    format!("s1:{entry}:{revision:016x}:{offset:016x}")
        .try_into()
        .expect("bounded source cursor")
}

fn hex(value: &str) -> Result<u64, OutputReadError> {
    if value.len() != 16
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(OutputReadError::InvalidCursor);
    }
    u64::from_str_radix(value, 16).map_err(|_| OutputReadError::InvalidCursor)
}
