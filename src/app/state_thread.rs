//! Replace or fork transcript state while preserving the background summary service.

use super::input_history_state::InputHistoryView;
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
        let update = self.session.replace_history(draft.entries, mode);
        self.session.reset_prompt_history();
        self.view.input_history = InputHistoryView::default();
        self.session.clear_codex_reports();
        for (turn, status) in draft.turns {
            self.record_historical_turn_status(&turn, status);
        }
        for record in draft.records {
            self.restore_history_record(record);
        }
        self.view.render_cache = RenderCacheState::new();
        self.view.viewport = ViewportState::new();
        self.session.restore_turn_boundary();
        self.apply_session_update(update);
    }

    fn restore_history_record(
        &mut self,
        record: super::item_history::history_draft::HistoryRecord,
    ) {
        self.codex.item_turn = record.turn.clone();
        super::codex_panel_events::report(self, &record.item);
        for (index, row, describe) in record.updates {
            self.session.transcript_update(
                index,
                row.role,
                Some(row.text),
                row.kind,
                row.file_path,
            );
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
                let entry = record
                    .native_index
                    .map(|index| self.session.messages()[index].id);
                if let Some(entry) = entry {
                    self.session.record_prompt_turn(entry, turn);
                }
                let position = self.session.record_native_user(id, turn, entry);
                if let (Some(index), Some(position)) = (record.native_index, position) {
                    self.session.transcript_set_kind(
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
        self.reset_thread(thread_id, false);
    }

    /// Clear native transcript rows while process callbacks remain live.
    pub(super) fn reset_preserving_approvals(&mut self, thread_id: impl Into<String>) {
        self.reset_thread(thread_id, true);
    }

    fn reset_thread(&mut self, thread_id: impl Into<String>, preserve: bool) {
        self.set_thread_id(thread_id);
        let update = self.session.reset_for_forked_thread();
        if update.transcript_reset {
            self.view.render_cache = RenderCacheState::new();
        }
        self.session.reset_prompt_history();
        self.view.input_history = InputHistoryView::default();
        self.view.viewport = ViewportState::new();
        if preserve {
            return;
        }
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
        self.session.clear_summaries();
        self.session.transcript_adopt_unkeyed(
            replacement
                .session
                .messages()
                .entries()
                .iter()
                .cloned()
                .map(Into::into)
                .collect(),
            crate::session::history_identity::HistoryMode::SameThread,
        );
        self.view.render_cache = RenderCacheState::new();
        self.view.viewport = ViewportState::new();
        self.mark_transcript_dirty();
    }
}
