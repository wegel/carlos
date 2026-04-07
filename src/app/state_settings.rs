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
        self.runtime.set_runtime_settings(model, effort, summary);
    }

    pub(super) fn merge_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.runtime.merge_runtime_settings(model, effort, summary);
    }

    pub(super) fn set_available_models(&mut self, models: Vec<crate::protocol_params::ModelInfo>) {
        self.runtime.set_available_models(models);
    }

    pub(super) fn set_runtime_capabilities(
        &mut self,
        supports_effort: bool,
        supports_summary: bool,
    ) {
        self.runtime
            .set_capabilities(supports_effort, supports_summary);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn queue_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.runtime.queue_runtime_settings(model, effort, summary);
    }

    #[cfg(test)]
    pub(super) fn take_pending_runtime_settings(
        &mut self,
    ) -> (Option<String>, Option<String>, Option<String>) {
        self.runtime.take_pending_runtime_settings()
    }

    pub(super) fn next_turn_runtime_settings(
        &self,
    ) -> (Option<String>, Option<String>, Option<String>) {
        self.runtime.next_turn_runtime_settings()
    }

    pub(super) fn mark_runtime_settings_applied(&mut self) {
        self.runtime.mark_runtime_settings_applied();
    }

    pub(super) fn runtime_settings_label(&self) -> String {
        self.runtime.runtime_settings_label()
    }

    pub(super) fn has_runtime_settings(&self) -> bool {
        self.runtime.has_runtime_settings()
    }

    pub(super) fn runtime_settings_pending(&self) -> bool {
        self.runtime.runtime_settings_pending()
    }

    pub(super) fn open_model_settings(&mut self) {
        self.runtime.open_model_settings();
    }

    pub(super) fn close_model_settings(&mut self) {
        self.runtime.close_model_settings();
    }

    pub(super) fn toggle_model_settings(&mut self) {
        if self.runtime.show_model_settings {
            self.close_model_settings();
        } else {
            self.open_model_settings();
        }
    }

    pub(super) fn model_settings_move_field(&mut self, forward: bool) {
        self.runtime.model_settings_move_field(forward);
    }

    pub(super) fn model_settings_cycle_effort(&mut self, step: isize) {
        self.runtime.model_settings_cycle_effort(step);
    }

    pub(super) fn model_settings_cycle_model(&mut self, step: isize) {
        self.runtime.model_settings_cycle_model(step);
    }

    pub(super) fn model_settings_cycle_summary(&mut self, step: isize) {
        self.runtime.model_settings_cycle_summary(step);
    }

    pub(super) fn model_settings_has_model_choices(&self) -> bool {
        self.runtime.model_settings_has_model_choices()
    }

    pub(super) fn model_settings_insert_char(&mut self, ch: char) {
        self.runtime.model_settings_insert_char(ch);
    }

    pub(super) fn model_settings_backspace(&mut self) {
        self.runtime.model_settings_backspace();
    }

    pub(super) fn apply_model_settings(&mut self) -> RuntimeDefaults {
        let defaults = self.runtime.apply_model_settings();
        let label = if self.runtime.supports_summary {
            "model/effort/summary"
        } else {
            "model/effort"
        };
        if self.active_turn_id.is_some() {
            self.set_status(format!("{label} pending next turn; saved as default"));
        } else {
            self.set_status(format!("{label} set for next turn; saved as default"));
        }
        defaults
    }

    pub(super) fn model_settings_model_value(&self) -> &str {
        self.runtime.model_settings_model_value()
    }

    pub(super) fn model_settings_effort_value(&self) -> &str {
        self.runtime.model_settings_effort_value()
    }

    pub(super) fn model_settings_summary_value(&self) -> &str {
        self.runtime.model_settings_summary_value()
    }

    pub(super) fn runtime_supports_summary(&self) -> bool {
        self.runtime.supports_summary
    }

    #[cfg(test)]
    pub(super) fn apply_default_reasoning_summary(&mut self, summary: Option<String>) {
        self.runtime.apply_default_reasoning_summary(summary);
    }
}
