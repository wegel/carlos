//! Cache exact task text ranges once and copy only rows visible in the shared panel.

use std::ops::Range;
use std::slice::from_ref;

use super::text::pages::{visit_ranges, SourceRanges};

struct RowRun {
    bytes: Range<u32>,
    step: u32,
    end: u32,
}

/// Compress consecutive rows with equal byte lengths and strides into one bounded record.
#[derive(Default)]
struct Rows {
    runs: Vec<RowRun>,
    count: u32,
}

impl Rows {
    fn push(&mut self, bytes: Range<usize>) {
        let bytes = u32::try_from(bytes.start).expect("bounded panel bytes")
            ..u32::try_from(bytes.end).expect("bounded panel bytes");
        let previous_end = self.runs.iter().rev().nth(1).map_or(0, |run| run.end);
        if let Some(last) = self.runs.last_mut() {
            let count = last.end - previous_end;
            let previous = last.bytes.start + last.step * (count - 1);
            if last.bytes.len() == bytes.len() && bytes.start >= previous {
                let step = bytes.start - previous;
                if count == 1 || step == last.step {
                    last.step = step;
                    last.end += 1;
                    self.count += 1;
                    return;
                }
            }
        }
        self.count += 1;
        self.runs.push(RowRun {
            bytes,
            step: 0,
            end: self.count,
        });
    }

    fn window(&self, start: usize, length: usize, mut visit: impl FnMut(Range<usize>)) {
        let first = self.runs.partition_point(|run| run.end as usize <= start);
        let end = start.saturating_add(length).min(self.count as usize);
        for (index, run) in self.runs.iter().enumerate().skip(first) {
            let before = index
                .checked_sub(1)
                .map_or(0, |i| self.runs[i].end as usize);
            if before >= end {
                break;
            }
            for row in start.max(before)..end.min(run.end as usize) {
                let offset = (row - before) * run.step as usize;
                visit(run.bytes.start as usize + offset..run.bytes.end as usize + offset);
            }
        }
    }
}

/// Keep one sanitized body, source map and compressed row index for one panel width.
#[derive(Default)]
pub(super) struct PanelRows {
    width: usize,
    original: String,
    text: SourceRanges,
    rows: Rows,
}

impl PanelRows {
    /// Rebuild only after the caller changes text or the terminal width.
    pub(super) fn rebuild(&mut self, lines: Vec<String>, width: usize) {
        self.width = width.max(1);
        self.original = lines.join("\n");
        self.text = SourceRanges::default();
        self.rows = Rows::default();
        let mut start = 0;
        for line in self.original.split('\n') {
            let offset = self.text.bytes();
            let original = start..start + line.len();
            let expanded = self
                .text
                .append_line(from_ref(&original), &self.original, line);
            visit_ranges(&expanded, self.width, |_, range, _| {
                self.rows.push(offset + range.start..offset + range.end);
            });
            start += line.len() + 1;
        }
        self.text.shrink_to_fit();
        self.rows.runs.shrink_to_fit();
    }

    /// Detect a resize without hashing or cloning the unchanged task body.
    pub(super) fn width(&self) -> usize {
        self.width
    }

    /// Clamp the same scroll offset as the former full-body painter and copy only visible rows.
    pub(super) fn window(&self, scroll: usize, height: usize) -> Vec<String> {
        let start = scroll.min((self.rows.count as usize).saturating_sub(height));
        let mut lines = Vec::with_capacity(height.min(self.rows.count as usize));
        self.rows.window(start, height, |range| {
            lines.push(self.text.text(&self.original, range))
        });
        lines
    }
}

#[cfg(test)]
#[path = "task_panel_rows_tests.rs"]
mod tests;
