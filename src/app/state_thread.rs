//! Replace or fork transcript state while preserving the background summary service.

use super::input_history_state::InputHistoryState;
use super::render_cache_state::RenderCacheState;
use super::state::AppState;
use super::viewport_state::ViewportState;

impl AppState {
    /// Adopt rows and rebuild their prompt, completion, label and view references.
    pub(super) fn replace_history_draft(
        &mut self,
        draft: super::item_history::history_draft::HistoryDraft,
        mode: crate::session::history_identity::HistoryMode,
    ) {
        self.summaries.clear();
        self.messages.replace_history(draft.entries, mode);
        self.completed_item_ids.clear();
        self.turn_status_by_id.clear();
        self.command_render_overrides.clear();
        let native = self.input_history.native_enabled();
        self.input_history = InputHistoryState::new();
        if native {
            self.input_history.enable_native();
        }
        self.codex.panel.reports.clear();
        for (turn, status) in draft.turns {
            self.record_historical_turn_status(&turn, status);
        }
        for record in draft.records {
            self.restore_history_record(record);
        }
        self.render_cache = RenderCacheState::new();
        self.viewport = ViewportState::new();
        self.turn_start_message_idx = self.active_turn_id.as_ref().map(|_| self.messages.len());
        self.mark_transcript_dirty();
    }

    fn restore_history_record(
        &mut self,
        record: super::item_history::history_draft::HistoryRecord,
    ) {
        self.codex.item_turn = record.turn.clone();
        super::codex_panel_events::report(self, &record.item);
        for (index, row, describe) in record.updates {
            self.messages
                .update(index, row.role, Some(row.text), row.kind, row.file_path);
            if describe {
                self.describe_tool_output(index, &record.item);
            }
        }
        for (index, text) in record.prompts {
            self.record_input_history(&text, Some(index));
        }
        if record.observe {
            self.observe_summary_item(&record.item, false);
        }
        let Some(id) = record.item["id"].as_str() else {
            return;
        };
        if record.item["type"] == "userMessage" {
            if let Some(turn) = &record.turn {
                let entry = record.native_index.map(|index| self.messages[index].id);
                if let Some(entry) = entry {
                    self.input_history.record_turn_id(entry, turn);
                }
                let position = self.input_history.record_native(id, turn, entry);
                if let (Some(index), Some(position)) = (record.native_index, position) {
                    self.messages.set_kind(
                        index,
                        if position == 0 {
                            super::MessageKind::Plain
                        } else {
                            super::MessageKind::UserSteer
                        },
                    );
                }
            }
        }
        if record.terminal {
            self.mark_item_completed(id);
        }
    }

    pub(super) fn reset_for_forked_thread(&mut self, thread_id: impl Into<String>) {
        self.set_thread_id(thread_id);
        self.entry_conversation = self.thread_id.clone();
        self.active_turn_id = None;
        self.messages.reset();
        self.render_cache = RenderCacheState::new();
        self.completed_item_ids.clear();
        self.turn_status_by_id.clear();
        self.command_render_overrides.clear();
        self.summaries.clear();
        let native = self.input_history.native_enabled();
        self.input_history = InputHistoryState::new();
        if native {
            self.input_history.enable_native();
        }
        self.turn_start_message_idx = None;
        self.viewport = ViewportState::new();
        self.context_usage = None;
        if self.codex.enabled {
            super::codex_callbacks::reset(self);
        } else {
            self.clear_pending_approval();
        }
    }
}

#[cfg(test)]
impl AppState {
    pub(super) fn replace_transcript_for_test(&mut self, replacement: AppState) {
        self.summaries.clear();
        self.messages.adopt_unkeyed(
            replacement
                .messages
                .entries()
                .iter()
                .cloned()
                .map(Into::into)
                .collect(),
            crate::session::history_identity::HistoryMode::SameThread,
        );
        self.render_cache = RenderCacheState::new();
        self.viewport = ViewportState::new();
        self.mark_transcript_dirty();
    }
}
