//! Read bounded client pages and tool output at a sealed session cursor.

use carlos_proto::ids::EntryId as WireEntryId;
use carlos_proto::session::{History, Snapshot};

use super::entry_id::EntryId;
use super::events::Published;

impl Published<'_> {
    /// Export the newest requested rows in chronological order; clamp to 1..=1000.
    pub fn snapshot(&self, limit: usize) -> Snapshot {
        let count = self.session.messages.len();
        let start = count.saturating_sub(limit.clamp(1, 1000));
        Snapshot {
            info: self.session.events.info.clone().expect("published info"),
            entries: self.session.messages[start..]
                .iter()
                .map(|m| self.session.export_entry(m))
                .collect(),
            approvals: self.session.events.approvals.clone(),
            seq: self.sequence(),
            has_older: start > 0,
        }
    }

    /// Fetch older stored rows; retired or foreign cursors fail instead of changing position.
    pub fn history(&self, before: &WireEntryId, limit: usize) -> Option<History> {
        let end = self.session.messages.position(EntryId::from_wire(before))?;
        let start = end.saturating_sub(limit.clamp(1, 1000));
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
