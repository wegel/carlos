//! Copy bounded original bytes from an accepted cache window to one platform-owned document.

use carlos_client::{Bodies, BodyToken};
use carlos_proto::bytes::ByteOffset;
use carlos_proto::output::OutputRevision;
use carlos_proto::source::Source;

const PIECE_BYTES: usize = 64 * 1024;

/// Explain why the caller cannot continue its accepted prefix from this cache window.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SourcePieceError {
    Retired,
    Unknown,
    Replaced,
    MissingPrefix {
        requested: ByteOffset,
        available: ByteOffset,
    },
    InvalidOffset,
    Allocation,
}

/// Move only a bounded newly accepted interval across the formatter's thread boundary.
pub(super) struct SourcePiece {
    pub token: BodyToken,
    pub source: Source,
    pub offset: ByteOffset,
    pub text: String,
    pub done: bool,
}

impl SourcePiece {
    /// Read the next interval after cursor/revision validation, without accepting raw wire text here.
    pub fn from_cache(
        cache: &Bodies,
        token: &BodyToken,
        revision: OutputRevision,
        offset: ByteOffset,
    ) -> Result<Self, SourcePieceError> {
        let window = cache.source(token).ok_or(SourcePieceError::Retired)?;
        let source = window.descriptor().ok_or(SourcePieceError::Unknown)?;
        if source.revision != revision {
            return Err(SourcePieceError::Replaced);
        }
        if offset < window.start() {
            return Err(SourcePieceError::MissingPrefix {
                requested: offset,
                available: window.start(),
            });
        }
        let bytes = window
            .end()
            .get()
            .checked_sub(offset.get())
            .ok_or(SourcePieceError::InvalidOffset)?;
        let mut skip = usize::try_from(offset.get() - window.start().get())
            .map_err(|_| SourcePieceError::InvalidOffset)?;
        let mut text = String::new();
        text.try_reserve_exact(bytes.min(PIECE_BYTES as u64) as usize)
            .map_err(|_| SourcePieceError::Allocation)?;
        for piece in window.pieces() {
            if skip >= piece.len() {
                skip -= piece.len();
                continue;
            }
            let rest = piece.get(skip..).ok_or(SourcePieceError::InvalidOffset)?;
            skip = 0;
            let mut end = rest.len().min(PIECE_BYTES - text.len());
            while !rest.is_char_boundary(end) {
                end -= 1;
            }
            text.push_str(&rest[..end]);
            if end < rest.len() || text.len() == PIECE_BYTES {
                break;
            }
        }
        if skip > 0 {
            return Err(SourcePieceError::InvalidOffset);
        }
        let done = window.is_done() && offset.get() + text.len() as u64 == source.bytes.get();
        Ok(Self {
            token: token.clone(),
            source: source.clone(),
            offset,
            text,
            done,
        })
    }
}

#[cfg(test)]
#[path = "accepted_source_tests.rs"]
mod tests;
