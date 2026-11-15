//! Copy a frozen selection through bounded row batches while keeping its text off the input loop.

use std::sync::atomic::Ordering;

use carlos_proto::source::Source;

use super::frame::FormattedBlock;
use super::transfer::send;
use super::{Formatter, Reply, Request, TransferError, Wanted};
use crate::app::render_entry_cache::PaintedRows;
use crate::app::selection::{Selection, SelectionText};

/// Keep the original global columns and row addresses across several source owners.
#[derive(Clone, PartialEq, Eq)]
pub(in crate::app) struct SelectionSlice {
    pub copy: u64,
    pub bounds: [usize; 4],
    pub base: usize,
    pub finish: bool,
}

/// Match the complete body and layout that produced the selected screen rows.
#[derive(Clone, PartialEq, Eq)]
pub(in crate::app) struct SelectionTarget {
    pub slice: SelectionSlice,
    pub source: Source,
    pub count: usize,
}

/// Ordinary viewport frames go to the terminal; selection rows feed its private copy instead.
#[derive(Clone, Default, PartialEq, Eq)]
pub(in crate::app) enum Target {
    #[default]
    Display,
    Selection(SelectionTarget),
}

/// Accumulate one explicit copy without retaining the rows used to build it.
pub(super) struct Selected {
    pub id: u64,
    text: SelectionText,
}

/// Append an exact ordered interval and finish only after the final selected entry.
pub(super) fn append(
    selected: &mut Option<Selected>,
    wanted: &Wanted,
    slice: &SelectionSlice,
    rows: &PaintedRows,
    first: usize,
    length: usize,
) -> Result<Reply, TransferError> {
    if wanted.copy.load(Ordering::Acquire) != slice.copy {
        return Err(TransferError::Retired);
    }
    if selected
        .as_ref()
        .is_none_or(|selected| selected.id != slice.copy)
    {
        let [ax, ay, fx, fy] = slice.bounds;
        *selected = Some(Selected {
            id: slice.copy,
            text: SelectionText::new(Selection {
                anchor_x: ax,
                anchor_line_idx: ay,
                focus_x: fx,
                focus_line_idx: fy,
                dragging: false,
            }),
        });
    }
    let current = selected.as_mut().expect("selected copy");
    for index in first..first.saturating_add(length) {
        let line = rows.get(index).ok_or(TransferError::Format)?;
        current.text.push(slice.base + index, line);
    }
    if !slice.finish {
        return Ok(Reply::Selected { copy: slice.copy });
    }
    let text = selected.take().expect("completed copy").text.finish();
    let backend = crate::clipboard::try_copy_clipboard(&text);
    Ok(Reply::Copied {
        copy: slice.copy,
        backend,
    })
}

/// Paint at most64rows at a time, even when a selection spans millions of wrapped rows.
pub(super) fn block(
    selected: &mut Option<Selected>,
    wanted: &Wanted,
    target: &SelectionTarget,
    block: &FormattedBlock,
    first: usize,
    length: usize,
) -> Result<Reply, TransferError> {
    let end = first
        .checked_add(length)
        .ok_or(TransferError::InvalidOffset)?;
    let mut reply = Reply::Selected {
        copy: target.slice.copy,
    };
    for start in (first..end).step_by(64) {
        let length = 64.min(end - start);
        let prepared = block
            .window(start, length)
            .map_err(|_| TransferError::Format)?;
        if prepared.count != target.count {
            return Err(TransferError::Replaced);
        }
        let rows = prepared.lines.ok_or(TransferError::Format)?;
        let mut slice = target.slice.clone();
        slice.finish &= start + length == end;
        reply = append(selected, wanted, &slice, &rows, start, length)?;
    }
    Ok(reply)
}

impl Formatter {
    /// Copy one explicitly requested source prefix once all its bytes are admitted.
    pub fn copy(&mut self, source: Source) -> Result<u64, TransferError> {
        let producer = self.producer.as_ref().ok_or(TransferError::Retired)?;
        if producer.job.revision != source.revision {
            return Err(TransferError::Replaced);
        }
        let id = self.begin_copy()?;
        self.producer.as_mut().expect("active copy").copy = Some((id, source));
        self.pump()?;
        Ok(id)
    }

    /// Start one private selection epoch, independent of the source job used for each entry.
    pub fn begin_copy(&mut self) -> Result<u64, TransferError> {
        self.next_copy = self
            .next_copy
            .checked_add(1)
            .ok_or(TransferError::Exhausted)?;
        self.wanted.copy.store(self.next_copy, Ordering::Release);
        if let Some(producer) = &mut self.producer {
            producer.copy = None;
        }
        Ok(self.next_copy)
    }

    /// Feed already painted selection rows without retaining their complete original source.
    pub fn selection_rows(
        &self,
        slice: SelectionSlice,
        rows: crate::app::render_entry_cache::PaintedRows,
        first: usize,
        length: usize,
    ) -> Result<bool, TransferError> {
        send(
            &self.sender,
            Request::SelectionRows {
                slice,
                rows,
                first,
                length,
            },
        )
    }

    /// Cancel a private copy without changing a source watch or backend session.
    pub fn cancel_copy(&mut self) {
        self.wanted.copy.store(0, Ordering::Release);
        if let Some(producer) = &mut self.producer {
            producer.copy = None;
        }
        // A full queue already wakes the owner; an idle owner must also release its partial copy.
        let _ = self.sender.try_send(Request::CancelCopy);
    }
}

/// Keep a malformed or canceled cached-row copy local to its private clipboard request.
pub(super) fn reply(
    selected: &mut Option<Selected>,
    wanted: &Wanted,
    slice: &SelectionSlice,
    rows: &PaintedRows,
    first: usize,
    length: usize,
) -> Reply {
    append(selected, wanted, slice, rows, first, length).unwrap_or_else(|error| Reply::CopyFailed {
        copy: slice.copy,
        error,
    })
}
