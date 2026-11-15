//! Edit and save backend context sizes for the next Carlos launch.

use carlos_core::backend::BackendKind;
use carlos_core::context_preferences;

const PRESETS: [Option<u64>; 7] = [
    None,
    Some(100_000),
    Some(200_000),
    Some(300_000),
    Some(500_000),
    Some(750_000),
    Some(1_000_000),
];

/// Keep the editor's draft separate from the saved choice.
#[derive(Default)]
pub(super) struct ContextSettings {
    pub(super) backend: Option<BackendKind>,
    pub(super) attached: bool,
    pub(super) draft: String,
    pub(super) error: Option<String>,
    saved: Option<u64>,
    replace_on_type: bool,
}

impl ContextSettings {
    /// Build an editor from checked backend facts without reading this device's host files.
    pub(super) fn from_saved(
        backend: Option<BackendKind>,
        attached: bool,
        saved: Option<u64>,
    ) -> Self {
        let mut editor = Self {
            backend,
            attached,
            saved,
            ..Default::default()
        };
        editor.open();
        editor
    }

    /// Restore the saved value whenever the user opens the editor.
    pub(super) fn open(&mut self) {
        self.draft = context_preferences::label(self.saved);
        self.replace_on_type = true;
        self.error = None;
    }

    /// Cycle presets without rounding a custom token count down accidentally.
    pub(super) fn cycle(&mut self, direction: isize) {
        if self.attached {
            return;
        }
        let value = context_preferences::parse(&self.draft).unwrap_or(None);
        let index = PRESETS.iter().position(|entry| *entry == value);
        let next = match index {
            Some(index) => (index as isize + direction).rem_euclid(PRESETS.len() as isize) as usize,
            None if direction > 0 => PRESETS.iter().position(|entry| *entry > value).unwrap_or(0),
            None => PRESETS
                .iter()
                .rposition(|entry| *entry < value)
                .unwrap_or(PRESETS.len() - 1),
        };
        self.draft = context_preferences::label(PRESETS[next]);
        self.replace_on_type = true;
        self.error = None;
    }

    /// Allow a typed size to replace the displayed preset.
    pub(super) fn insert(&mut self, text: &str) {
        if self.attached {
            return;
        }
        if self.replace_on_type {
            self.draft.clear();
        }
        self.replace_on_type = false;
        if self.draft.len() + text.len() <= 24 {
            self.draft.push_str(text);
        }
        self.error = None;
    }

    /// Clear a preset, or remove the final character of a typed count.
    pub(super) fn backspace(&mut self) {
        if self.attached {
            return;
        }
        if self.replace_on_type {
            self.draft.clear();
        } else {
            self.draft.pop();
        }
        self.replace_on_type = false;
        self.error = None;
    }
}
