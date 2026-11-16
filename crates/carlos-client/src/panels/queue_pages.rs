//! Retain one queue preview page and its bookmarks while finding the requested row.

use carlos_proto::ids::{QueueEntryId, QueueRevision};
use carlos_proto::queue::{QueueEntry, QueuePage, MAX_ENTRIES, MAX_PAGE_ENTRIES};

use crate::ClientError;

#[derive(Default)]
enum Target {
    #[default]
    Selected,
    Position(usize),
}

/// Keep previews for one page and addresses for at most the native queue's 128 pages.
#[derive(Default)]
pub struct QueuePageWindow {
    revision: Option<QueueRevision>,
    entries: Vec<QueueEntry>,
    position: Option<usize>,
    settled: bool,
    start: usize,
    next_start: usize,
    cursors: Vec<Option<QueueEntryId>>,
    target: Target,
}

impl QueuePageWindow {
    pub(crate) fn retained_bytes(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| {
                entry.id.as_str().len()
                    + entry.client_message.len()
                    + entry.preview.len()
                    + entry.display_id.as_ref().map_or(0, String::len)
                    + 128
            })
            .sum::<usize>()
            + self
                .cursors
                .iter()
                .flatten()
                .map(|id| id.as_str().len() + 32)
                .sum::<usize>()
            + 512
    }

    /// Borrow the revision that proves this window's rows.
    pub fn revision(&self) -> Option<&QueueRevision> {
        self.revision.as_ref()
    }

    /// Borrow the single retained preview page, not every page walked to find a target.
    pub fn entries(&self) -> &[QueueEntry] {
        &self.entries
    }

    /// Borrow the resolved global position of the device's requested target.
    pub fn position(&self) -> Option<usize> {
        self.position
    }

    /// Report whether the requested position or identity has finished its cursor walk.
    pub fn is_settled(&self) -> bool {
        self.settled
    }

    /// Require a fresh revision before native use while retaining the last displayed row.
    pub fn invalidate(&mut self) {
        self.revision = None;
    }

    /// Retire stale previews without selecting a different native entry.
    pub fn reset(&mut self, revision: QueueRevision, selected: bool) {
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

    /// Borrow the requested row once the last page walk has settled; callers still check freshness.
    pub fn selected(&self, selected: Option<&str>) -> Option<&QueueEntry> {
        self.settled.then_some(())?;
        self.entries
            .iter()
            .find(|entry| Some(entry.id.as_str()) == selected)
    }

    /// Return the next explicit page address without retaining its preview text.
    pub fn next(&self) -> Option<(usize, Option<QueueEntryId>)> {
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

    /// Move the requested list target, reading intervening pages only when their bookmarks are unknown.
    pub fn navigate(&mut self, direction: isize, count: usize, selected: &mut Option<String>) {
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
            (index as i128 + direction as i128).rem_euclid(count as i128) as usize
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
    pub fn accept(
        &mut self,
        page: QueuePage,
        start: usize,
        selected: &mut Option<String>,
    ) -> Result<(), ClientError> {
        if self.revision.as_ref() != Some(&page.queue.revision) {
            return Err(ClientError::StaleQueue);
        }
        let count = page.queue.count as usize;
        let end = start
            .checked_add(page.entries.len())
            .ok_or_else(|| ClientError::InvalidPayload("Queue page interval overflowed".into()))?;
        if count > MAX_ENTRIES
            || end > count
            || page.entries.len() > MAX_PAGE_ENTRIES
            || page.has_more && (page.entries.len() != MAX_PAGE_ENTRIES || end >= count)
            || !page.has_more && end != count
        {
            return Err(ClientError::InvalidPayload(
                "Queue page does not match its declared count; refresh before editing".into(),
            ));
        }
        if page.has_more {
            let next = start / MAX_PAGE_ENTRIES + 1;
            if next > self.cursors.len() {
                return Err(ClientError::InvalidPayload(
                    "Queue page skipped an unknown cursor".into(),
                ));
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
