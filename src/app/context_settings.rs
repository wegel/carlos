//! Edit and save backend context sizes for the next Carlos launch.

use anyhow::Result;

use super::state::AppState;
use super::Role;
use crate::backend::{BackendClient, BackendKind};
use crate::context_preferences;

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

    /// Explain what the selected size changes in the active backend.
    pub(super) fn help(&self) -> &str {
        if let Some(error) = self.error.as_deref() {
            return error;
        }
        if self.attached {
            return "Attached Codex server owns the size";
        }
        if self.backend == Some(BackendKind::Claude) && shell_override() {
            return "Shell sets size; choose Auto";
        }
        "Restart Carlos to apply size"
    }
}

/// Load one backend's default without enabling unsupported controls for Grok.
pub(super) fn initialize(app: &mut AppState, backend: BackendKind, attached: bool) -> Result<()> {
    let saved = if attached {
        None
    } else {
        context_preferences::load(backend)?
    };
    app.runtime.context = ContextSettings {
        backend: Some(backend),
        attached,
        saved,
        ..Default::default()
    };
    app.runtime.context.open();
    Ok(())
}

/// Reject invalid drafts before any settings or appearance choices are applied.
pub(super) fn save(client: &dyn BackendClient, app: &mut AppState) -> bool {
    if app.runtime.context.backend.is_none() || app.runtime.context.attached {
        return true;
    }
    let tokens = match context_preferences::parse(&app.runtime.context.draft) {
        Ok(tokens) => tokens,
        Err(error) => {
            app.runtime.context.error = Some(error.to_string());
            return false;
        }
    };
    if tokens == app.runtime.context.saved {
        return true;
    }
    if client.kind() == BackendKind::Claude && tokens.is_some() && shell_override() {
        app.runtime.context.error =
            Some("Unset CLAUDE_CODE_AUTO_COMPACT_WINDOW before choosing a size".into());
        return false;
    }
    if let Err(error) = context_preferences::save(client.kind(), tokens) {
        app.runtime.context.error = Some(error.to_string());
        return false;
    }
    app.runtime.context.saved = tokens;
    app.append_message(
        Role::System,
        "Context size saved; restart Carlos to apply it. The model may use a smaller maximum.",
    );
    true
}

fn shell_override() -> bool {
    std::env::var("CLAUDE_CODE_AUTO_COMPACT_WINDOW").is_ok_and(|value| !value.is_empty())
}
