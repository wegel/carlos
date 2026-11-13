//! Encode an entry-bound source position and bounded output parsing state.

use carlos_proto::ids::EntryId as WireEntryId;
use carlos_proto::output::{OutputCursor, OutputSegment};

use super::output::{OutputReadError, MAX_OUTPUT_BYTES, MAX_OUTPUT_SEGMENTS};
use super::output_normalize::{Control, ParseState};

#[derive(Debug, Default)]
pub(super) struct Cursor {
    pub(super) offset: usize,
    pub(super) parse: ParseState,
    pub(super) closed: bool,
}

impl Cursor {
    pub(super) fn decode(
        token: &OutputCursor,
        entry: &WireEntryId,
        revision: u64,
    ) -> Result<Self, OutputReadError> {
        let parts: Vec<_> = token.as_str().split(':').collect();
        if parts.len() != 8 || parts[0] != "o1" || parts[1] != entry.as_str() {
            return Err(OutputReadError::InvalidCursor);
        }
        let generation = hex(parts[2])?;
        if generation != revision {
            return Err(OutputReadError::Replaced);
        }
        let offset = usize::try_from(hex(parts[3])?).map_err(|_| OutputReadError::InvalidCursor)?;
        let parse = ParseState {
            control: Control::from_tag(tag(parts[4])?).ok_or(OutputReadError::InvalidCursor)?,
            line: tag(parts[5])?,
            prefix: tag(parts[6])?,
        };
        if !parse.valid() {
            return Err(OutputReadError::InvalidCursor);
        }
        let closed = match parts[7] {
            "0" => false,
            "1" => true,
            _ => return Err(OutputReadError::InvalidCursor),
        };
        Ok(Self {
            offset,
            parse,
            closed,
        })
    }

    pub(super) fn consume(&mut self, text: &str, diff: bool, limit: usize) -> Vec<OutputSegment> {
        let end = self
            .offset
            .saturating_add(limit.clamp(4, MAX_OUTPUT_BYTES))
            .min(text.len());
        let mut segments = Vec::new();
        for ch in text[self.offset..].chars() {
            if self.offset + ch.len_utf8() > end || segments.len() == MAX_OUTPUT_SEGMENTS {
                break;
            }
            self.parse.feed(ch, diff, &mut segments);
            self.offset += ch.len_utf8();
        }
        segments
    }

    pub(super) fn finish_if_done(
        &mut self,
        mut segments: Vec<OutputSegment>,
        done: bool,
    ) -> Vec<OutputSegment> {
        if done {
            self.parse.finish(&mut segments);
            self.closed = true;
        }
        segments
    }

    pub(super) fn encode(&self, entry: &WireEntryId, revision: u64) -> OutputCursor {
        format!(
            "o1:{entry}:{revision:016x}:{:016x}:{}:{}:{}:{}",
            self.offset,
            self.parse.control as u8,
            self.parse.line,
            self.parse.prefix,
            u8::from(self.closed)
        )
        .try_into()
        .expect("bounded core output cursor")
    }
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

fn tag(value: &str) -> Result<u8, OutputReadError> {
    if value.len() != 1 {
        return Err(OutputReadError::InvalidCursor);
    }
    value.parse().map_err(|_| OutputReadError::InvalidCursor)
}
