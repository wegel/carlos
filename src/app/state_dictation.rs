//! AppState impl: dictation profile setup, state transitions, and text commits.

#[cfg(test)]
use super::dictation_state::DictationPhase;
#[cfg(any(test, feature = "dictation"))]
use super::dictation_state::DictationProfileState;
use super::dictation_state::DictationRuntimeState;
use super::state::AppState;

impl AppState {
    // --- Configuration ---

    #[cfg(any(test, feature = "dictation"))]
    pub(super) fn configure_dictation(&mut self, profile: DictationProfileState) {
        self.dictation = DictationRuntimeState::with_profile(profile);
    }

    pub(super) fn disable_dictation(&mut self, reason: impl Into<String>) {
        self.dictation = DictationRuntimeState::disabled(reason);
    }

    #[cfg(test)]
    pub(super) fn dictation_phase(&self) -> &DictationPhase {
        self.dictation.phase()
    }

    pub(super) fn dictation_status_label(&self) -> Option<String> {
        self.dictation.status_label()
    }

    pub(super) fn dictation_active(&self) -> bool {
        self.dictation.is_active()
    }

    // --- Recording lifecycle ---

    pub(super) fn start_dictation_recording(&mut self) {
        if self.active_turn_id.is_some() {
            self.set_status("dictation unavailable while turn is active");
            return;
        }
        match self.dictation.start_recording() {
            Ok(()) => self.set_status("dictation recording"),
            Err(err) => self.set_status(err),
        }
    }

    pub(super) fn stop_dictation_recording(&mut self) {
        if self.dictation.stop_recording() {
            self.set_status("dictation transcribing");
        }
    }

    pub(super) fn cancel_dictation(&mut self) {
        if self.dictation.cancel() {
            self.set_status("dictation cancelled");
        }
    }

    #[cfg(test)]
    pub(super) fn apply_dictation_partial(&mut self, text: impl Into<String>) {
        self.dictation.apply_partial(text);
    }

    #[cfg(test)]
    pub(super) fn commit_dictation_final(&mut self, text: impl Into<String>) {
        if self.dictation.finish_transcription().is_some() {
            self.input_insert_text(text.into());
            self.set_status("dictation inserted");
        }
    }

    // --- Profile picker ---

    #[cfg(test)]
    pub(super) fn open_dictation_profile_picker(&mut self) {
        self.dictation.open_picker();
    }

    #[cfg(test)]
    pub(super) fn close_dictation_profile_picker(&mut self) {
        self.dictation.close_picker();
    }

    #[cfg(test)]
    pub(super) fn dictation_profile_picker_open(&self) -> bool {
        self.dictation.picker_open()
    }
}
