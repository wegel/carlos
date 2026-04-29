//! AppState impl: dictation profile setup, state transitions, and text commits.

#[cfg(test)]
use super::dictation_state::DictationPhase;
#[cfg(any(test, feature = "dictation"))]
use super::dictation_state::DictationProfileState;
use super::dictation_state::DictationRuntimeState;
use super::state::AppState;
#[cfg(feature = "dictation")]
use crate::dictation::capture::{DictationCaptureSession, DictationEvent};

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

    pub(super) fn dictation_recording(&self) -> bool {
        self.dictation.is_recording()
    }

    #[cfg(feature = "dictation")]
    pub(super) fn configure_dictation_event_sender(
        &mut self,
        tx: std::sync::mpsc::Sender<DictationEvent>,
    ) {
        self.dictation_events_tx = Some(tx);
    }

    #[cfg(feature = "dictation")]
    pub(super) fn handle_dictation_event(&mut self, event: DictationEvent) {
        match event {
            DictationEvent::AutoStopped(samples) => self.handle_dictation_auto_stop(samples),
            DictationEvent::CaptureError(err) => self.handle_dictation_capture_error(err),
        }
    }

    #[cfg(all(test, feature = "dictation"))]
    pub(super) fn last_dictation_audio_len(&self) -> Option<usize> {
        self.last_dictation_audio.as_ref().map(Vec::len)
    }

    // --- Recording lifecycle ---

    pub(super) fn start_dictation_recording(&mut self) {
        if self.active_turn_id.is_some() {
            self.set_status("dictation unavailable while turn is active");
            return;
        }
        match self.dictation.start_recording() {
            Ok(()) => self.start_dictation_capture(),
            Err(err) => self.set_status(err),
        }
    }

    pub(super) fn stop_dictation_recording(&mut self) {
        if !self.dictation.stop_recording() {
            return;
        }
        self.finish_dictation_capture();
    }

    pub(super) fn cancel_dictation(&mut self) {
        if self.dictation.cancel() {
            #[cfg(feature = "dictation")]
            if let Some(session) = self.dictation_capture.take() {
                session.cancel();
            }
            self.set_status("dictation cancelled");
        }
    }

    pub(super) fn restart_dictation_recording(&mut self) {
        self.cancel_dictation();
        self.start_dictation_recording();
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

impl AppState {
    #[cfg(feature = "dictation")]
    fn start_dictation_capture(&mut self) {
        self.last_dictation_audio = None;
        let Some(tx) = self.dictation_events_tx.clone() else {
            #[cfg(test)]
            {
                self.set_status("dictation recording");
                return;
            }
            #[cfg(not(test))]
            {
                self.dictation.cancel();
                self.set_status("dictation unavailable: event channel is not ready");
                return;
            }
        };
        match DictationCaptureSession::start_default_input(tx) {
            Ok(session) => {
                self.dictation_capture = Some(session);
                self.set_status("dictation recording");
            }
            Err(err) => {
                self.dictation.cancel();
                self.set_status(format!("dictation capture unavailable: {err}"));
            }
        }
    }

    #[cfg(not(feature = "dictation"))]
    fn start_dictation_capture(&mut self) {
        self.set_status("dictation recording");
    }

    #[cfg(feature = "dictation")]
    fn finish_dictation_capture(&mut self) {
        let Some(session) = self.dictation_capture.take() else {
            self.set_status("dictation transcribing");
            return;
        };
        match session.finish() {
            Ok(samples) => self.store_recorded_dictation(samples),
            Err(err) => {
                self.dictation.cancel();
                self.set_status(format!("dictation capture failed: {err}"));
            }
        }
    }

    #[cfg(not(feature = "dictation"))]
    fn finish_dictation_capture(&mut self) {
        self.set_status("dictation transcribing");
    }

    #[cfg(feature = "dictation")]
    fn handle_dictation_auto_stop(&mut self, samples: Vec<f32>) {
        if !self.dictation.stop_recording() {
            return;
        }
        if let Some(session) = self.dictation_capture.take() {
            session.cancel();
        }
        self.store_recorded_dictation(samples);
    }

    #[cfg(feature = "dictation")]
    fn handle_dictation_capture_error(&mut self, err: String) {
        if let Some(session) = self.dictation_capture.take() {
            session.cancel();
        }
        self.dictation.cancel();
        self.set_status(format!("dictation capture failed: {err}"));
    }

    #[cfg(feature = "dictation")]
    fn store_recorded_dictation(&mut self, samples: Vec<f32>) {
        let sample_count = samples.len();
        self.last_dictation_audio = Some(samples);
        self.set_status(format!("dictation transcribing ({sample_count} samples)"));
    }
}
