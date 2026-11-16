//! Prepend checked archive facts through the ordinary reducers without replacing current work.

use anyhow::{ensure, Context, Result};
use carlos_proto::session::Backend;
use serde_json::{json, Value};

use crate::backend::BackendKind;
use crate::claude_backend::history_window::{HistoryCursor, HistoryPage};
use crate::session::state::SessionState;

impl SessionState {
    pub(super) fn finish_claude_history_window(&mut self) {
        if let Some(newer) = self.claude.history.newer_records.take() {
            let older = self.claude.history.records.len().saturating_sub(newer);
            self.claude.history.records.rotate_right(older);
        }
    }

    pub(super) fn install_claude_history_window(&mut self, result: &Value) -> Result<()> {
        let cursor = initial_cursor(self.thread_id(), result)?;
        let connection = self.command_connection(BackendKind::Claude);
        self.claude.history.older = cursor.map(|cursor| (cursor, connection));
        Ok(())
    }

    /// Retire an old archive before adopting a different native conversation.
    pub(in crate::session) fn clear_claude_history_window(&mut self) {
        self.finish_claude_history_window();
        self.claude.history.older = None;
    }

    /// Capture only the archive prefix still owned by this root and helper generation.
    pub(crate) fn older_claude_history_cursor(&self) -> Result<Option<HistoryCursor>> {
        let Some((cursor, connection)) = &self.claude.history.older else {
            return Ok(None);
        };
        ensure!(
            cursor.thread == *self.thread_id()
                && *connection == self.command_connection(BackendKind::Claude),
            "Claude history belongs to a retired root or helper"
        );
        Ok(Some(cursor.clone()))
    }

    /// Preserve an available native prefix even when it has not been read yet.
    pub(crate) fn has_older_claude_history(&self) -> bool {
        self.claude.history.older.is_some()
    }

    /// Adopt a complete checked prefix without reverting live rows or publishing new-message events.
    pub(crate) fn accept_older_claude_history(
        &mut self,
        cursor: &HistoryCursor,
        page: HistoryPage,
    ) -> Result<usize> {
        ensure!(
            self.older_claude_history_cursor()?.as_ref() == Some(cursor),
            "Claude history read retired"
        );
        if let Some(next) = &page.older {
            ensure!(
                next.thread == cursor.thread
                    && next.snapshot == cursor.snapshot
                    && next.end < cursor.end,
                "Claude history repeated or changed its native scope"
            );
        }
        let count = self.prepend_claude_history(page.lines)?;
        let connection = self.command_connection(BackendKind::Claude);
        self.claude.history.older = page.older.map(|cursor| (cursor, connection));
        Ok(count)
    }

    fn prepend_claude_history(&mut self, lines: Vec<String>) -> Result<usize> {
        // This is a pure scratch model: no backend, threads, timers or transport are created.
        let mut prefix = SessionState::new(self.thread_id().clone(), Default::default(), false);
        prefix.enable_claude();
        prefix.set_client_identity(Backend::Claude, String::new());
        prefix.set_summary_cwd(self.summary_cwd().to_owned());
        let facts: Vec<Value> = lines
            .iter()
            .map(|line| serde_json::from_str(line))
            .collect::<Result<_, _>>()?;
        ensure!(
            facts.iter().all(|fact| fact["method"].is_string()),
            "Claude history fact has no method"
        );
        std::mem::swap(&mut prefix.messages.ids, &mut self.messages.ids);
        for fact in facts {
            prefix.receive_claude_message(
                fact["method"].as_str().expect("checked history method"),
                &fact["params"],
            );
        }
        std::mem::swap(&mut prefix.messages.ids, &mut self.messages.ids);
        let count = prefix.messages.len();
        let mut prompts = Vec::new();
        for index in 0..prefix.input_history.history_len() {
            if let Some(id) = prefix.input_history.entry_id(index) {
                prompts.push((id, prefix.input_history.text(index).to_owned()));
                if let Some(uuid) = prefix.input_history.turn_id(id) {
                    self.record_prompt_turn(id, uuid);
                }
            }
        }
        let mut journal = std::mem::take(&mut self.messages.journal);
        self.messages.prepend_transcript(prefix.messages);
        journal.prepend_stored(count);
        self.messages.journal = journal;
        self.input_history.prepend_history(prompts);
        for record in prefix.claude.history.records.into_iter().rev() {
            self.claude.history.records.push_front(record);
        }
        prefix
            .claude
            .history
            .unknown
            .append(&mut self.claude.history.unknown);
        self.claude.history.unknown = prefix.claude.history.unknown;
        if let Some(boundary) = &mut self.turn_boundary {
            boundary.offset += count;
        }
        Ok(count)
    }
}

fn initial_cursor(thread: &str, result: &Value) -> Result<Option<HistoryCursor>> {
    if result.get("windowEnd").is_none() {
        return Ok(None);
    }
    let end = result["windowEnd"]
        .as_u64()
        .context("Claude history has no window end")?;
    let start = result["windowStart"]
        .as_u64()
        .context("Claude history has no window start")?;
    ensure!(
        start <= end && end <= 100000,
        "Claude history has invalid window bounds"
    );
    ensure!(
        result["nextOffset"].is_null(),
        "Claude history has an invalid next offset"
    );
    ensure!(
        result["olderOffset"]
            == if start == 0 {
                Value::Null
            } else {
                json!(start)
            },
        "Claude history has an invalid older boundary"
    );
    let snapshot = result["snapshotId"]
        .as_str()
        .context("Claude history has no snapshot id")?;
    ensure!(
        !snapshot.is_empty() && snapshot.len() <= 4096,
        "Claude history has an invalid snapshot id"
    );
    Ok((start > 0).then(|| HistoryCursor {
        thread: thread.into(),
        snapshot: snapshot.into(),
        end: start,
    }))
}
