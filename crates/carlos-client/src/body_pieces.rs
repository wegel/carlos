//! Keep a bounded suffix of borrowed display pieces without joining a second full body.

use std::collections::VecDeque;

use carlos_proto::output::OutputSegment;
use carlos_proto::session::OutputKind;

use crate::bodies::BODY_BYTES;

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
    pub(crate) fn document_bytes(&self) -> Option<&str> {
        match self.segments.len() {
            0 => Some(""),
            1 => self.segments.front().map(|piece| piece.text.as_str()),
            _ => None,
        }
    }

    pub(crate) fn push_document(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        self.bytes += text.len();
        if let Some(piece) = self.segments.front_mut() {
            let needed = piece.text.len() + text.len();
            if piece.text.capacity() < needed {
                let target = needed
                    .max(piece.text.capacity().saturating_mul(2))
                    .min(BODY_BYTES);
                piece.text.reserve_exact(target - piece.text.len());
            }
            piece.text.push_str(&text);
        } else {
            let mut text = text;
            if text.capacity() > text.len().max(PIECE_BYTES) {
                text.shrink_to_fit();
            }
            self.segments.push_back(OutputSegment {
                kind: OutputKind::Plain,
                text,
            });
        }
    }

    pub(crate) fn projected_bytes(
        &self,
        added: impl Iterator<Item = (OutputKind, usize)>,
    ) -> usize {
        let mut lengths: VecDeque<_> = self
            .segments
            .iter()
            .map(|piece| (piece.kind, piece.text.len()))
            .collect();
        let mut bytes = self.bytes;
        for (kind, length) in added.filter(|(_, length)| *length > 0) {
            bytes += length;
            if let Some((_, last)) = lengths
                .back_mut()
                .filter(|(last_kind, last)| *last_kind == kind && *last + length <= PIECE_BYTES)
            {
                *last += length;
            } else {
                lengths.push_back((kind, length));
            }
            while bytes > WINDOW_BYTES || lengths.len() > WINDOW_PIECES {
                bytes -= lengths.pop_front().expect("nonempty lengths").1;
            }
        }
        bytes
    }

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
