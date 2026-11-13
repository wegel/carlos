//! Read bounded live output without cloning or rescanning the previous body.

use std::fmt;

use carlos_proto::ids::EntryId as WireEntryId;
use carlos_proto::output::{OutputChunk, OutputCursor, OutputRevision};

use super::entry_id::EntryId;
use super::events::Published;
use super::models::MessageKind;
use super::output_cursor::Cursor;

/// The maximum source bytes consumed by one output read.
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
/// The maximum classified pieces emitted by one output read.
pub const MAX_OUTPUT_SEGMENTS: usize = 256;

/// A retired row, replaced body or invalid resume position requires explicit client recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputReadError {
    EntryNotFound,
    NotTool,
    Replaced,
    InvalidCursor,
}

impl fmt::Display for OutputReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EntryNotFound => "output entry not found",
            Self::NotTool => "entry has no tool output",
            Self::Replaced => "output body replaced; restart without a cursor",
            Self::InvalidCursor => "invalid output cursor",
        })
    }
}
impl std::error::Error for OutputReadError {}

impl Published<'_> {
    /// Consume a bounded source suffix; cursors also resume split controls and diff prefixes.
    pub fn output_chunk(
        &self,
        entry: &WireEntryId,
        from: Option<&OutputCursor>,
        limit: usize,
    ) -> Result<OutputChunk, OutputReadError> {
        let id = EntryId::from_wire(entry);
        let message = self
            .session
            .messages
            .entry(id)
            .ok_or(OutputReadError::EntryNotFound)?;
        if !super::event_rows::is_tool(message) {
            return Err(OutputReadError::NotTool);
        }
        let revision = self.session.messages.output_revision(id);
        let mut cursor = from
            .map(|token| Cursor::decode(token, entry, revision))
            .transpose()?
            .unwrap_or_default();
        if !message.text.is_char_boundary(cursor.offset) {
            return Err(OutputReadError::InvalidCursor);
        }
        if cursor.closed && cursor.offset < message.text.len() {
            return Err(OutputReadError::Replaced);
        }
        let segments = cursor.consume(&message.text, message.kind == MessageKind::Diff, limit);
        let has_more = cursor.offset < message.text.len();
        let done = !has_more && self.session.messages.is_complete(id);
        let segments = cursor.finish_if_done(segments, done);
        Ok(OutputChunk {
            entry: entry.clone(),
            revision: OutputRevision::new(revision),
            from: from.cloned(),
            next: cursor.encode(entry, revision),
            segments,
            has_more,
            done,
            seq: self.sequence(),
        })
    }
}
