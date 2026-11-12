//! Choose full or folded tool output from an explicit preference and terminal size.

use super::models::{Message, MessageKind, Role};
use super::view_models::{DisplayKind, DisplayMessage, TerminalSize};

/// Let the terminal size choose, or force one view until the user changes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ToolOutputMode {
    #[default]
    Auto,
    Compact,
    Full,
}

impl ToolOutputMode {
    /// Read the names accepted by `/tools` and the saved UI settings.
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "compact" => Some(Self::Compact),
            "full" => Some(Self::Full),
            _ => None,
        }
    }

    /// Return the command spelling of this mode.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Compact => "compact",
            Self::Full => "full",
        }
    }

    /// Cycle the three choices with either arrow key.
    pub(super) fn cycle(self, step: isize) -> Self {
        let modes = [Self::Auto, Self::Compact, Self::Full];
        let index = modes.iter().position(|&mode| mode == self).unwrap_or(0) as isize;
        modes[(index + step).rem_euclid(3) as usize]
    }
}

/// Remember the user's choice separately from Auto's current choice.
#[derive(Default)]
pub(super) struct ToolOutputState {
    pub(super) mode: ToolOutputMode,
    auto_compact: Option<bool>,
}

impl ToolOutputState {
    /// Use different shrink/grow cutoffs so small resize changes do not flap.
    pub(super) fn update_size(&mut self, size: TerminalSize) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        let small = size.is_small();
        let roomy = size.width >= 84 && size.height >= 22;
        self.auto_compact = Some(match self.auto_compact {
            Some(true) => !roomy,
            _ => small,
        });
    }

    /// Return the effective view without changing the saved preference.
    pub(super) fn compact(&self) -> bool {
        self.compact_for(self.mode)
    }

    fn compact_for(&self, mode: ToolOutputMode) -> bool {
        match mode {
            ToolOutputMode::Auto => self.auto_compact.unwrap_or(false),
            ToolOutputMode::Compact => true,
            ToolOutputMode::Full => false,
        }
    }

    /// Show Auto's current choice beside the value in the settings panel.
    pub(super) fn settings_label(&self, mode: ToolOutputMode) -> &'static str {
        match mode {
            ToolOutputMode::Auto if self.compact_for(mode) => "Auto (Compact)",
            ToolOutputMode::Auto => "Auto (Full)",
            ToolOutputMode::Compact => "Compact",
            ToolOutputMode::Full => "Full",
        }
    }
}

/// Replace tool contents with a label, including short results and failures.
pub(super) fn compact_tool_message(message: &Message, rows: usize) -> Option<DisplayMessage<'_>> {
    if !matches!(message.role, Role::ToolCall | Role::ToolOutput)
        && message.kind != MessageKind::Diff
    {
        return None;
    }
    let command_body = message.role == Role::ToolCall && message.text.contains('\n');
    Some(DisplayMessage {
        source: message,
        kind: DisplayKind::ToolSummary {
            rows: if command_body { 0 } else { rows },
        },
    })
}
