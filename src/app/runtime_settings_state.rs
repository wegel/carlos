use crate::app::RuntimeDefaults;
use crate::protocol::ModelInfo;

pub(super) const DEFAULT_EFFORT_OPTIONS: [&str; 6] =
    ["none", "minimal", "low", "medium", "high", "xhigh"];
pub(super) const DEFAULT_SUMMARY_OPTIONS: [&str; 4] = ["auto", "concise", "detailed", "none"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModelSettingsField {
    Model,
    Effort,
    Summary,
}

pub(super) struct RuntimeSettingsState {
    pub(super) current_model: Option<String>,
    pub(super) current_effort: Option<String>,
    pub(super) current_summary: Option<String>,
    pub(super) pending_model: Option<String>,
    pub(super) pending_effort: Option<String>,
    pub(super) pending_summary: Option<String>,
    pub(super) show_model_settings: bool,
    pub(super) model_settings_field: ModelSettingsField,
    pub(super) model_settings_model_input: String,
    pub(super) model_settings_model_index: usize,
    pub(super) model_settings_effort_options: Vec<String>,
    pub(super) model_settings_effort_index: usize,
    pub(super) model_settings_summary_options: Vec<String>,
    pub(super) model_settings_summary_index: usize,
    pub(super) available_models: Vec<ModelInfo>,
}

impl RuntimeSettingsState {
    pub(super) fn new() -> Self {
        Self {
            current_model: None,
            current_effort: None,
            current_summary: None,
            pending_model: None,
            pending_effort: None,
            pending_summary: None,
            show_model_settings: false,
            model_settings_field: ModelSettingsField::Model,
            model_settings_model_input: String::new(),
            model_settings_model_index: 0,
            model_settings_effort_options: Vec::new(),
            model_settings_effort_index: 3,
            model_settings_summary_options: DEFAULT_SUMMARY_OPTIONS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            model_settings_summary_index: 0,
            available_models: Vec::new(),
        }
    }

    pub(super) fn set_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.current_model = model.and_then(normalize_non_empty);
        self.current_effort = effort.and_then(normalize_non_empty);
        self.current_summary = summary.and_then(normalize_non_empty);
    }

    pub(super) fn set_available_models(&mut self, mut models: Vec<ModelInfo>) {
        models.sort_by_key(|m| !m.is_default);
        self.available_models = models;
    }

    pub(super) fn queue_runtime_settings(
        &mut self,
        model: Option<String>,
        effort: Option<String>,
        summary: Option<String>,
    ) {
        self.pending_model = model.and_then(normalize_non_empty);
        self.pending_effort = effort.and_then(normalize_non_empty);
        self.pending_summary = summary.and_then(normalize_non_empty);
    }

    #[cfg(test)]
    pub(super) fn take_pending_runtime_settings(
        &mut self,
    ) -> (Option<String>, Option<String>, Option<String>) {
        (
            self.pending_model.clone(),
            self.pending_effort.clone(),
            self.pending_summary.clone(),
        )
    }

    pub(super) fn next_turn_runtime_settings(
        &self,
    ) -> (Option<String>, Option<String>, Option<String>) {
        (
            self.pending_model
                .clone()
                .or_else(|| self.current_model.clone()),
            self.pending_effort
                .clone()
                .or_else(|| self.current_effort.clone()),
            self.pending_summary
                .clone()
                .or_else(|| self.current_summary.clone()),
        )
    }

    pub(super) fn mark_runtime_settings_applied(&mut self) {
        if self.pending_model.is_some()
            || self.pending_effort.is_some()
            || self.pending_summary.is_some()
        {
            self.current_model = self.pending_model.clone();
            self.current_effort = self.pending_effort.clone();
            self.current_summary = self.pending_summary.clone();
            self.pending_model = None;
            self.pending_effort = None;
            self.pending_summary = None;
        }
    }

    pub(super) fn runtime_settings_label(&self) -> String {
        let shown_model = self
            .pending_model
            .as_deref()
            .or(self.current_model.as_deref())
            .unwrap_or("model?");
        let shown_effort = self
            .pending_effort
            .as_deref()
            .or(self.current_effort.as_deref())
            .unwrap_or("effort?");
        let shown_summary = self
            .pending_summary
            .as_deref()
            .or(self.current_summary.as_deref())
            .unwrap_or("summary?");
        let mut out = format!("{shown_model}/{shown_effort}/{shown_summary}");

        let pending_differs = self.pending_model.as_deref() != self.current_model.as_deref()
            || self.pending_effort.as_deref() != self.current_effort.as_deref()
            || self.pending_summary.as_deref() != self.current_summary.as_deref();
        if self.runtime_settings_pending() && pending_differs {
            out.push('*');
        }

        out
    }

    pub(super) fn has_runtime_settings(&self) -> bool {
        self.current_model.is_some()
            || self.current_effort.is_some()
            || self.current_summary.is_some()
    }

    pub(super) fn runtime_settings_pending(&self) -> bool {
        self.pending_model.is_some()
            || self.pending_effort.is_some()
            || self.pending_summary.is_some()
    }

    pub(super) fn open_model_settings(&mut self) {
        self.show_model_settings = true;
        self.model_settings_field = ModelSettingsField::Model;
        let preferred_model = self
            .pending_model
            .as_deref()
            .or(self.current_model.as_deref())
            .unwrap_or("");
        self.model_settings_model_index = if self.available_models.is_empty() {
            0
        } else {
            self.available_models
                .iter()
                .position(|m| m.model == preferred_model)
                .unwrap_or(0)
        };
        self.model_settings_model_input = self
            .available_models
            .get(self.model_settings_model_index)
            .map(|m| m.model.clone())
            .unwrap_or_else(|| preferred_model.to_string());
        self.refresh_model_settings_efforts();
        let preferred_summary = self
            .pending_summary
            .as_deref()
            .or(self.current_summary.as_deref())
            .unwrap_or("auto");
        self.model_settings_summary_index = self
            .model_settings_summary_options
            .iter()
            .position(|option| option == preferred_summary)
            .unwrap_or(0);
    }

    pub(super) fn close_model_settings(&mut self) {
        self.show_model_settings = false;
    }

    pub(super) fn model_settings_move_field(&mut self, forward: bool) {
        self.model_settings_field = match (self.model_settings_field, forward) {
            (ModelSettingsField::Model, true) => ModelSettingsField::Effort,
            (ModelSettingsField::Effort, true) => ModelSettingsField::Summary,
            (ModelSettingsField::Summary, true) => ModelSettingsField::Model,
            (ModelSettingsField::Model, false) => ModelSettingsField::Summary,
            (ModelSettingsField::Effort, false) => ModelSettingsField::Model,
            (ModelSettingsField::Summary, false) => ModelSettingsField::Effort,
        };
    }

    pub(super) fn model_settings_cycle_effort(&mut self, step: isize) {
        if self.model_settings_effort_options.is_empty() {
            self.model_settings_effort_options = DEFAULT_EFFORT_OPTIONS
                .iter()
                .map(|s| (*s).to_string())
                .collect();
        }
        let len = self.model_settings_effort_options.len() as isize;
        let cur = self.model_settings_effort_index as isize;
        let next = (cur + step).rem_euclid(len);
        self.model_settings_effort_index = next as usize;
    }

    pub(super) fn model_settings_cycle_model(&mut self, step: isize) {
        if self.available_models.is_empty() {
            return;
        }
        let len = self.available_models.len() as isize;
        let cur = self.model_settings_model_index as isize;
        let next = (cur + step).rem_euclid(len);
        self.model_settings_model_index = next as usize;
        if let Some(model) = self.available_models.get(self.model_settings_model_index) {
            self.model_settings_model_input = model.model.clone();
        }
        self.refresh_model_settings_efforts();
    }

    pub(super) fn model_settings_cycle_summary(&mut self, step: isize) {
        let len = self.model_settings_summary_options.len() as isize;
        let cur = self.model_settings_summary_index as isize;
        let next = (cur + step).rem_euclid(len);
        self.model_settings_summary_index = next as usize;
    }

    pub(super) fn model_settings_has_model_choices(&self) -> bool {
        !self.available_models.is_empty()
    }

    pub(super) fn model_settings_insert_char(&mut self, ch: char) {
        self.model_settings_model_input.push(ch);
    }

    pub(super) fn model_settings_backspace(&mut self) {
        self.model_settings_model_input.pop();
    }

    pub(super) fn apply_model_settings(&mut self) -> RuntimeDefaults {
        let model = normalize_non_empty(self.model_settings_model_value().to_string());
        let effort = normalize_non_empty(self.model_settings_effort_value().to_string());
        let summary = normalize_non_empty(self.model_settings_summary_value().to_string());
        let defaults = RuntimeDefaults {
            model: model.clone(),
            effort: effort.clone(),
            summary: summary.clone(),
        };
        self.queue_runtime_settings(model, effort, summary);
        self.show_model_settings = false;
        defaults
    }

    pub(super) fn model_settings_model_value(&self) -> &str {
        if let Some(model) = self.available_models.get(self.model_settings_model_index) {
            return model.model.as_str();
        }
        self.model_settings_model_input.as_str()
    }

    pub(super) fn model_settings_effort_value(&self) -> &str {
        self.model_settings_effort_options
            .get(self.model_settings_effort_index)
            .map(String::as_str)
            .unwrap_or("medium")
    }

    pub(super) fn model_settings_summary_value(&self) -> &str {
        self.model_settings_summary_options
            .get(self.model_settings_summary_index)
            .map(String::as_str)
            .unwrap_or("auto")
    }

    #[cfg(test)]
    pub(super) fn apply_default_reasoning_summary(&mut self, summary: Option<String>) {
        if self.current_summary.is_none() && self.pending_summary.is_none() {
            self.pending_summary = summary.and_then(normalize_non_empty);
        }
    }

    fn refresh_model_settings_efforts(&mut self) {
        let requested = self
            .pending_effort
            .as_deref()
            .or(self.current_effort.as_deref())
            .unwrap_or("medium")
            .to_string();

        let (options, default_effort) =
            if let Some(model) = self.available_models.get(self.model_settings_model_index) {
                let options = if model.supported_efforts.is_empty() {
                    DEFAULT_EFFORT_OPTIONS
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect::<Vec<_>>()
                } else {
                    model.supported_efforts.clone()
                };
                (options, model.default_effort.clone())
            } else {
                (
                    DEFAULT_EFFORT_OPTIONS
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect::<Vec<_>>(),
                    None,
                )
            };

        self.model_settings_effort_options = options;
        self.model_settings_effort_index = self
            .model_settings_effort_options
            .iter()
            .position(|e| e.eq_ignore_ascii_case(&requested))
            .or_else(|| {
                default_effort.as_deref().and_then(|d| {
                    self.model_settings_effort_options
                        .iter()
                        .position(|e| e.eq_ignore_ascii_case(d))
                })
            })
            .unwrap_or_else(|| {
                effort_index("medium")
                    .min(self.model_settings_effort_options.len().saturating_sub(1))
            });
    }
}

fn normalize_non_empty(s: String) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn effort_index(value: &str) -> usize {
    DEFAULT_EFFORT_OPTIONS
        .iter()
        .position(|v| v.eq_ignore_ascii_case(value))
        .unwrap_or(3)
}
