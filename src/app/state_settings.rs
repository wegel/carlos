//! AppState impl: runtime/model settings delegation.

use super::state::AppState;
use super::RuntimeDefaults;

impl AppState {
    // --- Runtime settings ---

    pub(super) fn set_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.session.set_runtime_settings(model, effort, summary);
    }

    pub(super) fn merge_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.session.merge_runtime_settings(model, effort, summary);
    }

    pub(super) fn set_available_models(&mut self, models: Vec<crate::protocol_params::ModelInfo>) {
        self.session.set_available_models(models);
    }

    pub(super) fn set_runtime_capabilities(
        &mut self,
        supports_effort: bool,
        supports_summary: bool,
    ) {
        self.session
            .set_runtime_capabilities(supports_effort, supports_summary);
    }

    pub(super) fn queue_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.session.queue_runtime_settings(model, effort, summary);
    }

    #[cfg(test)]
    pub(super) fn take_pending_runtime_settings(
        &mut self,
    ) -> (Option<String>, Option<String>, Option<String>) {
        self.session.pending_runtime_settings()
    }

    pub(super) fn next_turn_runtime_settings(
        &self,
    ) -> (Option<String>, Option<String>, Option<String>) {
        self.session.next_turn_runtime_settings()
    }

    pub(super) fn mark_runtime_settings_applied(&mut self) {
        self.session.mark_runtime_settings_applied();
    }

    pub(super) fn runtime_settings_label(&self) -> String {
        self.session.runtime_settings_label()
    }

    pub(super) fn open_model_settings(&mut self) {
        self.view.runtime_settings.model_settings_tool_output = self.view.tool_output.mode;
        self.view.layout.draft = self.view.layout.themes;
        self.view.summary_enabled_draft = self.session.summaries_enabled();
        self.view
            .runtime_settings
            .open_model_settings(self.session.runtime_settings());
        if self.codex.enabled {
            super::codex_settings_draft::open_editor(self);
            super::codex_settings_draft::restore_editor(self);
        }
    }

    pub(super) fn close_model_settings(&mut self) {
        self.view.runtime_settings.close_model_settings();
    }

    pub(super) fn toggle_model_settings(&mut self) {
        if self.view.runtime_settings.show_model_settings {
            self.close_model_settings();
        } else {
            self.open_model_settings();
        }
    }

    pub(super) fn model_settings_move_field(&mut self, forward: bool) {
        self.view
            .runtime_settings
            .model_settings_move_field(self.session.runtime_settings(), forward);
    }

    pub(super) fn model_settings_cycle_effort(&mut self, step: isize) {
        self.view.runtime_settings.model_settings_cycle_effort(step);
    }

    pub(super) fn model_settings_cycle_model(&mut self, step: isize) {
        self.view
            .runtime_settings
            .model_settings_cycle_model(self.session.runtime_settings(), step);
    }

    pub(super) fn model_settings_cycle_summary(&mut self, step: isize) {
        self.view
            .runtime_settings
            .model_settings_cycle_summary(step);
    }

    pub(super) fn model_settings_has_model_choices(&self) -> bool {
        self.view
            .runtime_settings
            .model_settings_has_model_choices(self.session.runtime_settings())
    }

    pub(super) fn model_settings_insert_char(&mut self, ch: char) {
        self.view.runtime_settings.model_settings_insert_char(ch);
    }

    pub(super) fn model_settings_backspace(&mut self) {
        self.view.runtime_settings.model_settings_backspace();
    }

    pub(super) fn apply_model_settings(&mut self) -> RuntimeDefaults {
        self.apply_summary_setting();
        self.view.tool_output.mode = self.view.runtime_settings.model_settings_tool_output;
        self.view.layout.themes = self.view.layout.draft;
        let defaults = self
            .view
            .runtime_settings
            .saved_choices(self.session.runtime_settings());
        self.queue_runtime_settings(
            defaults.model.clone(),
            defaults.effort.clone(),
            defaults.summary.clone(),
        );
        self.view.runtime_settings.close_model_settings();
        let label = if self.session.runtime_settings().supports_summary() {
            "model/effort/summary"
        } else {
            "model/effort"
        };
        if self.session.active_turn_id().is_some() {
            self.set_hint(format!("{label} pending next turn; saved as default"));
        } else {
            self.set_hint(format!("{label} set for next turn; saved as default"));
        }
        defaults
    }

    pub(super) fn model_settings_model_value(&self) -> &str {
        self.view
            .runtime_settings
            .model_settings_model_value(self.session.runtime_settings())
    }

    pub(super) fn model_settings_effort_value(&self) -> &str {
        self.view.runtime_settings.model_settings_effort_value()
    }

    pub(super) fn model_settings_summary_value(&self) -> &str {
        self.view
            .runtime_settings
            .model_settings_summary_value(self.session.runtime_settings())
    }

    #[cfg(test)]
    pub(super) fn apply_default_reasoning_summary(&mut self, summary: Option<String>) {
        self.session.apply_default_reasoning_summary(summary);
    }
}
