//! Share local dictation profiles, controls and private text insertion between terminal paths.

use crossterm::event::{KeyCode, KeyEvent};

#[cfg(test)]
use crate::app::dictation_state::DictationPhase;
#[cfg(any(test, feature = "dictation"))]
use crate::app::dictation_state::DictationProfileState;
use crate::app::dictation_state::{DictationEndpointMode, DictationRuntimeState};
use crate::app::notifications::is_ctrl_char;
use crate::app::view_state::ViewState;

impl ViewState {
    /// Consume the existing dictation gestures after higher-priority editors and overlays.
    pub(in crate::app) fn dictation_key(&mut self, key: KeyEvent, turn_active: bool) -> bool {
        match (key.code, key.modifiers) {
            (code, mods) if is_ctrl_char(code, mods, 'd') => {
                if self.dictation_recording() {
                    self.stop_dictation_recording();
                } else if self.dictation_active() {
                    self.restart_dictation_recording(turn_active);
                } else {
                    self.start_dictation_recording(turn_active);
                }
            }
            (KeyCode::F(7), mods) if mods.is_empty() => self.switch_dictation_profile_next(),
            (KeyCode::F(9), mods) if mods.is_empty() => self.toggle_dictation_endpoint_mode(),
            (KeyCode::Enter, mods) if mods.is_empty() && self.dictation_recording() => {
                self.stop_dictation_recording()
            }
            (KeyCode::Esc, mods) if mods.is_empty() && self.dictation_active() => {
                self.cancel_dictation()
            }
            _ => return false,
        }
        self.esc_armed_at = None;
        self.esc_interrupted_at = None;
        true
    }

    // --- Configuration ---

    #[cfg(test)]
    pub(in crate::app) fn configure_dictation(&mut self, profile: DictationProfileState) {
        self.dictation = DictationRuntimeState::with_profile(profile);
    }

    #[cfg(feature = "dictation")]
    pub(in crate::app) fn configure_dictation_profiles(
        &mut self,
        profiles: Vec<DictationProfileState>,
        active_id: &str,
    ) {
        self.dictation = DictationRuntimeState::with_profiles(profiles, active_id);
    }

    pub(in crate::app) fn disable_dictation(&mut self, reason: impl Into<String>) {
        self.dictation = DictationRuntimeState::disabled(reason);
    }

    #[cfg(test)]
    pub(in crate::app) fn dictation_phase(&self) -> &DictationPhase {
        self.dictation.phase()
    }

    pub(in crate::app) fn dictation_status_label(&self) -> Option<String> {
        self.dictation.status_label()
    }

    pub(in crate::app) fn dictation_active(&self) -> bool {
        self.dictation.is_active()
    }

    pub(in crate::app) fn dictation_recording(&self) -> bool {
        self.dictation.is_recording()
    }

    pub(in crate::app) fn dictation_endpoint_mode_label(&self) -> &str {
        self.dictation.endpoint_mode_label()
    }

    pub(in crate::app) fn toggle_dictation_endpoint_mode(&mut self) {
        let mode = match self.dictation.toggle_endpoint_mode() {
            Ok(mode) => mode,
            Err(err) => {
                self.dictation_hint(err);
                return;
            }
        };
        #[cfg(feature = "dictation")]
        if let Some(session) = &self.dictation_io.capture {
            session.set_auto_stop_enabled(mode == DictationEndpointMode::Auto);
        }
        self.dictation_hint(match mode {
            DictationEndpointMode::Auto => "dictation endpoint: auto",
            DictationEndpointMode::Manual => "dictation endpoint: manual (Enter to stop)",
        });
    }

    // --- Recording lifecycle ---

    pub(in crate::app) fn start_dictation_recording(&mut self, turn_active: bool) {
        if turn_active {
            self.dictation_hint("dictation unavailable while turn is active");
            return;
        }
        match self.dictation.start_recording() {
            Ok(()) => self.start_dictation_capture(),
            Err(err) => self.dictation_hint(err),
        }
    }

    pub(in crate::app) fn stop_dictation_recording(&mut self) {
        if !self.dictation.stop_recording() {
            return;
        }
        #[cfg(feature = "dictation")]
        {
            self.dictation_io.capture_id = None;
        }
        self.finish_dictation_capture();
    }

    pub(in crate::app) fn cancel_dictation(&mut self) {
        if self.dictation.cancel() {
            #[cfg(feature = "dictation")]
            {
                if let Some(cancel) = self.dictation_io.cancel.take() {
                    cancel.cancel();
                }
                self.dictation_io.request_id = None;
                self.dictation_io.capture_id = None;
                if let Some(session) = self.dictation_io.capture.take() {
                    session.cancel();
                }
            }
            self.dictation_hint("dictation cancelled");
        }
    }

    pub(in crate::app) fn restart_dictation_recording(&mut self, turn_active: bool) {
        self.cancel_dictation();
        self.start_dictation_recording(turn_active);
    }

    #[cfg(feature = "dictation")]
    pub(in crate::app) fn switch_dictation_profile_next(&mut self) {
        if self.dictation_active() {
            self.cancel_dictation();
        }
        match self.dictation.cycle_profile() {
            Ok(profile) => {
                let name = profile.name.clone();
                self.dictation_hint(format!("dictation profile: {name}"));
            }
            Err(err) => self.dictation_hint(format!("dictation profile unavailable: {err}")),
        }
    }

    #[cfg(not(feature = "dictation"))]
    pub(in crate::app) fn switch_dictation_profile_next(&mut self) {
        self.dictation_hint("dictation unavailable: rebuild with --features dictation");
    }

    #[cfg(test)]
    pub(in crate::app) fn apply_dictation_partial(&mut self, text: impl Into<String>) {
        self.dictation.apply_partial(text);
    }

    #[cfg(test)]
    pub(in crate::app) fn commit_dictation_final(&mut self, text: impl Into<String>) {
        if self.dictation.finish_transcription().is_some() {
            self.insert_dictation_text(text.into());
            self.dictation_hint("dictation inserted");
        }
    }

    // --- Profile picker ---

    #[cfg(test)]
    pub(in crate::app) fn open_dictation_profile_picker(&mut self) {
        self.dictation.open_picker();
    }

    #[cfg(test)]
    pub(in crate::app) fn close_dictation_profile_picker(&mut self) {
        self.dictation.close_picker();
    }

    #[cfg(test)]
    pub(in crate::app) fn dictation_profile_picker_open(&self) -> bool {
        self.dictation.picker_open()
    }
    pub(super) fn dictation_hint(&mut self, text: impl Into<String>) {
        self.status_hint = Some(text.into());
    }

    #[cfg(any(test, feature = "dictation"))]
    pub(super) fn insert_dictation_text(&mut self, text: String) {
        self.esc_armed_at = None;
        self.esc_interrupted_at = None;
        self.input.insert_str(text);
    }
}

#[cfg(test)]
#[path = "view_dictation_tests.rs"]
mod tests;
