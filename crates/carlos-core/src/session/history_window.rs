//! Dispatch checked native windows without giving clients a backend cursor or a second reader.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use carlos_proto::ids::EntryId as WireEntryId;

use super::entry_id::EntryId;
use super::state::SessionState;
use crate::backend::BackendClient;
use crate::{claude_backend, protocol_history};

/// Retain backend-specific continuation data only inside the owner and its worker.
pub(crate) enum HistoryCursor {
    Codex(protocol_history::HistoryCursor),
    Claude(claude_backend::history_window::HistoryCursor),
}

/// Return a complete native window without introducing a public wire reply family.
pub(crate) enum HistoryPage {
    Codex(protocol_history::HistoryPage),
    Claude(claude_backend::history_window::HistoryPage),
}

/// Use the captured backend handle while the owner remains free to process controls.
pub(crate) fn older_window(
    client: &dyn BackendClient,
    cursor: &HistoryCursor,
    timeout: Duration,
) -> Result<HistoryPage> {
    match cursor {
        HistoryCursor::Codex(cursor) => {
            protocol_history::older_window(client, cursor, timeout).map(HistoryPage::Codex)
        }
        HistoryCursor::Claude(cursor) => {
            claude_backend::history_window::older_window(client, cursor, timeout)
                .map(HistoryPage::Claude)
        }
    }
}

impl SessionState {
    /// Validate a public before-id before deciding whether native I/O is necessary.
    pub(crate) fn history_window_cursor(
        &self,
        before: &WireEntryId,
        limit: usize,
    ) -> Result<Option<HistoryCursor>> {
        if let Some(cursor) = self.older_native_cursor(before, limit)? {
            return Ok(Some(HistoryCursor::Codex(cursor)));
        }
        let end = self
            .messages
            .position(EntryId::from_wire(before))
            .context("retired or foreign history cursor")?;
        if end >= limit.clamp(1, 1000) {
            return Ok(None);
        }
        Ok(self
            .older_claude_history_cursor()?
            .map(HistoryCursor::Claude))
    }

    /// Keep a returned window within the original backend, root and native scope.
    pub(crate) fn accept_history_window(
        &mut self,
        cursor: &HistoryCursor,
        page: HistoryPage,
    ) -> Result<usize> {
        match (cursor, page) {
            (HistoryCursor::Codex(cursor), HistoryPage::Codex(page)) => {
                self.accept_older_native_history(cursor, page)
            }
            (HistoryCursor::Claude(cursor), HistoryPage::Claude(page)) => {
                self.accept_older_claude_history(cursor, page)
            }
            _ => bail!("native history backend changed"),
        }
    }
}
