//! Read bounded client pages and tool output at a sealed session cursor.

use carlos_proto::ids::EntryId as WireEntryId;
use carlos_proto::session::{History, Snapshot};

use super::entry_id::EntryId;
use super::events::Published;

impl Published<'_> {
    /// Export the newest requested rows in chronological order; clamp to 1..=1000.
    pub fn snapshot(&self, limit: usize) -> Snapshot {
        self.session.export_snapshot(limit)
    }

    /// Fetch older stored rows; retired or foreign cursors fail instead of changing position.
    pub fn history(&self, before: &WireEntryId, limit: usize) -> Option<History> {
        let end = self.session.messages.position(EntryId::from_wire(before))?;
        let start = super::event_rows::page_start(&self.session.messages[..end], limit);
        Some(History {
            entries: self.session.messages[start..end]
                .iter()
                .map(|m| self.session.export_entry(m))
                .collect(),
            has_older: start > 0,
            seq: self.sequence(),
        })
    }
}

impl crate::session::state::SessionState {
    pub(super) fn export_snapshot(&self, limit: usize) -> Snapshot {
        let count = self.messages.len();
        let start = super::event_rows::page_start(&self.messages[..count], limit);
        Snapshot {
            info: self.events.info.clone().expect("published info"),
            settings: self.events.settings.clone(),
            goal: self.export_goal(),
            queue: self.export_queue(),
            tasks: self.export_tasks(),
            entries: self.messages[start..]
                .iter()
                .map(|m| self.export_entry(m))
                .collect(),
            approvals: self.events.approvals.clone(),
            seq: self.events.seq,
            has_older: start > 0,
        }
    }
}
