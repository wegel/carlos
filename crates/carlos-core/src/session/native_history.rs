//! Keep native history cursors private and bound them to the original root and connection.

use std::collections::HashSet;

use anyhow::{ensure, Context, Result};
use carlos_proto::ids::EntryId as WireEntryId;
use serde_json::Value;

use super::entry_id::EntryId;
use super::state::SessionState;
use crate::backend::BackendKind;
use crate::protocol_history::{
    HistoryCursor, HistoryPage, MAX_HISTORY_BYTES, MAX_PAGES, MAX_RECORDS,
};

#[derive(Default)]
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
pub(super) struct NativeHistory {
    cursor: Option<HistoryCursor>,
    connection: (u64, u64),
    seen: HashSet<String>,
    turns: HashSet<String>,
    bytes: usize,
}

impl SessionState {
    /// Bind the initial native cursor and skipped-page ledger to this connection.
    pub(crate) fn install_native_history(
        &mut self,
        thread: &Value,
        continuation: Option<(HistoryCursor, Vec<String>)>,
    ) {
        let Some((cursor, visited)) = continuation else {
            self.native_history = Default::default();
            return;
        };
        let seen: HashSet<_> = visited.into_iter().collect();
        let turns: HashSet<_> = turn_ids(thread).collect();
        let bytes = seen.iter().chain(turns.iter()).map(String::len).sum();
        self.native_history = NativeHistory {
            seen,
            cursor: Some(cursor),
            connection: self.command_connection(BackendKind::Codex),
            turns,
            bytes,
        };
    }

    /// Report an older native continuation without treating a failed read as the end.
    pub(crate) fn has_older_native_history(&self) -> bool {
        self.native_history.cursor.is_some()
    }

    /// Fetch a native window only when stored rows cannot fill this checked client page.
    pub(crate) fn older_native_cursor(
        &self,
        before: &WireEntryId,
        limit: usize,
    ) -> Result<Option<HistoryCursor>> {
        let end = self
            .messages
            .position(EntryId::from_wire(before))
            .context("retired or foreign history cursor")?;
        if end >= limit.clamp(1, 1000) {
            return Ok(None);
        }
        let Some(cursor) = &self.native_history.cursor else {
            return Ok(None);
        };
        ensure!(
            self.native_history_current(cursor),
            "native history belongs to a retired root or connection"
        );
        Ok(Some(cursor.clone()))
    }

    /// Reject a late window from another root, connection or consumed continuation.
    pub(crate) fn native_history_current(&self, cursor: &HistoryCursor) -> bool {
        self.native_history.cursor.as_ref() == Some(cursor)
            && cursor.thread == *self.thread_id()
            && self.native_history.connection == self.command_connection(BackendKind::Codex)
    }

    /// Validate an older window before changing stored rows or consuming its native cursor.
    pub(crate) fn accept_older_native_history(
        &mut self,
        cursor: &HistoryCursor,
        mut page: HistoryPage,
    ) -> Result<usize> {
        ensure!(
            self.native_history_current(cursor),
            "native history read retired"
        );
        ensure!(
            page.thread["id"] == *self.thread_id(),
            "native history belongs to another root"
        );
        ensure!(
            page.visited
                .iter()
                .all(|cursor| !self.native_history.seen.contains(cursor)),
            "native history repeated a prior cursor"
        );
        if let Some(next) = &page.older {
            ensure!(
                next.thread == cursor.thread && !self.native_history.seen.contains(&next.next),
                "native history repeated or returned a foreign cursor"
            );
        }
        ensure!(
            page.visited.len() <= MAX_PAGES.saturating_sub(self.native_history.seen.len()),
            "native history exceeds 65536 pages"
        );
        let ids: Vec<_> = turn_ids(&page.thread).collect();
        let bytes = self.native_history.check_growth(&ids, &page.visited)?;
        let turns = page.thread["turns"]
            .as_array_mut()
            .context("native history has no turns")?;
        turns.retain(|turn| {
            !turn["id"]
                .as_str()
                .is_some_and(|id| self.native_history.turns.contains(id))
        });
        let count = self.prepend_native_thread(&page.thread);
        if page.older.is_none() {
            self.native_history = Default::default();
        } else {
            self.native_history.turns.extend(ids);
            self.native_history.seen.extend(page.visited);
            self.native_history.cursor = page.older;
            self.native_history.bytes += bytes;
        }
        Ok(count)
    }
}

fn turn_ids(thread: &Value) -> impl Iterator<Item = String> + '_ {
    thread["turns"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|turn| turn["id"].as_str().map(str::to_owned))
}

#[cfg(test)]
#[path = "native_history_tests.rs"]
mod tests;

impl NativeHistory {
    fn check_growth(&self, ids: &[String], visited: &[String]) -> Result<usize> {
        let added: Vec<_> = ids.iter().filter(|id| !self.turns.contains(*id)).collect();
        ensure!(
            added.len() <= MAX_RECORDS.saturating_sub(self.turns.len()),
            "native history exceeds 500000 turns"
        );
        let bytes = added.iter().map(|id| id.len()).sum::<usize>()
            + visited.iter().map(String::len).sum::<usize>();
        ensure!(
            bytes <= MAX_HISTORY_BYTES.saturating_sub(self.bytes),
            "native history cursor metadata exceeds 64 MiB"
        );
        Ok(bytes)
    }
}
