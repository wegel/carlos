//! Report native item edits and explicit client cursor changes synchronously.

use super::transcript_updates::SessionUpdate;

/// Carry redraw reports without storing any client widget or focus.
#[derive(Default)]
pub(crate) struct ItemChanges {
    pub(crate) transcript: TranscriptChanges,
    pub(crate) reset_navigation: bool,
}

/// Keep ordinary item reports on the stack; full history may spill into a vector.
#[derive(Default)]
pub(crate) struct TranscriptChanges {
    inline: [SessionUpdate; 4],
    count: usize,
    overflow: Vec<SessionUpdate>,
}

impl ItemChanges {
    /// Keep the original order of changed transcript ranges.
    pub(in crate::session) fn push(&mut self, update: SessionUpdate) {
        if update.dirty_from.is_none() && !update.transcript_reset {
            return;
        }
        let reports = &mut self.transcript;
        if reports.count < reports.inline.len() {
            reports.inline[reports.count] = update;
            reports.count += 1;
        } else {
            reports.overflow.push(update);
        }
    }

    /// Record a mutation and return its original result.
    pub(in crate::session) fn track<T>(&mut self, (result, update): (T, SessionUpdate)) -> T {
        self.push(update);
        result
    }
}

impl TranscriptChanges {
    /// Check whether the handler changed any transcript row.
    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl IntoIterator for TranscriptChanges {
    type Item = SessionUpdate;
    type IntoIter = std::iter::Chain<
        std::iter::Take<std::array::IntoIter<SessionUpdate, 4>>,
        std::vec::IntoIter<SessionUpdate>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.inline
            .into_iter()
            .take(self.count)
            .chain(self.overflow)
    }
}
