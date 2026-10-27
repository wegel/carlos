//! Choose the desktop gutter or phone section headers independently of tool folding.

use super::models::TerminalSize;
use super::state::AppState;

/// Choose how Carlos separates transcript blocks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum LayoutMode {
    #[default]
    Auto,
    Desktop,
    Phone,
}

impl LayoutMode {
    /// Read a layout name from the saved UI settings.
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "desktop" => Some(Self::Desktop),
            "phone" => Some(Self::Phone),
            _ => None,
        }
    }

    /// Return the spelling stored in the UI settings file.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Desktop => "desktop",
            Self::Phone => "phone",
        }
    }

    /// Move through Auto, Desktop, and Phone with either arrow key.
    pub(super) fn cycle(self, step: isize) -> Self {
        let modes = [Self::Auto, Self::Desktop, Self::Phone];
        let index = modes.iter().position(|&mode| mode == self).unwrap_or(0) as isize;
        modes[(index + step).rem_euclid(3) as usize]
    }

    /// Resolve an explicit choice or use the terminal size.
    pub(super) fn phone(self, size: TerminalSize) -> bool {
        match self {
            Self::Auto => size.is_small(),
            Self::Desktop => false,
            Self::Phone => true,
        }
    }
}

/// Keep the applied layout and the settings draft separate.
#[derive(Default)]
pub(super) struct LayoutState {
    pub(super) mode: LayoutMode,
    pub(super) draft: LayoutMode,
    pub(super) phone: bool,
    small: bool,
}

impl LayoutState {
    /// Update the active view and the Auto label after a resize.
    pub(super) fn update_size(&mut self, size: TerminalSize) {
        self.small = size.is_small();
        self.phone = self.mode.phone(size);
    }

    /// Describe the settings draft without applying it to the transcript.
    pub(super) fn draft_label(&self) -> &'static str {
        match self.draft {
            LayoutMode::Auto if self.small => "Auto (Phone)",
            LayoutMode::Auto => "Auto (Desktop)",
            LayoutMode::Desktop => "Desktop",
            LayoutMode::Phone => "Phone",
        }
    }
}

impl AppState {
    /// Reserve blank padding on phones or a glyph and space on desktops.
    pub(super) fn content_x(&self, size: TerminalSize) -> usize {
        if self.layout.mode.phone(size) {
            1
        } else {
            super::MSG_CONTENT_X
        }
    }

    /// Leave a blank column at each body row’s right edge.
    pub(super) fn transcript_width(&self, size: TerminalSize) -> usize {
        size.width.saturating_sub(self.content_x(size) + 1)
    }
}
