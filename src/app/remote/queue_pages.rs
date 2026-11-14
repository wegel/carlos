//! Keep one bounded queue preview page and opaque cursors while preserving the selected identity.

use anyhow::{bail, Result};
use carlos_proto::ids::{QueueEntryId, QueueRevision};
use carlos_proto::queue::{QueueEntry, QueuePage, MAX_PAGE_ENTRIES};

#[derive(Default)]
enum Target {
    #[default]
    Selected,
    Position(usize),
}

/// Keep previews for one page and addresses for at most the native queue's 128 pages.
#[derive(Default)]
pub(super) struct Pages {
    pub(super) revision: Option<QueueRevision>,
    pub(super) entries: Vec<QueueEntry>,
    pub(super) position: Option<usize>,
    pub(super) settled: bool,
    start: usize,
    next_start: usize,
    cursors: Vec<Option<QueueEntryId>>,
    target: Target,
}

impl Pages {
    /// Retire stale previews without selecting a different native entry.
    pub(super) fn reset(&mut self, revision: QueueRevision, selected: bool) {
        *self = Self {
            revision: Some(revision),
            cursors: vec![None],
            target: if selected {
                Target::Selected
            } else {
                Target::Position(0)
            },
            ..Self::default()
        };
    }

    /// Borrow only the selected entry currently proven by this page's revision.
    pub(super) fn selected(&self, selected: Option<&str>) -> Option<&QueueEntry> {
        self.settled.then_some(())?;
        self.entries
            .iter()
            .find(|entry| Some(entry.id.as_str()) == selected)
    }

    /// Return the next explicit page address without retaining its preview text.
    pub(super) fn next(&self) -> Option<(usize, Option<QueueEntryId>)> {
        if self.settled {
            return None;
        }
        Some((
            self.next_start,
            self.cursors
                .get(self.next_start / MAX_PAGE_ENTRIES)?
                .clone(),
        ))
    }

    /// Move only after an explicit arrow, reading intervening pages when their addresses are unknown.
    pub(super) fn navigate(
        &mut self,
        direction: isize,
        count: usize,
        selected: &mut Option<String>,
    ) {
        if count == 0 {
            *selected = None;
            self.position = None;
            return;
        }
        let current = match self.target {
            Target::Position(index) if !self.settled => Some(index),
            _ => self.position,
        };
        let index = current.map_or(0, |index| {
            (index as isize + direction).rem_euclid(count as isize) as usize
        });
        self.target = Target::Position(index);
        self.settled = false;
        if let Some(entry) = index
            .checked_sub(self.start)
            .and_then(|index| self.entries.get(index))
        {
            *selected = Some(entry.id.to_string());
            self.position = Some(index);
            self.settled = true;
        } else {
            self.next_start = (index / MAX_PAGE_ENTRIES).min(self.cursors.len().saturating_sub(1))
                * MAX_PAGE_ENTRIES;
        }
    }

    /// Accept a bounded page, stopping once the selected id or requested ordinal is present.
    pub(super) fn accept(
        &mut self,
        page: QueuePage,
        start: usize,
        selected: &mut Option<String>,
    ) -> Result<()> {
        let count = page.queue.count as usize;
        let end = start + page.entries.len();
        if end > count
            || page.entries.len() > MAX_PAGE_ENTRIES
            || page.has_more && (page.entries.len() != MAX_PAGE_ENTRIES || end >= count)
            || !page.has_more && end != count
        {
            bail!("Queue page does not match its declared count; refresh before editing");
        }
        if page.has_more {
            let next = start / MAX_PAGE_ENTRIES + 1;
            if next > self.cursors.len() {
                bail!("Queue page skipped an unknown cursor");
            }
            let cursor = page.entries.last().expect("nonempty page").id.clone();
            if next == self.cursors.len() {
                self.cursors.push(Some(cursor));
            } else {
                self.cursors[next] = Some(cursor);
            }
        }
        self.start = start;
        self.entries = page.entries;
        self.position = self
            .entries
            .iter()
            .position(|entry| Some(entry.id.as_str()) == selected.as_deref())
            .map(|index| start + index);
        self.settled = match self.target {
            Target::Selected => self.position.is_some() || !page.has_more,
            Target::Position(index) => self.select_position(index, count, selected),
        };
        self.next_start = end;
        Ok(())
    }

    fn select_position(
        &mut self,
        index: usize,
        count: usize,
        selected: &mut Option<String>,
    ) -> bool {
        if count == 0 {
            *selected = None;
            self.position = None;
            return true;
        }
        if let Some(entry) = index
            .checked_sub(self.start)
            .and_then(|index| self.entries.get(index))
        {
            *selected = Some(entry.id.to_string());
            self.position = Some(index);
            true
        } else {
            false
        }
    }
}
