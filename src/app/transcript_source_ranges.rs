//! Rebuild requested text from original spans and sparse tab positions.

use std::mem::size_of;
use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::app::text::visual_width;

struct Run {
    end: usize,
    original: usize,
}

struct Tab {
    end: usize,
    plain: usize,
}

/// Map expanded logical text to the one retained original source.
#[derive(Default)]
pub(in crate::app) struct SourceRanges {
    runs: Vec<Run>,
    tabs: Vec<Tab>,
    plain_bytes: usize,
    expanded_bytes: usize,
}

impl SourceRanges {
    /// Map one complete logical line, including graphemes crossing style boundaries.
    pub fn append_line(&mut self, originals: &[Range<usize>], source: &str, plain: &str) -> String {
        let plain_offset = self.plain_bytes;
        for original in originals {
            if !original.is_empty() {
                self.plain_bytes += original.len();
                self.runs.push(Run {
                    end: self.plain_bytes,
                    original: original.start,
                });
            }
        }
        debug_assert_eq!(self.plain_bytes - plain_offset, plain.len());
        debug_assert!(originals
            .iter()
            .all(|bytes| source.get(bytes.clone()).is_some()));
        let expanded = self.expand(plain, plain_offset);
        self.expanded_bytes += expanded.len();
        expanded
    }

    /// Locate the next appended text in the flattened expanded coordinates.
    pub fn bytes(&self) -> usize {
        self.expanded_bytes
    }

    /// Copy only requested expanded bytes, including parts of a tab's spaces.
    pub fn text(&self, source: &str, wanted: Range<usize>) -> String {
        self.text_with(wanted, |bytes, output| output.push_str(&source[bytes]))
    }

    /// Rebuild requested bytes through an accessor to separately retained original spans.
    pub fn text_with(
        &self,
        wanted: Range<usize>,
        mut copy: impl FnMut(Range<usize>, &mut String),
    ) -> String {
        let mut result = String::with_capacity(wanted.len());
        let mut index = self.tabs.partition_point(|tab| tab.end <= wanted.start);
        let mut added = index.checked_sub(1).map_or(0, |before| {
            self.tabs[before].end - self.tabs[before].plain - 1
        });
        let mut byte = wanted.start;
        while byte < wanted.end {
            let Some(tab) = self.tabs.get(index) else {
                self.copy_plain(byte - added..wanted.end - added, &mut copy, &mut result);
                break;
            };
            let start = tab.plain + added;
            if byte < start {
                let end = wanted.end.min(start);
                self.copy_plain(byte - added..end - added, &mut copy, &mut result);
                byte = end;
            }
            if byte >= wanted.end {
                break;
            }
            let end = wanted.end.min(tab.end);
            result.extend(std::iter::repeat_n(' ', end - byte));
            byte = end;
            if byte == tab.end {
                added = tab.end - tab.plain - 1;
                index += 1;
            }
        }
        result
    }

    /// Include every retained span and tab slot in the caller's metadata measurement.
    pub fn capacity_bytes(&self) -> usize {
        self.runs.capacity() * size_of::<Run>() + self.tabs.capacity() * size_of::<Tab>()
    }

    /// Release unused source-span and tab slots after a complete long-line index.
    pub fn shrink_to_fit(&mut self) {
        self.runs.shrink_to_fit();
        self.tabs.shrink_to_fit();
    }

    fn expand(&mut self, plain: &str, plain_offset: usize) -> String {
        if !plain.contains('\t') {
            return plain.to_string();
        }
        let ascii = plain
            .bytes()
            .position(|byte| !byte.is_ascii() || matches!(byte, b'\t' | b'\r' | b'\n'))
            .unwrap_or(plain.len());
        // Keep the last ASCII scalar with a possible combining/emoji suffix.
        let prefix = ascii.saturating_sub(1);
        let mut column = prefix;
        let mut expanded = String::with_capacity(plain.len());
        expanded.push_str(&plain[..prefix]);
        for (byte, grapheme) in plain[prefix..].grapheme_indices(true) {
            if grapheme == "\t" {
                let spaces = 8 - column % 8;
                expanded.extend(std::iter::repeat_n(' ', spaces));
                self.tabs.push(Tab {
                    end: self.expanded_bytes + expanded.len(),
                    plain: plain_offset + prefix + byte,
                });
                column += spaces;
            } else {
                expanded.push_str(grapheme);
                column = if matches!(grapheme, "\r" | "\n") {
                    0
                } else {
                    column + visual_width(grapheme)
                };
            }
        }
        expanded
    }

    fn copy_plain(
        &self,
        wanted: Range<usize>,
        copy: &mut impl FnMut(Range<usize>, &mut String),
        result: &mut String,
    ) {
        let first = self.runs.partition_point(|run| run.end <= wanted.start);
        for index in first..self.runs.len() {
            let run = &self.runs[index];
            let start = index
                .checked_sub(1)
                .map_or(0, |before| self.runs[before].end);
            if start >= wanted.end {
                break;
            }
            let from = wanted.start.saturating_sub(start);
            let until = wanted.end.min(run.end) - start;
            copy(run.original + from..run.original + until, result);
        }
    }
}

#[cfg(test)]
#[path = "transcript_source_ranges_tests.rs"]
mod tests;
