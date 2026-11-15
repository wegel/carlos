//! Keep checked viewport frames while one worker owns the selected complete original.

use std::collections::HashMap;

use carlos_client::{Bodies, BodyToken};
use carlos_proto::ids::EntryId;
use carlos_proto::output::OutputRevision;
use carlos_proto::source::Source;

use crate::app::flow_transcript::EntryContext;
use crate::app::render_entry_cache::{PreparedRow, RowFormat};
use crate::app::transcript_body::formatter::{
    Facts, Formatter, JobId, Reply, Target, Transfer, View,
};
use crate::app::transcript_input::RenderInput;

struct Active {
    token: BodyToken,
    job: JobId,
    revision: OutputRevision,
    view: View,
}

struct Frame {
    source: Source,
    view: View,
    rows: PreparedRow,
}

/// Report new viewport rows or a private clipboard outcome without returning original bytes.
pub(super) enum Update {
    Rows(EntryId),
    Copied {
        copy: u64,
        backend: crate::clipboard::ClipboardBackend,
    },
    Selected(u64),
    CopyFailed {
        copy: u64,
        error: super::super::transcript_body::formatter::TransferError,
    },
}

/// Retain small frames independently of the active worker's original source.
#[derive(Default)]
pub(super) struct Formatting {
    worker: Option<Formatter>,
    active: Option<Active>,
    frames: HashMap<EntryId, Frame>,
}

impl Formatting {
    /// Start or move the one complete-source owner after the terminal has drawn.
    pub fn request(
        &mut self,
        token: &BodyToken,
        revision: OutputRevision,
        view: View,
    ) -> anyhow::Result<()> {
        if self.worker.is_none() {
            self.worker = Some(Formatter::new()?);
        }
        let worker = self.worker.as_mut().expect("created formatter");
        let job = match &self.active {
            Some(active) if active.token == *token && active.revision == revision => active.job,
            _ => worker.open(token.clone(), revision).map_err(format_error)?,
        };
        worker.request_view(view.clone()).map_err(format_error)?;
        self.active = Some(Active {
            token: token.clone(),
            job,
            revision,
            view,
        });
        Ok(())
    }

    /// Transfer only source bytes already accepted by the pure cache.
    pub fn feed(&mut self, bodies: &Bodies) -> anyhow::Result<Transfer> {
        let Some(worker) = self
            .worker
            .as_mut()
            .filter(|worker| worker.offset().is_some())
        else {
            return Ok(Transfer::Waiting);
        };
        worker.feed(bodies).map_err(format_error)
    }

    /// Poll independently of socket traffic so formatting also completes offline.
    pub fn poll(&mut self) -> anyhow::Result<Vec<Update>> {
        let Some(worker) = &mut self.worker else {
            return Ok(Vec::new());
        };
        let mut changed = Vec::new();
        for reply in worker.poll() {
            match reply {
                Reply::Frame {
                    job,
                    source,
                    prepared,
                    ..
                } => {
                    let Some(active) = self.active.as_ref().filter(|active| active.job == job)
                    else {
                        continue;
                    };
                    let id = active.token.entry().clone();
                    self.frames.insert(
                        id.clone(),
                        Frame {
                            source,
                            view: active.view.clone(),
                            rows: prepared,
                        },
                    );
                    changed.push(Update::Rows(id));
                }
                Reply::Copied { copy, backend, .. } => {
                    changed.push(Update::Copied { copy, backend })
                }
                Reply::Selected { copy } => changed.push(Update::Selected(copy)),
                Reply::CopyFailed { copy, error } => {
                    changed.push(Update::CopyFailed { copy, error })
                }
                Reply::Failed { error, .. } => return Err(format_error(error)),
                #[cfg(test)]
                Reply::Digest { .. } => {}
            }
        }
        Ok(changed)
    }

    /// Lend rows only when the current entry and preceding block still match their layout.
    pub fn prepared(
        &self,
        id: &EntryId,
        input: RenderInput<'_>,
        source: &Source,
        format: RowFormat,
        previous: Option<EntryContext>,
    ) -> Option<PreparedRow> {
        let frame = self.frames.get(id)?;
        if frame.source.revision != source.revision
            || frame.source.bytes > source.bytes
            || frame.view.facts != Facts::from_input(input)
            || frame.view.format != format
            || frame.view.previous != previous
        {
            return None;
        }
        let mut rows = frame.rows.clone();
        rows.loading |= frame.source.bytes != source.bytes;
        Some(rows)
    }

