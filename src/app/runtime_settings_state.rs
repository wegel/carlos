//! Model, effort, and summary settings state with pending-vs-confirmed cycling.

#[path = "runtime_settings_model_settings.rs"]
mod runtime_settings_model_settings;

use crate::app::tool_output_state::ToolOutputMode;
use crate::session::runtime_settings::RuntimeSettingsState;

// --- Default Options ---

pub(super) const DEFAULT_EFFORT_OPTIONS: [&str; 6] =
    ["none", "minimal", "low", "medium", "high", "xhigh"];
pub(super) const DEFAULT_SUMMARY_OPTIONS: [&str; 4] = ["auto", "concise", "detailed", "none"];

// --- Field Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModelSettingsField {
    Model,
    Effort,
    Summary,
    Context,
    DictationEndpoint,
    DesktopTheme,
    PhoneTheme,
    ToolLabels,
    ToolOutput,
}

// --- Settings State ---

pub(super) struct RuntimeSettingsView {
    pub(super) context: super::context_settings::ContextSettings,
    pub(super) show_model_settings: bool,
    pub(super) model_settings_field: ModelSettingsField,
    pub(super) model_settings_tool_output: ToolOutputMode,
    pub(super) model_settings_model_input: String,
    pub(super) model_settings_model_index: usize,
    pub(super) model_settings_effort_options: Vec<String>,
    pub(super) model_settings_effort_index: usize,
    pub(super) model_settings_summary_options: Vec<String>,
    pub(super) model_settings_summary_index: usize,
}

// --- Editor Logic ---

impl RuntimeSettingsView {
    pub(super) fn new() -> Self {
        Self {
            context: Default::default(),
            show_model_settings: false,
            model_settings_field: ModelSettingsField::Model,
            model_settings_tool_output: ToolOutputMode::Auto,
            model_settings_model_input: String::new(),
            model_settings_model_index: 0,
            model_settings_effort_options: default_effort_vec(),
            model_settings_effort_index: 3,
            model_settings_summary_options: DEFAULT_SUMMARY_OPTIONS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            model_settings_summary_index: 0,
        }
    }

    pub(super) fn open_model_settings(&mut self, runtime: &RuntimeSettingsState) {
        self.context.open();
        self.show_model_settings = true;
        self.model_settings_field = ModelSettingsField::Model;
        let preferred_model = runtime
            .pending_model()
            .as_deref()
            .or(runtime.current_model().as_deref())
            .unwrap_or("");
        self.model_settings_model_index = runtime
            .available_models()
            .iter()
            .position(|m| m.model == preferred_model)
            .unwrap_or(0);
        self.model_settings_model_input = runtime
            .available_models()
            .get(self.model_settings_model_index)
            .map(|m| m.model.clone())
            .unwrap_or_else(|| preferred_model.to_string());
        self.refresh_model_settings_efforts(runtime);
        let preferred_summary = runtime
            .pending_summary()
            .as_deref()
            .or(runtime.current_summary().as_deref())
            .unwrap_or("auto");
        self.model_settings_summary_index = self
            .model_settings_summary_options
            .iter()
            .position(|o| o == preferred_summary)
            .unwrap_or(0);
    }

    pub(super) fn close_model_settings(&mut self) {
        self.show_model_settings = false;
    }

    pub(super) fn model_settings_move_field(
        &mut self,
        runtime: &RuntimeSettingsState,
        forward: bool,
    ) {
        let fields = self.model_settings_fields(runtime);
        let len = fields.len();
        if len <= 1 {
            self.model_settings_field = ModelSettingsField::Model;
            return;
        }
        let current = fields
            .iter()
            .position(|field| *field == self.model_settings_field)
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        self.model_settings_field = fields[next];
    }

    pub(super) fn model_settings_cycle_effort(&mut self, step: isize) {
        if self.model_settings_effort_options.is_empty() {
            self.model_settings_effort_options = default_effort_vec();
        }
        let len = self.model_settings_effort_options.len() as isize;
        let cur = self.model_settings_effort_index as isize;
        let next = (cur + step).rem_euclid(len);
        self.model_settings_effort_index = next as usize;
    }

    pub(super) fn model_settings_cycle_model(
        &mut self,
        runtime: &RuntimeSettingsState,
        step: isize,
    ) {
        if runtime.available_models().is_empty() {
            return;
        }
        let len = runtime.available_models().len() as isize;
        let cur = self.model_settings_model_index as isize;
        let next = (cur + step).rem_euclid(len);
        self.model_settings_model_index = next as usize;
        if let Some(model) = runtime
            .available_models()
            .get(self.model_settings_model_index)
        {
            self.model_settings_model_input = model.model.clone();
        }
        self.refresh_model_settings_efforts(runtime);
    }

    pub(super) fn model_settings_cycle_summary(&mut self, step: isize) {
        let len = self.model_settings_summary_options.len() as isize;
        let cur = self.model_settings_summary_index as isize;
        let next = (cur + step).rem_euclid(len);
        self.model_settings_summary_index = next as usize;
    }

    pub(super) fn model_settings_has_model_choices(&self, runtime: &RuntimeSettingsState) -> bool {
        !runtime.available_models().is_empty()
    }

    pub(super) fn model_settings_insert_char(&mut self, ch: char) {
        self.model_settings_model_input.push(ch);
    }

    pub(super) fn model_settings_backspace(&mut self) {
        self.model_settings_model_input.pop();
    }
}

fn default_effort_vec() -> Vec<String> {
    DEFAULT_EFFORT_OPTIONS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn effort_index(value: &str) -> usize {
    DEFAULT_EFFORT_OPTIONS
        .iter()
        .position(|v| v.eq_ignore_ascii_case(value))
        .unwrap_or(3)
}
