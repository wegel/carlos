//! Delegate embedded-terminal dictation to the shared device view.

#[cfg(test)]
use super::dictation_state::{DictationPhase, DictationProfileState};
use super::state::AppState;
#[cfg(feature = "dictation")]
use crate::dictation::events::DictationEvent;

impl AppState {
    pub(super) fn dictation_status_label(&self) -> Option<String> {
        self.view.dictation_status_label()
    }

    pub(super) fn dictation_active(&self) -> bool {
        self.view.dictation_active()
    }

    pub(super) fn dictation_recording(&self) -> bool {
        self.view.dictation_recording()
    }

    pub(super) fn dictation_endpoint_mode_label(&self) -> &str {
        self.view.dictation_endpoint_mode_label()
    }

    pub(super) fn toggle_dictation_endpoint_mode(&mut self) {
        self.view.toggle_dictation_endpoint_mode()
    }

    pub(super) fn stop_dictation_recording(&mut self) {
        self.view.stop_dictation_recording()
    }

    pub(super) fn cancel_dictation(&mut self) {
        self.view.cancel_dictation()
    }

    pub(super) fn start_dictation_recording(&mut self) {
        self.view
            .start_dictation_recording(self.session.active_turn_id().is_some());
    }

    pub(super) fn restart_dictation_recording(&mut self) {
        self.view
            .restart_dictation_recording(self.session.active_turn_id().is_some());
    }

    pub(super) fn switch_dictation_profile_next(&mut self) {
        self.view.switch_dictation_profile_next();
    }

    #[cfg(feature = "dictation")]
    pub(super) fn configure_dictation_event_sender(
        &mut self,
        tx: std::sync::mpsc::Sender<DictationEvent>,
    ) {
        self.view.configure_dictation_event_sender(tx);
    }

    #[cfg(feature = "dictation")]
    pub(super) fn handle_dictation_event(&mut self, event: DictationEvent) {
        self.view.handle_dictation_event(event);
    }

    #[cfg(all(test, feature = "dictation"))]
    pub(super) fn last_dictation_audio_len(&self) -> Option<usize> {
        self.view.last_dictation_audio_len()
    }

    #[cfg(all(test, feature = "dictation"))]
    pub(super) fn configure_dictation_profiles(
        &mut self,
        profiles: Vec<DictationProfileState>,
        active_id: &str,
    ) {
        self.view.configure_dictation_profiles(profiles, active_id);
    }

    #[cfg(test)]
    pub(super) fn configure_dictation(&mut self, profile: DictationProfileState) {
        self.view.configure_dictation(profile);
    }

    #[cfg(test)]
    pub(super) fn dictation_phase(&self) -> &DictationPhase {
        self.view.dictation_phase()
    }

    #[cfg(test)]
    pub(super) fn apply_dictation_partial(&mut self, text: impl Into<String>) {
        self.view.apply_dictation_partial(text);
    }

    #[cfg(test)]
    pub(super) fn commit_dictation_final(&mut self, text: impl Into<String>) {
        self.view.commit_dictation_final(text);
    }

    #[cfg(test)]
    pub(super) fn open_dictation_profile_picker(&mut self) {
        self.view.open_dictation_profile_picker();
    }

    #[cfg(test)]
    pub(super) fn close_dictation_profile_picker(&mut self) {
        self.view.close_dictation_profile_picker();
    }

    #[cfg(test)]
    pub(super) fn dictation_profile_picker_open(&self) -> bool {
        self.view.dictation_profile_picker_open()
    }
}
