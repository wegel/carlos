//! Apply native item identities through the transcript store and collect plain reports.

use std::borrow::Cow;

use serde_json::Value;

use super::item_changes::ItemChanges;
use super::models::{MessageKind, Role};
use super::native_activity::format_native_activity;
use super::state::SessionState;
use super::summary_request::has_command;

/// Record a bounded native collaboration report and its visible tool row.
pub(super) fn upsert_native_activity(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    item: &Value,
) -> bool {
    super::codex::panel_events::report(session, item);
    let Some(text) = format_native_activity(item) else {
        return false;
    };
    if let Some(id) = item.get("id").and_then(Value::as_str) {
        let key = session.backend_entry_key(id);
        changes.track(session.upsert_mapped_message(
            &key,
            Role::ToolCall,
            text,
            MessageKind::Plain,
            None,
        ));
    }
    true
}

/// Observe a command only after confirming that the native item carries one.
pub(super) fn receive_summary(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    item: &Value,
    live: bool,
) {
    let Some(id) = item["id"].as_str() else {
        return;
    };
    if !has_command(item) {
        return;
    }
    let key = session.backend_entry_key(id);
    let scope = key.turn.as_deref().map(|turn| (turn, key.part));
    changes.push(session.observe_summary_item(item, live, &key, scope));
}

/// Deduplicate a submitted prompt and report whether the client cursor must reset.
pub(super) fn record_prompt(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    text: &str,
    index: Option<usize>,
) {
    let entry = index.and_then(|index| session.messages().get(index).map(|row| row.id));
    changes.reset_navigation |= session.record_prompt_history(text, entry);
}

/// Attach native turn positions to the exact visible prompt, including steering items.
pub(super) fn record_native_turn(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    item: &str,
    turn: &str,
) {
    let previous = session.replace_codex_item_turn(Some(turn.into()));
    let index = session.item_message_index(&session.backend_entry_key(item));
    session.replace_codex_item_turn(previous);
    let entry = index.map(|index| session.messages()[index].id);
    if let Some(entry) = entry {
        session.record_prompt_turn(entry, turn);
    }
    let position = session.record_native_user(item, turn, entry);
    if let (Some(index), Some(position)) = (index, position) {
        let kind = if position == 0 {
            MessageKind::Plain
        } else {
            MessageKind::UserSteer
        };
        if session.messages()[index].kind != kind {
            session.transcript_set_kind(index, kind);
            changes.push(session.dirty_entry(index));
        }
    }
}

/// Normalize a live reasoning row after applying its native delta.
pub(super) fn receive_reasoning(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    item: &str,
    delta: &str,
) {
    let key = session.backend_entry_key(item);
    if session.is_item_completed(&key) {
        return;
    }
    changes.push(session.upsert_reasoning_delta(&key, delta));
    if let Some(index) = session.item_message_index(&key) {
        changes.push(session.normalize_reasoning_at(index));
    }
}

impl SessionState {
    /// Select a display part only for the duration of an internal native item edit.
    pub(in crate::session) fn with_native_item_part<R>(
        &mut self,
        item: &str,
        part: usize,
        apply: impl FnOnce(&mut Self, &str) -> R,
    ) -> R {
        let previous = self.replace_codex_item_part(part);
        let result = apply(self, item);
        self.replace_codex_item_part(previous);
        result
    }
}

/// A final native command may carry status without repeating the streamed body.
pub(crate) fn command_output_missing(item: &Value) -> bool {
    item.get("type").and_then(Value::as_str) == Some("commandExecution")
        && [
            "aggregatedOutput",
            "aggregated_output",
            "output",
            "stdout",
            "stderr",
            "formattedOutput",
            "formatted_output",
        ]
        .iter()
        .all(|key| {
            item.get(key)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        })
}

/// Retain the already received command body when the final native item omits it.
pub(super) fn with_streamed_output<'a>(session: &SessionState, item: &'a Value) -> Cow<'a, Value> {
    if command_output_missing(item) {
        let message = item
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| session.item_message_index(&session.backend_entry_key(id)))
            .and_then(|index| session.messages().get(index));
        if let Some(message) =
            message.filter(|row| row.role == Role::ToolOutput && !row.text.is_empty())
        {
            let mut item = item.clone();
            item["aggregatedOutput"] = Value::String(message.text.clone());
            return Cow::Owned(item);
        }
    }
    Cow::Borrowed(item)
}

/// Attach native tool facts and retain the original incremental redraw report.
pub(super) fn describe_output(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    index: usize,
    item: &Value,
) {
    let key = item["id"].as_str().map(|id| session.backend_entry_key(id));
    changes.push(session.describe_tool_output(index, item, key.as_ref()));
}
