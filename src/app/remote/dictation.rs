//! Keep microphone events and dictated text on the terminal device.

use crossterm::event::KeyEvent;

use super::Device;
use crate::app::cli::CliOptions;

impl Device {
    /// Load local profiles and connect local capture and transcription events.
    pub(super) fn configure_dictation(&mut self, options: &CliOptions) {
        self.view
            .configure_dictation_options(options.dictation_profile.as_deref());
        #[cfg(feature = "dictation")]
        self.view.connect_local_dictation_events();
    }

    /// Insert checked dictated text into this device's private composer.
    pub(super) fn drain_dictation(&mut self) -> bool {
        #[cfg(feature = "dictation")]
        {
            self.view.drain_local_dictation_events()
        }
        #[cfg(not(feature = "dictation"))]
        {
            false
        }
    }

    /// Handle local dictation keys after overlays have declined them.
    pub(super) fn dictation_key(&mut self, key: KeyEvent) -> bool {
        let turn_active = self
            .client
            .session(&self.session)
            .is_some_and(|replica| replica.info().current_turn.is_some());
        self.view.dictation_key(key, turn_active)
    }

    /// Add the active profile and endpoint label beside the ordinary status.
    pub(super) fn dictation_status(&self, status: String) -> String {
        match self.view.dictation_status_label() {
            Some(label) => format!("{label} · {status}"),
            None => status,
        }
    }
}

#[cfg(test)]
#[path = "dictation_tests.rs"]
mod tests;
