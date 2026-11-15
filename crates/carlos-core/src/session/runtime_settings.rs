//! Own backend model choices and pending settings independently of an editor.

use super::model_info::ModelInfo;
use super::state::SessionState;

/// Carry model choices between app startup, the session, and the defaults file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDefaults {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub summary: Option<String>,
}

/// Identify one backend choice when recording or dropping an override.
#[derive(Clone, Copy)]
pub enum RuntimeSetting {
    Model,
    Effort,
    Summary,
}

/// Own backend capabilities, acknowledged choices, and pending turn overrides.
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
pub struct RuntimeSettingsState {
    startup_preferences: RuntimeDefaults,
    context_shell_override: Option<bool>,
    pub(in crate::session) native_settings_pending: bool,
    pub(in crate::session) supports_effort: bool,
    pub(in crate::session) supports_summary: bool,
    pub(in crate::session) current_model: Option<String>,
    pub(in crate::session) current_effort: Option<String>,
    pub(in crate::session) current_summary: Option<String>,
    pub(in crate::session) pending_model: Option<String>,
    pub(in crate::session) pending_effort: Option<String>,
    pub(in crate::session) pending_summary: Option<String>,
    pub(in crate::session) available_models: Vec<ModelInfo>,
    pub(in crate::session) catalog_hash: blake3::Hash,
}

impl RuntimeSettingsState {
    pub(in crate::session) fn new(preferences: RuntimeDefaults) -> Self {
        Self {
            startup_preferences: preferences,
            context_shell_override: None,
            native_settings_pending: false,
            supports_effort: true,
            supports_summary: true,
            current_model: None,
            current_effort: None,
            current_summary: None,
            pending_model: None,
            pending_effort: None,
            pending_summary: None,
            available_models: Vec::new(),
            catalog_hash: blake3::hash(&[]),
        }
    }

    /// Borrow saved startup choices before a backend resolves its effective settings.
    pub fn startup_preferences(&self) -> &RuntimeDefaults {
        &self.startup_preferences
    }

    /// Borrow the supports effort fact.
    pub fn supports_effort(&self) -> bool {
        self.supports_effort
    }

    /// Borrow the supports summary fact.
    pub fn supports_summary(&self) -> bool {
        self.supports_summary
    }

    /// Borrow the current model fact.
    pub fn current_model(&self) -> &Option<String> {
        &self.current_model
    }

    /// Borrow the current effort fact.
    pub fn current_effort(&self) -> &Option<String> {
        &self.current_effort
    }

    /// Borrow the current summary fact.
    pub fn current_summary(&self) -> &Option<String> {
        &self.current_summary
    }

    /// Borrow the pending model fact.
    pub fn pending_model(&self) -> &Option<String> {
        &self.pending_model
    }

    /// Borrow the pending effort fact.
    pub fn pending_effort(&self) -> &Option<String> {
        &self.pending_effort
    }

    /// Borrow the pending summary fact.
    pub fn pending_summary(&self) -> &Option<String> {
        &self.pending_summary
    }

    /// Borrow the available models fact.
    pub fn available_models(&self) -> &[ModelInfo] {
        &self.available_models
    }
}

impl SessionState {
    /// Capture whether the launched child's context-size variable has a nonempty value.
    pub fn configure_context_shell_override(&mut self, effective: Option<&str>) {
        self.runtime.context_shell_override =
            Some(effective.is_some_and(|value| !value.is_empty()));
    }

    /// Prefer captured child settings while legacy callers retain worker-side environment reads.
    pub(crate) fn context_shell_override(&self) -> Option<bool> {
        self.runtime.context_shell_override
    }

    /// Borrow backend settings without exposing mutable facts.
    pub fn runtime_settings(&self) -> &RuntimeSettingsState {
        &self.runtime
    }

    /// Set runtime capabilities.
    pub fn set_runtime_capabilities(&mut self, supports_effort: bool, supports_summary: bool) {
        self.runtime.supports_effort = supports_effort;
        self.runtime.supports_summary = supports_summary;
        if !supports_effort {
            self.runtime.current_effort = None;
            self.runtime.pending_effort = None;
        }
        if !supports_summary {
            self.runtime.current_summary = None;
            self.runtime.pending_summary = None;
        }
    }