    /// Admit one private copy request without returning original bytes to the input thread.
    pub fn copy(&mut self, source: Source) -> anyhow::Result<u64> {
        self.worker
            .as_mut()
            .expect("active source owner")
            .copy(source)
            .map_err(format_error)
    }

    /// Start a selection even when every selected row is already painted.
    pub fn begin_copy(&mut self) -> anyhow::Result<u64> {
        if self.worker.is_none() {
            self.worker = Some(Formatter::new()?);
        }
        self.worker
            .as_mut()
            .expect("copy worker")
            .begin_copy()
            .map_err(format_error)
    }

    /// Queue a bounded literal chunk under the same permission as source and selection copies.
    pub fn literal_copy(
        &self,
        copy: u64,
        offset: usize,
        text: &str,
        finish: bool,
    ) -> anyhow::Result<bool> {
        self.worker
            .as_ref()
            .expect("copy worker")
            .literal_copy(copy, offset, text, finish)
            .map_err(format_error)
    }

    /// Reject stale copy status or chunks without canceling a newer explicit shortcut.
    pub fn copy_is_current(&self, copy: u64) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|worker| worker.copy_is_current(copy))
    }

    /// Queue shared rows without copying their strings into another terminal cache.
    pub fn selection_rows(
        &mut self,
        slice: super::super::transcript_body::formatter::SelectionSlice,
        rows: crate::app::render_entry_cache::PaintedRows,
        first: usize,
        length: usize,
    ) -> anyhow::Result<bool> {
        self.worker
            .as_ref()
            .expect("copy worker")
            .selection_rows(slice, rows, first, length)
            .map_err(format_error)
    }

    /// Keep a private copy from completing after the user replaces it.
    pub fn cancel_copy(&mut self) {
        if let Some(worker) = &mut self.worker {
            worker.cancel_copy();
        }
    }

    /// Remove the frozen-prefix read boundary after its copy finishes.
    pub fn end_selection(&mut self) {
        self.cancel_copy();
        let Some(active) = &mut self.active else {
            return;
        };
        if let Target::Selection(target) = &active.view.target {
            let worker = self.worker.as_mut().expect("active worker");
            if worker
                .offset()
                .is_some_and(|offset| offset.get() == target.source.bytes.get())
            {
                worker.clear_view();
                active.view.target = Target::Display;
                active.view.length = 0;
            } else {
                worker.retire();
                self.active = None;
            }
        }
    }

    /// Stop new reads while the worker consumes a complete frozen selection prefix.
    pub fn selection_ready(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| match &active.view.target {
                super::super::transcript_body::formatter::Target::Selection(target) => self
                    .worker
                    .as_ref()
                    .and_then(|worker| worker.offset())
                    .is_some_and(|offset| offset.get() >= target.source.bytes.get()),
                _ => false,
            })
    }

    /// Pause a watch for queued source bytes, not merely an unsent viewport description.
    pub fn has_unadmitted_bytes(&self, bodies: &Bodies) -> bool {
        self.active.as_ref().is_some_and(|active| {
            let end = bodies
                .source(&active.token)
                .map(|window| window.end().get());
            let admitted = self
                .worker
                .as_ref()
                .and_then(|worker| worker.offset())
                .map(|offset| offset.get());
            end.zip(admitted)
                .is_some_and(|(end, admitted)| end > admitted)
        })
    }

    /// Find the accepted window that must stay pinned until its pending piece is admitted.
    pub fn token(&self) -> Option<&BodyToken> {
        self.active.as_ref().map(|active| &active.token)
    }

    /// Retire all frames without spawning a second worker beside an unfinished old job.
    pub fn clear(&mut self) {
        if let Some(worker) = &mut self.worker {
            worker.cancel_copy();
            worker.retire();
        }
        self.active = None;
        self.frames.clear();
    }

    /// Forget a removed or replaced entry and reject already queued worker results.
    pub fn remove(&mut self, id: &EntryId) {
        self.frames.remove(id);
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.token.entry() == id)
        {
            self.worker.as_mut().expect("active worker").retire();
            self.active = None;
        }
    }
}

fn format_error(error: crate::app::transcript_body::formatter::TransferError) -> anyhow::Error {
    anyhow::anyhow!("Cannot format accepted source: {error:?}")
}
