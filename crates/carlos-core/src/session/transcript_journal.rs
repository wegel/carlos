//! Coalesce unpublished edits without cloning text or keeping an unbounded log.

use super::entry_id::EntryId;

pub(super) const MAX_PENDING: usize = 256;

#[derive(Debug, Clone, Copy)]
pub(super) struct RowChange {
    pub(super) id: EntryId,
    pub(super) index: usize,
    pub(super) replace: bool,
    pub(super) delta: Option<usize>,
    pub(super) facts: bool,
}

#[derive(Debug)]
pub(super) struct Journal {
    pub(super) published: usize,
    pub(super) reset: bool,
    pub(super) overflow: bool,
    pub(super) earliest: usize,
    pub(super) rows: Vec<RowChange>,
}

impl Default for Journal {
    fn default() -> Self {
        Self {
            published: 0,
            reset: true,
            overflow: false,
            earliest: usize::MAX,
            rows: Vec::new(),
        }
    }
}

impl Journal {
    pub(super) fn record(&mut self, change: RowChange) {
        if self.reset || change.index >= self.published {
            return;
        }
        self.earliest = self.earliest.min(change.index);
        if self.overflow {
            return;
        }
        if let Some(old) = self.rows.iter_mut().find(|old| old.id == change.id) {
            old.replace |= change.replace;
            old.facts |= change.facts;
            if old.delta.is_none() {
                old.delta = change.delta;
            }
        } else if self.rows.len() == MAX_PENDING {
            self.overflow = true;
            self.rows.clear();
        } else {
            self.rows.push(change);
        }
    }
}
