//! Keep the nearest 32 native rows while scanning arbitrarily many saved-session pages.

use std::cmp::{Ordering, Reverse};

use super::{SavedPage, PAGE_ROWS};
use crate::session::models::ThreadSummary;

/// Capture a native ordering boundary inside the daemon, never in the socket protocol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageAnchor {
    updated_at: i64,
    created_at: i64,
    native_id: String,
}

/// Select one neighboring window without retaining all earlier native cursor pages.
#[derive(Clone, Debug)]
pub enum PageDirection {
    Newest,
    Older(PageAnchor),
    Newer(PageAnchor),
}

impl PageAnchor {
    /// Capture exactly the selected row's native tie-breaker for the next worker.
    pub fn from_row(row: &ThreadSummary) -> Self {
        Self {
            updated_at: row.updated_at,
            created_at: row.created_at,
            native_id: row.id.clone(),
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        (
            Reverse(self.updated_at),
            Reverse(self.created_at),
            &self.native_id,
        )
            .cmp(&(
                Reverse(other.updated_at),
                Reverse(other.created_at),
                &other.native_id,
            ))
    }
}

pub(super) struct Window {
    direction: PageDirection,
    rows: Vec<ThreadSummary>,
    first: Option<PageAnchor>,
    last: Option<PageAnchor>,
}

impl Window {
    pub(super) fn new(direction: PageDirection) -> Self {
        Self {
            direction,
            rows: Vec::with_capacity(PAGE_ROWS + 1),
            first: None,
            last: None,
        }
    }

    pub(super) fn push(&mut self, mut row: ThreadSummary) {
        if row.id.is_empty() || row.id.len() > 4096 || row.cwd.len() > 16384 {
            return;
        }
        clip(&mut row.preview, 4096);
        if let Some(name) = &mut row.name {
            clip(name, 512);
        }
        let key = PageAnchor::from_row(&row);
        if self
            .first
            .as_ref()
            .is_none_or(|first| key.compare(first).is_lt())
        {
            self.first = Some(key.clone());
        }
        if self
            .last
            .as_ref()
            .is_none_or(|last| key.compare(last).is_gt())
        {
            self.last = Some(key.clone());
        }
        let eligible = match &self.direction {
            PageDirection::Newest => true,
            PageDirection::Older(anchor) => key.compare(anchor).is_gt(),
            PageDirection::Newer(anchor) => key.compare(anchor).is_lt(),
        };
        if !eligible || self.rows.iter().any(|old| old.id == row.id) {
            return;
        }
        let index = self
            .rows
            .binary_search_by(|old| PageAnchor::from_row(old).compare(&key))
            .unwrap_or_else(|index| index);
        self.rows.insert(index, row);
        if self.rows.len() > PAGE_ROWS {
            if matches!(self.direction, PageDirection::Newer(_)) {
                self.rows.remove(0);
            } else {
                self.rows.pop();
            }
        }
    }

    pub(super) fn is_complete_before(&self, updated_at: i64) -> bool {
        match &self.direction {
            PageDirection::Newer(anchor) => updated_at < anchor.updated_at,
            PageDirection::Newest | PageDirection::Older(_) => {
                self.rows.len() == PAGE_ROWS
                    && self
                        .rows
                        .last()
                        .is_some_and(|last| updated_at < last.updated_at)
            }
        }
    }

    pub(super) fn finish(self) -> SavedPage {
        let first = self.rows.first().map(PageAnchor::from_row);
        let last = self.rows.last().map(PageAnchor::from_row);
        SavedPage {
            has_newer: first
                .zip(self.first)
                .is_some_and(|(row, first)| first.compare(&row).is_lt()),
            has_older: last
                .zip(self.last)
                .is_some_and(|(row, last)| last.compare(&row).is_gt()),
            rows: self.rows,
        }
    }
}

fn clip(text: &mut String, limit: usize) {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
}
