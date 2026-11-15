//! Consume numbered session changes before reading transcript render caches.

use carlos_core::session::events::Change;

use super::state::AppState;

impl AppState {
    pub(super) fn consume_core_events(&mut self) {
        let batch = self.session.publish();
        let mut dirty = None;
        for record in batch.records() {
            if self.view.core_cursor.checked_next() != Some(record.seq) {
                dirty = Some(0);
            }
            self.view.core_cursor = record.seq;
            let index = match record.change {
                Change::Added(id)
                | Change::Replaced(id)
                | Change::Facts(id)
                | Change::Text { entry: id, .. } => batch.position(id),
                Change::Reset { dirty_from } => {
                    Some(dirty_from.and_then(|id| batch.position(id)).unwrap_or(0))
                }
                Change::Approval(_)
                | Change::ApprovalResolved { .. }
                | Change::Info
                | Change::Goal
                | Change::Queue
                | Change::Tasks
                | Change::Settings => None,
            };
            if let Some(index) = index {
                dirty = Some(dirty.map_or(index, |old: usize| old.min(index)));
            }
        }
        if self.view.core_cursor != batch.sequence() {
            dirty = Some(0);
            self.view.core_cursor = batch.sequence();
        }
        if let Some(index) = dirty {
            self.view
                .render_cache
                .mark_transcript_dirty_from(batch.entry_count(), index);
        }
    }
}
