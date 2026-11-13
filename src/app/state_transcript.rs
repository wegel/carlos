//! AppState impl: transcript/message manipulation, rendered lines, and selection.

use carlos_core::session::models::Role;

use super::selection::{RenderedLineSource, Selection};
use super::state::AppState;
use super::view_models::RenderedLine;

impl AppState {
    /// Resolve one session report and retain the existing incremental cache path.
    pub(super) fn apply_session_update(
        &mut self,
        update: carlos_core::session::transcript_updates::SessionUpdate,
    ) {
        let _ = update;
        self.consume_core_events();
    }

    /// Apply one synchronous item report before another terminal event runs.
    pub(super) fn apply_item_changes(
        &mut self,
        changes: carlos_core::session::item_changes::ItemChanges,
    ) {
        for update in changes.transcript {
            self.apply_session_update(update);
        }
        if changes.reset_navigation {
            self.reset_input_history_navigation();
        }
    }

    // --- Transcript mutation ---

    pub(super) fn mark_transcript_dirty(&mut self) {
        self.mark_transcript_dirty_from(0);
    }

    pub(super) fn mark_transcript_dirty_from(&mut self, idx: usize) {
        self.view
            .render_cache
            .mark_transcript_dirty_from(self.session.messages().len(), idx);
    }

    pub(super) fn sync_auto_follow_bottom(&mut self, max_scroll: usize) {
        self.view.viewport.sync_auto_follow_bottom(max_scroll);
    }

    pub(super) fn ensure_rendered_lines(
        &mut self,
        width: usize,
        hidden_user_message_idx: Option<usize>,
    ) {
        self.consume_core_events();
        if self.view.render_cache.rendered_width != width
            || self.view.render_cache.rendered_compact != self.view.tool_output.compact()
            || self.view.render_cache.rendered_theme != self.view.layout.active
        {
            self.view.viewport.selection = None;
            self.view.viewport.mouse_drag_mode = super::selection::MouseDragMode::Undecided;
        }
        let anchor = (!self.view.viewport.auto_follow_bottom)
            .then(|| {
                self.view
                    .render_cache
                    .message_anchor(self.view.viewport.scroll_top)
            })
            .flatten()
            .and_then(|(index, row)| {
                self.session
                    .messages()
                    .get(index)
                    .map(|message| (message.id, row))
            });
        self.view.render_cache.ensure_rendered_lines(
            self.session.messages(),
            width,
            hidden_user_message_idx,
            self.view.tool_output.compact(),
            self.view.layout.active,
        );
        if let Some(line) = anchor.and_then(|(id, row)| {
            self.session
                .messages()
                .position(id)
                .and_then(|index| self.view.render_cache.line_for_anchor((index, row)))
        }) {
            self.view.viewport.scroll_top = line;
        }
    }

    pub(super) fn append_message(&mut self, role: Role, text: impl Into<String>) -> usize {
        let (index, update) = self.session.append_message(role, text);
        self.apply_session_update(update);
        index
    }

    pub(super) fn append_diff_message(
        &mut self,
        role: Role,
        file_path: Option<String>,
        diff: impl Into<String>,
    ) -> usize {
        let (index, update) = self.session.append_diff_message(role, file_path, diff);
        self.apply_session_update(update);
        index
    }

    #[cfg(test)]
    pub(super) fn upsert_turn_diff(&mut self, turn_id: &str, diff: &str) {
        let update = self.session.upsert_turn_diff(turn_id, diff);
        self.apply_session_update(update);
    }

    #[cfg(test)]
    pub(super) fn set_command_override(&mut self, call_id: &str, summary: String) {
        let key = self.item_key(call_id);
        let update = self.session.set_command_override(&key, summary);
        self.apply_session_update(update);
    }

    // --- Rendered lines & selection ---

    pub(super) fn rendered_line_count(&self) -> usize {
        self.view.render_cache.rendered_line_count()
    }

    pub(super) fn rendered_line_at(&self, idx: usize) -> Option<&RenderedLine> {
        self.view.render_cache.rendered_line_at(idx)
    }

    pub(super) fn ensure_rendered_range_materialized(&mut self, start_idx: usize, end_idx: usize) {
        self.consume_core_events();
        self.view.render_cache.ensure_rendered_range_materialized(
            self.session.messages(),
            start_idx,
            end_idx,
        );
    }

    #[cfg(test)]
    pub(super) fn snapshot_rendered_lines(&mut self) -> Vec<RenderedLine> {
        self.consume_core_events();
        self.view
            .render_cache
            .snapshot_rendered_lines(self.session.messages())
    }

    pub(super) fn selected_text(&mut self, selection: Selection) -> String {
        let start_idx = selection.anchor_line_idx.min(selection.focus_line_idx);
        let end_idx = selection.anchor_line_idx.max(selection.focus_line_idx);
        if self.view.render_cache.rendered_total_lines > 0 {
            self.ensure_rendered_range_materialized(
                start_idx.min(
                    self.view
                        .render_cache
                        .rendered_total_lines
                        .saturating_sub(1),
                ),
                end_idx.min(
                    self.view
                        .render_cache
                        .rendered_total_lines
                        .saturating_sub(1),
                ),
            );
        }
        super::selection::selected_text(selection, self)
    }
}

impl RenderedLineSource for AppState {
    fn len(&self) -> usize {
        self.rendered_line_count()
    }

    fn get(&self, idx: usize) -> Option<&RenderedLine> {
        self.rendered_line_at(idx)
    }
}

#[cfg(test)]
#[path = "state_transcript_checks.rs"]
mod checks;
