//! Feed explicit small literal chunks through the same private clipboard epoch and worker queue.

use std::sync::atomic::Ordering;
use std::sync::mpsc::SyncSender;

use super::transfer::send;
use super::{Formatter, Reply, Request, TransferError};

pub(super) const MAX_PIECE: usize = 64 * 1024;

pub(super) struct Piece {
    pub(super) copy: u64,
    offset: usize,
    text: String,
    finish: bool,
}

/// Retain only the current explicit literal copy; epoch changes discard its partial text.
pub(super) struct Literal {
    pub(super) copy: u64,
    text: String,
}

/// Append exact ordered UTF-8 chunks, returning the complete text only on its final chunk.
pub(super) fn append(
    pending: &mut Option<Literal>,
    copy: u64,
    offset: usize,
    text: &str,
    finish: bool,
) -> Result<Option<String>, TransferError> {
    if offset == 0 && pending.as_ref().is_none_or(|current| current.copy != copy) {
        *pending = Some(Literal {
            copy,
            text: String::new(),
        });
    }
    let current = pending
        .as_mut()
        .filter(|current| current.copy == copy)
        .ok_or(TransferError::Retired)?;
    if current.text.len() != offset || text.len() > MAX_PIECE || text.is_empty() && !finish {
        *pending = None;
        return Err(TransferError::InvalidOffset);
    }
    current
        .text
        .try_reserve(text.len())
        .map_err(|_| TransferError::Allocation)?;
    current.text.push_str(text);
    Ok(finish.then(|| pending.take().expect("complete literal copy").text))
}

pub(super) fn receive(
    pending: &mut Option<Literal>,
    piece: Piece,
    replies: &SyncSender<Reply>,
) -> Result<(), TransferError> {
    let copy = piece.copy;
    let reply = match append(pending, copy, piece.offset, &piece.text, piece.finish) {
        Ok(Some(text)) => Some(Reply::Copied {
            copy,
            backend: crate::clipboard::try_copy_clipboard(&text),
        }),
        Ok(None) => None,
        Err(error) => {
            *pending = None;
            Some(Reply::CopyFailed { copy, error })
        }
    };
    if let Some(reply) = reply {
        replies.send(reply).map_err(|_| TransferError::Closed)?;
    }
    Ok(())
}

impl Formatter {
    /// Check whether an older copy still owns the one clipboard permission.
    pub fn copy_is_current(&self, copy: u64) -> bool {
        self.wanted.copy.load(Ordering::Acquire) == copy
    }
    /// Queue one literal piece under an existing copy permission without changing source ownership.
    pub fn literal_copy(
        &self,
        copy: u64,
        offset: usize,
        text: &str,
        finish: bool,
    ) -> Result<bool, TransferError> {
        if self.wanted.copy.load(Ordering::Acquire) != copy {
            return Err(TransferError::Retired);
        }
        if text.len() > MAX_PIECE {
            return Err(TransferError::InvalidOffset);
        }
        send(
            &self.sender,
            Request::LiteralCopy(Piece {
                copy,
                offset,
                text: text.into(),
                finish,
            }),
        )
    }
}

#[cfg(test)]
#[path = "transcript_formatter_literal_tests.rs"]
mod tests;
