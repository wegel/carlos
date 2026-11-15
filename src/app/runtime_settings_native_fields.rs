//! Keep inherited summary values and model-specific effort choices honest in either settings editor.

use carlos_core::session::runtime_settings::RuntimeDefaults;

use super::{RuntimeSettingsRef, RuntimeSettingsView};

pub(in crate::app) const THREAD_SUMMARY: &str = "Use thread setting";

pub(in crate::app) struct NativeOmissions {
    pub effort: bool,
    pub summary: bool,
}

impl RuntimeSettingsView {
    pub(in crate::app) fn refresh_native_editor(
        &mut self,
        runtime: RuntimeSettingsRef<'_>,
        opening: bool,
    ) {
        if !model_supports_effort(runtime, self.model_settings_model_value(runtime)) {
            self.model_settings_effort_options.clear();
        }
        if runtime.current.summary.is_some() || runtime.pending.summary.is_some() {
            return;
        }
        if !self
            .model_settings_summary_options
            .iter()
            .any(|value| value == THREAD_SUMMARY)
        {
            self.model_settings_summary_options
                .insert(0, THREAD_SUMMARY.into());
            self.model_settings_summary_index = 0;
        }
        if opening {
            self.model_settings_summary_index = self
                .model_settings_summary_options
                .iter()
                .position(|value| value == THREAD_SUMMARY)
                .unwrap_or(0);
        }
    }

    pub(in crate::app) fn normalize_native_choices(
        &self,
        runtime: RuntimeSettingsRef<'_>,
        defaults: &mut RuntimeDefaults,
    ) -> NativeOmissions {
        let omissions = NativeOmissions {
            effort: !model_supports_effort(runtime, defaults.model.as_deref().unwrap_or("")),
            summary: defaults.summary.as_deref() == Some(THREAD_SUMMARY),
        };
        if omissions.effort {
            defaults.effort = None;
        }
        if omissions.summary {
            defaults.summary = None;
        }
        omissions
    }
}

pub(in crate::app) fn model_supports_effort(runtime: RuntimeSettingsRef<'_>, model: &str) -> bool {
    runtime
        .models
        .iter()
        .find(|entry| entry.model == model)
        .is_none_or(|entry| !entry.supported_efforts.is_empty())
}
