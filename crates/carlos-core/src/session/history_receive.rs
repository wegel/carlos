//! Adopt checked native history and rebuild session-owned prompt and tool facts.

use super::history_identity::HistoryMode;
use super::item_changes::ItemChanges;
use super::item_history_draft::{HistoryDraft, HistoryRecord};
use super::state::SessionState;
use crate::session::models::MessageKind;

impl SessionState {
    /// Adopt all rows before restoring prompt, tool and turn facts in native order.
    pub(crate) fn replace_native_history(
        &mut self,
        draft: HistoryDraft,
        mode: HistoryMode,
    ) -> ItemChanges {
        self.replace_history(draft.entries, mode);
        let mut changes = ItemChanges {
            history_replaced: true,
            ..Default::default()
        };
        self.reset_prompt_history();
        self.clear_codex_reports();
        for (turn, status) in draft.turns {
            self.record_historical_turn_status(&turn, status);
        }
        for record in draft.records {
            self.restore_history_record(&mut changes, record);
        }
        self.restore_turn_boundary();
        changes
    }

    fn restore_history_record(&mut self, changes: &mut ItemChanges, record: HistoryRecord) {
        self.replace_codex_item_turn(record.turn.clone());
        super::codex::panel_events::report(self, &record.item);
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
            } else if row.role == crate::session::models::Role::ToolCall {
                self.describe_tool_facts(index, &record.item);
            }
        }
        for (index, text) in record.prompts {
            super::item_receive::record_prompt(self, changes, &text, Some(index));
        }
        if record.observe {
            super::item_receive::receive_summary(self, &record.item, false);
        }
        let Some(id) = record.item["id"].as_str() else {
            return;
        };
        if record.item["type"] == "userMessage" {
            if let Some(turn) = &record.turn {
                let entry = record.native_index.map(|index| self.messages()[index].id);
                if let Some(entry) = entry {
                    self.record_prompt_turn(entry, turn);
                }
                let position = self.record_native_user(id, turn, entry);
                if let (Some(index), Some(position)) = (record.native_index, position) {
                    self.transcript_set_kind(
                        index,
                        if position == 0 {
                            MessageKind::Plain
                        } else {
                            MessageKind::UserSteer
                        },
                    );
                }
            }
        }
        if record.terminal {
            self.mark_item_completed(&self.backend_entry_key(id));
        }
    }
}
