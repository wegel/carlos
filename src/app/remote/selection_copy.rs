//! Fetch missing selected rows in bounded batches without replacing the visible viewport.

use std::collections::VecDeque;

use carlos_proto::ids::EntryId;

use super::transcript::Transcript;
use super::Device;
use crate::app::render_cache_state::{SelectionBlock, TranscriptSource};
use crate::app::render_entry_cache::PaintedRows;
use crate::app::selection::Selection;
use crate::app::transcript_body::formatter::SelectionTarget;
use crate::app::transcript_body::formatter::{Facts, SelectionSlice, Target, View};
use crate::clipboard::{clipboard_backend_label, ClipboardBackend};

enum Content {
    Rows(PaintedRows),
    Source { entry: EntryId, view: View },
}

struct Part {
    base: usize,
    first: usize,
    last: usize,
    content: Content,
}

/// Retain entry addresses and small shared painted windows, never every selected body row.
pub(super) struct SelectionCopy {
    parts: VecDeque<Part>,
    bounds: [usize; 4],
    pub id: Option<u64>,
    waiting: bool,
}

impl SelectionCopy {
    pub fn entry(&self) -> Option<&EntryId> {
        match &self.parts.front()?.content {
            Content::Rows(_) => None,
            Content::Source { entry, .. } => Some(entry),
        }
    }

    pub fn revision(&self) -> Option<carlos_proto::output::OutputRevision> {
        match &self.parts.front()?.content {
            Content::Source {
                view:
                    View {
                        target: Target::Selection(target),
                        ..
                    },
                ..
            } => Some(target.source.revision),
            _ => None,
        }
    }

    pub fn view(&self) -> Option<(EntryId, View)> {
        let part = self.parts.front()?;
        let Content::Source { entry, view } = &part.content else {
            return None;
        };
        let mut view = view.clone();
        view.first = part.first;
        view.length = part.last - part.first + 1;
        if let Target::Selection(target) = &mut view.target {
            target.slice = self.slice(part);
        }
        Some((entry.clone(), view))
    }

    fn slice(&self, part: &Part) -> SelectionSlice {
        SelectionSlice {
            copy: self.id.expect("started selection"),
            bounds: self.bounds,
            base: part.base,
            finish: self.parts.len() == 1,
        }
    }

    fn advanced(&mut self) {
        self.parts.pop_front();
        self.waiting = false;
    }
}

impl Device {
    /// Freeze the same clipped interval used by the existing selected-text function.
    pub(super) fn begin_selection_copy(&mut self, selection: Selection) -> anyhow::Result<()> {
        let Some(transcript) = Transcript::new(&self.client, &self.session, &self.documents) else {
            return Ok(());
        };
        let cache = &mut self.view.render_cache;
        let total = cache.rendered_line_count();
        if total == 0 {
            return Ok(());
        }
        let first = selection
            .anchor_line_idx
            .min(selection.focus_line_idx)
            .min(total - 1);
        let last = selection
            .anchor_line_idx
            .max(selection.focus_line_idx)
            .min(total - 1);
        cache.materialize_inputs(&transcript, first, last);
        let start = cache.message_anchor(first).expect("selected first row").0;
        let end = cache.message_anchor(last).expect("selected last row").0;
        let bounds = [
            selection.anchor_x,
            selection.anchor_line_idx,
            selection.focus_x,
            selection.focus_line_idx,
        ];
        let mut parts = VecDeque::new();
        for index in start..=end {
            let block = cache.selection_block(index).expect("captured block");
            if block.count == 0 {
                continue;
            }
            let row = transcript.row(index).expect("selected entry");
            let entry = self
                .client
                .session(&self.session)
                .and_then(|replica| replica.entry_at(index));
            parts.push_back(capture(
                block,
                first,
                last,
                row.facts,
                entry.map(|entry| entry.id.clone()),
                bounds,
            )?);
        }
        self.documents.copy = None;
        self.documents.formatting.cancel_copy();
        self.documents.selection = Some(SelectionCopy {
            parts,
            bounds,
            id: None,
            waiting: false,
        });
        self.view.status_hint = Some("Copying selected text…".into());
        Ok(())
    }

    /// Queue at most one row batch; the worker acknowledges it before the next source is opened.
    pub(super) fn drive_selection_copy(&mut self) -> anyhow::Result<()> {
        let missing = self
            .documents
            .selection
            .as_ref()
            .and_then(|selection| selection.entry())
            .is_some_and(|id| {
                self.client
                    .session(&self.session)
                    .is_none_or(|replica| replica.entry(id).is_none())
            });
        if missing {
            self.documents.selection = None;
            self.documents.formatting.end_selection();
            self.view.status_hint = Some("Selected entry is no longer available".into());
            return Ok(());
        }
        let Some(selection) = &mut self.documents.selection else {
            return Ok(());
        };
        if selection.id.is_none() {
            selection.id = Some(self.documents.formatting.begin_copy()?);
        }
        if selection.waiting {
            return Ok(());
        }
        let Some(part) = selection.parts.front() else {
            return Ok(());
        };
        if let Content::Rows(rows) = &part.content {
            let length = part.last - part.first + 1;
            selection.waiting = self.documents.formatting.selection_rows(
                selection.slice(part),
                rows.clone(),
                part.first,
                length,
            )?;
        }
        Ok(())
    }

    pub(super) fn selection_part_copied(&mut self, id: u64) {
        if let Some(selection) = &mut self.documents.selection {
            if selection.id == Some(id) {
                selection.advanced();
            }
        }
    }

    pub(super) fn selection_copied(&mut self, id: u64, backend: ClipboardBackend) -> bool {
        if self
            .documents
            .selection
            .as_ref()
            .is_none_or(|selection| selection.id != Some(id))
        {
            return false;
        }
        self.documents.selection = None;
        self.documents.formatting.end_selection();
        self.set_status_notice(format!(
            "copied selection ({})",
            clipboard_backend_label(backend)
        ));
        true
    }
}

fn capture(
    block: SelectionBlock,
    first: usize,
    last: usize,
    facts: crate::app::transcript_input::RenderInput<'_>,
    entry: Option<EntryId>,
    bounds: [usize; 4],
) -> anyhow::Result<Part> {
    let first = first.saturating_sub(block.start);
    let last = last.saturating_sub(block.start).min(block.count - 1);
    let content = match block.rows {
        Some(rows) if rows.covers(first, last) => Content::Rows(rows),
        _ => {
            let source = block
                .source
                .ok_or_else(|| anyhow::anyhow!("Wait for the selected source to load"))?;
            Content::Source {
                entry: entry
                    .ok_or_else(|| anyhow::anyhow!("Selected entry is no longer available"))?,
                view: View {
                    facts: Facts::from_input(facts),
                    format: block.format,
                    previous: block.previous,
                    first,
                    length: last - first + 1,
                    target: Target::Selection(SelectionTarget {
                        slice: SelectionSlice {
                            copy: 0,
                            bounds,
                            base: block.start,
                            finish: false,
                        },
                        source,
                        count: block.count,
                    }),
                },
            }
        }
    };
    Ok(Part {
        base: block.start,
        first,
        last,
        content,
    })
}
