//! AppState impl: transcript/message manipulation, rendered lines, and selection.

use carlos_core::session::models::Role;

use super::selection::{RenderedLineSource, Selection};
use super::state::AppState;
use super::view_models::RenderedLine;

impl AppState {
    /// Consume core events and apply caller-local history navigation effects.
    pub(super) fn apply_item_changes(
        &mut self,
        changes: carlos_core::session::item_changes::ItemChanges,
    ) {
        self.consume_core_events();
        if changes.reset_navigation {
            self.reset_input_history_navigation();
        }
    }

    // --- Transcript mutation ---

    #[cfg(test)]
    pub(super) fn mark_transcript_dirty(&mut self) {
        self.mark_transcript_dirty_from(0);
    }

    #[cfg(test)]
    pub(super) fn mark_transcript_dirty_from(&mut self, idx: usize) {
        self.view
            .render_cache
            .mark_transcript_dirty_from(self.session.messages().len(), idx);
    }

    pub(super) fn ensure_rendered_lines(
        &mut self,
        width: usize,
        hidden_user_message_idx: Option<usize>,
    ) {
        self.consume_core_events();
        if !self.view.render_cache.matches_view(
            width,
            self.view.tool_output.compact(),
            self.view.layout.active,
        ) {
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
        let index = self.session.append_message(role, text);
        self.consume_core_events();
        index
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
