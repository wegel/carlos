//! Choose a transcript theme independently for wide and small terminals.

use super::models::TerminalSize;
use super::state::AppState;

/// Choose how Carlos separates transcript entries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum TranscriptTheme {
    #[default]
    Gutter,
    Flow,
}

impl TranscriptTheme {
    /// Read a saved theme, accepting the former device-specific names.
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "gutter" | "desktop" => Some(Self::Gutter),
            "flow" | "phone" => Some(Self::Flow),
            _ => None,
        }
    }

    /// Return the spelling stored in the settings file.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Gutter => "gutter",
            Self::Flow => "flow",
        }
    }

    /// Return the name shown in Ctrl+M.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Gutter => "Gutter",
            Self::Flow => "Flow",
        }
    }

    /// Cycle the available themes in either direction.
    pub(super) fn cycle(self, step: isize) -> Self {
        if step.rem_euclid(2) == 0 {
            self
        } else {
            match self {
                Self::Gutter => Self::Flow,
                Self::Flow => Self::Gutter,
            }
        }
    }

    /// Use inline symbols and section headings for Flow.
    pub(super) fn is_flow(self) -> bool {
        self == Self::Flow
    }
}

/// Keep independent theme choices for the two screen sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ThemePreferences {
    pub(super) desktop: TranscriptTheme,
    pub(super) phone: TranscriptTheme,
}

impl Default for ThemePreferences {
    fn default() -> Self {
        Self {
            desktop: TranscriptTheme::Gutter,
            phone: TranscriptTheme::Flow,
        }
    }
}

impl ThemePreferences {
    /// Select a theme using the shared small-terminal threshold.
    pub(super) fn for_size(self, size: TerminalSize) -> TranscriptTheme {
        if size.is_small() {
            self.phone
        } else {
            self.desktop
        }
    }

    /// Preserve a former global override when reading older settings.
    pub(super) fn from_legacy(value: Option<&str>) -> Self {
        match value.and_then(TranscriptTheme::parse) {
            Some(theme) => Self {
                desktop: theme,
                phone: theme,
            },
            None => Self::default(),
        }
    }
}

/// Keep applied choices, settings drafts, and the active theme separate.
#[derive(Default)]
pub(super) struct LayoutState {
    pub(super) themes: ThemePreferences,
    pub(super) draft: ThemePreferences,
    pub(super) active: TranscriptTheme,
}

impl LayoutState {
    /// Resolve the theme after a resize or settings change.
    pub(super) fn update_size(&mut self, size: TerminalSize) {
        self.active = self.themes.for_size(size);
    }

    /// Resolve Flow without depending on the last render pass.
    pub(super) fn uses_flow(&self, size: TerminalSize) -> bool {
        self.themes.for_size(size).is_flow()
    }
}

impl AppState {
    /// Reserve blank padding in Flow or a glyph and space in Gutter.
    pub(super) fn content_x(&self, size: TerminalSize) -> usize {
        if self.layout.uses_flow(size) {
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