    /// Set runtime settings.
    pub fn set_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.runtime.current_model = model.and_then(normalize_non_empty);
        self.runtime.current_effort = effort.and_then(normalize_non_empty);
        self.runtime.current_summary = summary.and_then(normalize_non_empty);
    }

    /// Merge runtime settings.
    pub fn merge_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        if let Some(model) = model.and_then(normalize_non_empty) {
            self.runtime.current_model = Some(model);
        }
        if self.runtime.supports_effort {
            if let Some(effort) = effort.and_then(normalize_non_empty) {
                self.runtime.current_effort = Some(effort);
            }
        }
        if self.runtime.supports_summary {
            if let Some(summary) = summary.and_then(normalize_non_empty) {
                self.runtime.current_summary = Some(summary);
            }
        }
    }

    /// Set available models.
    pub fn set_available_models(&mut self, mut models: Vec<ModelInfo>) {
        models.sort_by_key(|m| !m.is_default);
        self.runtime.catalog_hash = super::settings_publication::catalog_hash(&models);
        self.runtime.available_models = models;
    }

    /// Queue runtime settings.
    pub fn queue_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.runtime.pending_model = model.and_then(normalize_non_empty);
        self.runtime.pending_effort = effort.and_then(normalize_non_empty);
        self.runtime.pending_summary = summary.and_then(normalize_non_empty);
    }

    /// Copy pending choices without consuming them.
    pub fn pending_runtime_settings(&self) -> (Option<String>, Option<String>, Option<String>) {
        (
            self.runtime.pending_model.clone(),
            self.runtime.pending_effort.clone(),
            self.runtime.pending_summary.clone(),
        )
    }

    /// Next turn runtime settings.
    pub fn next_turn_runtime_settings(&self) -> (Option<String>, Option<String>, Option<String>) {
        (
            self.runtime
                .pending_model
                .clone()
                .or_else(|| self.runtime.current_model.clone()),
            self.runtime
                .pending_effort
                .clone()
                .or_else(|| self.runtime.current_effort.clone()),
            self.runtime
                .pending_summary
                .clone()
                .or_else(|| self.runtime.current_summary.clone()),
        )
    }

    /// Mark runtime settings applied.
    pub fn mark_runtime_settings_applied(&mut self) {
        if self.runtime.pending_model.is_some()
            || self.runtime.pending_effort.is_some()
            || self.runtime.pending_summary.is_some()
        {
            self.runtime.current_model = self.runtime.pending_model.clone();
            self.runtime.current_effort = self.runtime.pending_effort.clone();
            self.runtime.current_summary = self.runtime.pending_summary.clone();
            self.runtime.pending_model = None;
            self.runtime.pending_effort = None;
            self.runtime.pending_summary = None;
        }
    }

    /// Runtime settings label.
    pub fn runtime_settings_label(&self) -> String {
        let shown_model = self
            .runtime
            .pending_model
            .as_deref()
            .or(self.runtime.current_model.as_deref())
            .unwrap_or("model?");
        let shown_effort = self
            .runtime
            .pending_effort
            .as_deref()
            .or(self.runtime.current_effort.as_deref());
        let shown_summary = self
            .runtime
            .pending_summary
            .as_deref()
            .or(self.runtime.current_summary.as_deref());

        let mut parts = vec![shown_model.to_string()];
        if let Some(effort) = shown_effort {
            parts.push(effort.to_string());
        }
        if let Some(summary) = shown_summary {
            parts.push(summary.to_string());
        }

        let mut out = parts.join("/");

        let pending_differs = self.runtime.pending_model.as_deref()
            != self.runtime.current_model.as_deref()
            || self.runtime.pending_effort.as_deref() != self.runtime.current_effort.as_deref()
            || self.runtime.pending_summary.as_deref() != self.runtime.current_summary.as_deref();
        if self.runtime_settings_pending() && pending_differs {
            out.push('*');
        }

        out
    }

    /// Runtime settings pending.
    pub(crate) fn runtime_settings_pending(&self) -> bool {
        self.runtime.pending_model.is_some()
            || self.runtime.pending_effort.is_some()
            || self.runtime.pending_summary.is_some()
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Apply the fixture preference when the thread has no explicit reasoning choice.
    pub fn apply_default_reasoning_summary(&mut self, summary: Option<String>) {
        if self.runtime.current_summary.is_none() && self.runtime.pending_summary.is_none() {
            self.runtime.pending_summary = summary.and_then(normalize_non_empty);
        }
    }
}

/// Trim a saved or submitted choice and discard empty strings.
pub fn normalize_non_empty(s: String) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
