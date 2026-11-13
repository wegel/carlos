//! Keep Claude inheritance rules with the runtime choices they change.

use crate::session::runtime_settings::{RuntimeDefaults, RuntimeSetting};
use crate::session::state::SessionState;

impl SessionState {
    /// Resolve native inheritance once launch settings have been read.
    pub fn start_claude_settings(&mut self) {
        self.claude.phase = super::state::ClaudePhase::Starting;
        self.claude.inherits_model = self.runtime.current_model.is_none();
        self.claude.inherits_effort = self
            .runtime
            .current_effort
            .as_deref()
            .is_none_or(|effort| effort == "auto");
    }

    /// Forget inherited acknowledged choices on switch unless a pending override exists.
    pub(crate) fn switch_claude_runtime_choices(&mut self) {
        if self.claude.inherits_model && self.runtime.pending_model.is_none() {
            self.record_runtime_choice(RuntimeSetting::Model, None);
        }
        if self.claude.inherits_effort && self.runtime.pending_effort.is_none() {
            self.record_runtime_choice(RuntimeSetting::Effort, None);
        }
    }

    /// Omit inherited choices so native settings remain in charge.
    pub fn claude_prompt_choices(
        &self,
        model: Option<String>,
        effort: Option<String>,
    ) -> (Option<String>, Option<String>) {
        (
            model.filter(|_| !self.claude.inherits_model),
            effort.filter(|_| !self.claude.inherits_effort),
        )
    }

    /// Keep appearance-only saves from pinning inherited model or effort values.
    pub fn prepare_saved_claude_choices(
        &mut self,
        editing: Option<&(String, String)>,
        defaults: &mut RuntimeDefaults,
    ) {
        let Some((model, effort)) = editing else {
            return;
        };
        let model_changed = defaults
            .model
            .as_deref()
            .is_some_and(|choice| choice != model);
        let effort_changed = defaults
            .effort
            .as_deref()
            .is_some_and(|choice| choice != effort);
        if self.claude.inherits_model && !model_changed {
            defaults.model = None;
            self.clear_pending_runtime_choice(RuntimeSetting::Model);
        } else {
            self.claude.inherits_model = false;
        }
        if (self.claude.inherits_effort && !effort_changed)
            || defaults
                .effort
                .as_deref()
                .is_none_or(|effort| effort == "auto")
        {
            defaults.effort = None;
            self.inherit_runtime_effort();
            self.claude.inherits_effort = true;
        } else {
            self.claude.inherits_effort = false;
        }
    }
}
