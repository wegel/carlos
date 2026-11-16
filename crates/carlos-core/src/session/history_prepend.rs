//! Restore facts for older entries without replacing newer rows or publishing a transcript reset.

use serde_json::Value;

use super::item_history_draft::{build, HistoryRecord};
use super::state::SessionState;

impl SessionState {
    pub(super) fn prepend_native_thread(&mut self, thread: &Value) -> usize {
        let previous = self.replace_codex_item_turn(None);
        let draft = build(self, thread);
        let mut journal = std::mem::take(&mut self.messages.journal);
        let ids = self.messages.prepend_history(draft.entries);
        for (turn, status) in draft.turns {
            self.record_historical_turn_status(&turn, status);
        }
        let mut prompts = Vec::new();
        for record in draft.records {
            for (index, text) in &record.prompts {
                prompts.push((ids[*index], text.clone()));
            }
            self.restore_older_record(record);
        }
        self.input_history.prepend_history(prompts);
        if let Some(boundary) = &mut self.turn_boundary {
            boundary.offset += ids.len();
        }
        journal.prepend_stored(ids.len());
        self.messages.journal = journal;
        self.replace_codex_item_turn(previous);
        ids.len()
    }

    fn restore_older_record(&mut self, record: HistoryRecord) {
        self.replace_codex_item_turn(record.turn.clone());
        for (index, row, describe) in record.updates {
            self.transcript_update(index, row.role, Some(row.text), row.kind, row.file_path);
            if describe {
                self.describe_tool_output(
                    index,
                    &record.item,
                    record.item["id"]
                        .as_str()
                        .map(|id| self.backend_entry_key(id))
                        .as_ref(),
                );
            } else if row.role == super::models::Role::ToolCall {
                self.describe_tool_facts(index, &record.item);
            }
        }
        if record.observe {
            super::item_receive::receive_summary(self, &record.item, false);
        }
        let Some(id) = record.item["id"].as_str() else {
            return;
        };
        if record.item["type"] == "userMessage" {
            if let Some(turn) = &record.turn {
                let entry = record.native_index.map(|index| self.messages[index].id);
                if let Some(entry) = entry {
                    self.record_prompt_turn(entry, turn);
                }
                self.record_native_user(id, turn, entry);
            }
        }
        if record.terminal {
            self.mark_item_completed(&self.backend_entry_key(id));
        }
    }
}
