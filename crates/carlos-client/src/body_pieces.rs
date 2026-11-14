//! Keep a bounded suffix of borrowed display pieces without joining a second full body.

use std::collections::VecDeque;

use carlos_proto::output::OutputSegment;

pub(crate) const WINDOW_BYTES: usize = 256 * 1024;
const PIECE_BYTES: usize = 64 * 1024;
const WINDOW_PIECES: usize = 1024;

#[derive(Default)]
pub(crate) struct Pieces {
    pub(crate) segments: VecDeque<OutputSegment>,
    pub(crate) bytes: usize,
    pub(crate) discarded: u64,
}

impl Pieces {
    pub(crate) fn push(&mut self, mut piece: OutputSegment) {
        if piece.text.is_empty() {
            return;
        }
        if piece.text.capacity() > piece.text.len().max(PIECE_BYTES) {
            piece.text.shrink_to_fit();
        }
        self.bytes += piece.text.len();
        if let Some(last) = self.segments.back_mut().filter(|last| {
            last.kind == piece.kind && last.text.len() + piece.text.len() <= PIECE_BYTES
        }) {
            last.text.push_str(&piece.text);
        } else {
            self.segments.push_back(piece);
        }
        while self.bytes > WINDOW_BYTES || self.segments.len() > WINDOW_PIECES {
            let old = self.segments.pop_front().expect("nonempty pieces");
            self.bytes -= old.text.len();
            self.discarded += old.text.len() as u64;
        }
    }
}
