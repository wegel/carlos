//! Keep sparse body revisions while ordinary new rows need no extra per-row storage.

use super::entry_id::EntryId;
use super::event_rows::is_tool;
use super::models::EntryDraft;
use super::transcript::Transcript;

impl Transcript {
    /// Read an output body generation without allocating a record for the initial body.
    pub(super) fn output_revision(&self, id: EntryId) -> u64 {
        self.output_revisions.get(&id).copied().unwrap_or(0)
    }

    pub(super) fn replace_output(&mut self, index: usize) {
        if index >= self.journal.published {
            return;
        }
        if let Some(message) = self.entries.get(index).filter(|message| is_tool(message)) {
            let next = self
                .output_revision(message.id)
                .checked_add(1)
                .expect("output revision exhausted");
            self.output_revisions.insert(message.id, next);
        }
    }

    pub(super) fn retain_output_revision(
        &self,
        adopted: &mut Transcript,
        id: EntryId,
        draft: &EntryDraft,
    ) {
        let Some(old) = self.entry(id) else {
            return;
        };
        let changed = old.text != draft.text || old.kind != draft.kind;
        let revision = self.output_revision(id);
        let next = if changed && is_tool(old) {
            revision.checked_add(1).expect("output revision exhausted")
        } else {
            revision
        };
        if next != 0 {
            adopted.output_revisions.insert(id, next);
        }
    }
}
