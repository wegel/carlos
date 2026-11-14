//! Borrow bounded body windows without joining original or normalized text.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::output::OutputSegment;
use carlos_proto::source::Source;

use crate::bodies::{OutputWindow, SourceWindow};

impl SourceWindow {
    /// Borrow original bytes in order; the caller interprets controls only after joining needed pieces.
    pub fn pieces(&self) -> impl Iterator<Item = &str> {
        self.pieces.segments.iter().map(|piece| piece.text.as_str())
    }
    /// Identify the first retained byte; discarded prefixes require an explicit earlier seek.
    pub fn start(&self) -> ByteOffset {
        ByteOffset::new(self.start.get() + self.pieces.discarded)
    }
    /// Identify the end of the retained interval, including bytes not yet interpreted by a renderer.
    pub fn end(&self) -> ByteOffset {
        self.end
    }
    /// Borrow the newest original-source descriptor seen by this read.
    pub fn descriptor(&self) -> Option<&Source> {
        self.metadata_known.then_some(&self.source)
    }
    /// Determine whether the server reports unread bytes after this interval.
    pub fn has_more(&self) -> bool {
        self.has_more
    }
    /// Check whether the native entry was complete at the last accepted chunk.
    pub fn is_done(&self) -> bool {
        self.done
    }
}

impl OutputWindow {
    /// Borrow normalized classified pieces; they cannot recover terminal escapes or exact copy text.
    pub fn pieces(&self) -> impl Iterator<Item = &OutputSegment> {
        self.pieces.segments.iter()
    }
    /// Distinguish a bounded suffix from output beginning at its first visible byte.
    pub fn has_missing_prefix(&self) -> bool {
        self.resumed || self.pieces.discarded > 0
    }
    /// Determine whether another bounded native read is needed.
    pub fn has_more(&self) -> bool {
        self.has_more
    }
    /// Check whether the native output is complete at the accepted cursor.
    pub fn is_done(&self) -> bool {
        self.done
    }
}
