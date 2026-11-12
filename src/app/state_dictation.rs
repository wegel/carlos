//! AppState impl: dictation profile setup, state transitions, and text commits.

#[cfg(feature = "dictation")]
use std::sync::Arc;

use super::dictation_state::DictationEndpointMode;
#[cfg(test)]
use super::dictation_state::DictationPhase;
#[cfg(any(test, feature = "dictation"))]
use super::dictation_state::DictationProfileState;
use super::dictation_state::DictationRuntimeState;
use super::state::AppState;
#[cfg(feature = "dictation")]
use crate::dictation::capture::DictationCaptureSession;
#[cfg(feature = "dictation")]
use crate::dictation::events::DictationEvent;
#[cfg(feature = "dictation")]
use crate::dictation::worker::{DictationCancelToken, DictationWorker, DictationWorkerProfile};

impl AppState {
    // --- Configuration ---

    #[cfg(test)]
    pub(super) fn configure_dictation(&mut self, profile: DictationProfileState) {
        self.view.dictation = DictationRuntimeState::with_profile(profile);
    }

    #[cfg(feature = "dictation")]
    pub(super) fn configure_dictation_profiles(
        &mut self,
        profiles: Vec<DictationProfileState>,
        active_id: &str,
    ) {
        self.view.dictation = DictationRuntimeState::with_profiles(profiles, active_id);
    }

    pub(super) fn disable_dictation(&mut self, reason: impl Into<String>) {
        self.view.dictation = DictationRuntimeState::disabled(reason);
    }

    #[cfg(test)]
    pub(super) fn dictation_phase(&self) -> &DictationPhase {
        self.view.dictation.phase()
    }

    pub(super) fn dictation_status_label(&self) -> Option<String> {
        self.view.dictation.status_label()
    }

    pub(super) fn dictation_active(&self) -> bool {
        self.view.dictation.is_active()
    }

    pub(super) fn dictation_recording(&self) -> bool {
        self.view.dictation.is_recording()
    }

    pub(super) fn dictation_endpoint_mode_label(&self) -> &str {
        self.view.dictation.endpoint_mode_label()
    }

    pub(super) fn toggle_dictation_endpoint_mode(&mut self) {
        let mode = match self.view.dictation.toggle_endpoint_mode() {
            Ok(mode) => mode,
            Err(err) => {
                self.set_hint(err);
                return;
            }
        };
        #[cfg(feature = "dictation")]
        if let Some(session) = &self.view.dictation_io.capture {
            session.set_auto_stop_enabled(mode == DictationEndpointMode::Auto);
        }
        self.set_hint(match mode {
            DictationEndpointMode::Auto => "dictation endpoint: auto",
            DictationEndpointMode::Manual => "dictation endpoint: manual (Enter to stop)",
        });
    }

    #[cfg(feature = "dictation")]
    pub(super) fn configure_dictation_event_sender(
        &mut self,
        tx: std::sync::mpsc::Sender<DictationEvent>,
    ) {
        self.view.dictation_io.worker = Some(DictationWorker::spawn(tx.clone()));
        self.view.dictation_io.events_tx = Some(tx);
    }

    #[cfg(feature = "dictation")]
    pub(super) fn handle_dictation_event(&mut self, event: DictationEvent) {
        match event {
            DictationEvent::CaptureAutoStopped(samples) => self.handle_dictation_auto_stop(samples),
            DictationEvent::CaptureError(err) => self.handle_dictation_capture_error(err),
            DictationEvent::TranscriptionCancelled { request_id } => {
                self.handle_dictation_transcription_cancelled(request_id);
            }
            DictationEvent::TranscriptionError {
                request_id,
                message,
            } => self.handle_dictation_transcription_error(request_id, message),
            DictationEvent::TranscriptionFinal { request_id, text } => {
                self.handle_dictation_transcription_final(request_id, text);
            }
        }
    }

    #[cfg(all(test, feature = "dictation"))]
    pub(super) fn last_dictation_audio_len(&self) -> Option<usize> {
        self.view.dictation_io.last_audio.as_ref().map(Vec::len)
    }

    // --- Recording lifecycle ---

    pub(super) fn start_dictation_recording(&mut self) {
        if self.session.active_turn_id().is_some() {
            self.set_hint("dictation unavailable while turn is active");
            return;
        }
        match self.view.dictation.start_recording() {
            Ok(()) => self.start_dictation_capture(),
            Err(err) => self.set_hint(err),
        }
    }

    pub(super) fn stop_dictation_recording(&mut self) {
        if !self.view.dictation.stop_recording() {
            return;
        }
        self.finish_dictation_capture();
    }

    pub(super) fn cancel_dictation(&mut self) {
        if self.view.dictation.cancel() {
            #[cfg(feature = "dictation")]
            {
                if let Some(cancel) = self.view.dictation_io.cancel.take() {
                    cancel.cancel();
                }
                self.view.dictation_io.request_id = None;
                if let Some(session) = self.view.dictation_io.capture.take() {
                    session.cancel();
                }
            }
            self.set_hint("dictation cancelled");
        }
    }

    pub(super) fn restart_dictation_recording(&mut self) {
        self.cancel_dictation();
        self.start_dictation_recording();
    }

    #[cfg(feature = "dictation")]
    pub(super) fn switch_dictation_profile_next(&mut self) {
        if self.dictation_active() {
            self.cancel_dictation();
        }
        match self.view.dictation.cycle_profile() {
            Ok(profile) => {
                let name = profile.name.clone();
                self.set_hint(format!("dictation profile: {name}"));
            }
            Err(err) => self.set_hint(format!("dictation profile unavailable: {err}")),
        }
    }

    #[cfg(not(feature = "dictation"))]
    pub(super) fn switch_dictation_profile_next(&mut self) {
        self.set_hint("dictation unavailable: rebuild with --features dictation");
    }

    #[cfg(test)]
    pub(super) fn apply_dictation_partial(&mut self, text: impl Into<String>) {
        self.view.dictation.apply_partial(text);
    }

    #[cfg(test)]
    pub(super) fn commit_dictation_final(&mut self, text: impl Into<String>) {
        if self.view.dictation.finish_transcription().is_some() {
            self.input_insert_text(text.into());
            self.set_hint("dictation inserted");
        }
    }

    // --- Profile picker ---

    #[cfg(test)]
    pub(super) fn open_dictation_profile_picker(&mut self) {
        self.view.dictation.open_picker();
    }

    #[cfg(test)]
    pub(super) fn close_dictation_profile_picker(&mut self) {
        self.view.dictation.close_picker();
    }

    #[cfg(test)]
    pub(super) fn dictation_profile_picker_open(&self) -> bool {
        self.view.dictation.picker_open()
    }
}

impl AppState {
    #[cfg(feature = "dictation")]
    fn start_dictation_capture(&mut self) {
        self.view.dictation_io.last_audio = None;
        let Some(tx) = self.view.dictation_io.events_tx.clone() else {
            #[cfg(test)]
            {
                self.set_hint("dictation recording");
                return;
            }
            #[cfg(not(test))]
            {
                self.view.dictation.cancel();
                self.set_hint("dictation unavailable: event channel is not ready");
                return;
            }
        };
        match DictationCaptureSession::start_default_input(
            tx,
            self.view.dictation.auto_endpoint_enabled(),
        ) {
            Ok(session) => {
                self.view.dictation_io.capture = Some(session);
                self.set_hint("dictation recording");
            }
            Err(err) => {
                self.view.dictation.cancel();
                self.set_hint(format!("dictation capture unavailable: {err}"));
            }
        }
    }

    #[cfg(not(feature = "dictation"))]
    fn start_dictation_capture(&mut self) {
        self.set_hint("dictation recording");
    }

    #[cfg(feature = "dictation")]
    fn finish_dictation_capture(&mut self) {
        let Some(session) = self.view.dictation_io.capture.take() else {
            self.set_hint("dictation transcribing");
            return;
        };
        match session.finish() {
            Ok(samples) => self.request_dictation_transcription(samples),
            Err(err) => {
                self.view.dictation.cancel();
                self.set_hint(format!("dictation capture failed: {err}"));
            }
        }
    }

    #[cfg(not(feature = "dictation"))]
    fn finish_dictation_capture(&mut self) {
        self.set_hint("dictation transcribing");
    }

    #[cfg(feature = "dictation")]
    fn handle_dictation_auto_stop(&mut self, samples: Arc<Vec<f32>>) {
        if !self.view.dictation.stop_recording() {
            return;
        }
        if let Some(session) = self.view.dictation_io.capture.take() {
            session.cancel();
        }
        let samples = Arc::try_unwrap(samples).unwrap_or_else(|samples| (*samples).clone());
        self.request_dictation_transcription(samples);
    }

    #[cfg(feature = "dictation")]
    fn handle_dictation_capture_error(&mut self, err: String) {
        if let Some(session) = self.view.dictation_io.capture.take() {
            session.cancel();
        }
        self.view.dictation.cancel();
        self.set_hint(format!("dictation capture failed: {err}"));
    }

    #[cfg(feature = "dictation")]
    fn request_dictation_transcription(&mut self, samples: Vec<f32>) {
        let sample_count = samples.len();
        let Some(profile) = self.worker_profile() else {
            self.view.dictation_io.last_audio = Some(samples);
            self.set_hint(format!("dictation transcribing ({sample_count} samples)"));
            return;
        };
        let Some(worker) = &self.view.dictation_io.worker else {
            self.view.dictation_io.last_audio = Some(samples);
            self.set_hint(format!("dictation transcribing ({sample_count} samples)"));
            return;
        };

        let request_id = self.view.dictation_io.next_request_id;
        self.view.dictation_io.next_request_id =
            self.view.dictation_io.next_request_id.saturating_add(1);
        let cancel = DictationCancelToken::new();
        match worker.transcribe(request_id, profile, samples, cancel.clone()) {
            Ok(()) => {
                self.view.dictation_io.cancel = Some(cancel);
                self.view.dictation_io.request_id = Some(request_id);
                self.set_hint("dictation transcribing");
            }
            Err(err) => {
                self.view.dictation.cancel();
                self.set_hint(format!("dictation worker unavailable: {err}"));
            }
        }
    }

    #[cfg(feature = "dictation")]
    fn handle_dictation_transcription_final(&mut self, request_id: u64, text: String) {
        if self.view.dictation_io.request_id != Some(request_id) {
            return;
        }
        self.view.dictation_io.cancel = None;
        self.view.dictation_io.request_id = None;
        if self.view.dictation.finish_transcription().is_some() {
            if !text.is_empty() {
                self.input_insert_text(text);
            }
            self.set_hint("dictation inserted");
        }
    }

    #[cfg(feature = "dictation")]
    fn handle_dictation_transcription_error(&mut self, request_id: u64, message: String) {
        if self.view.dictation_io.request_id != Some(request_id) {
            return;
        }
        self.view.dictation_io.cancel = None;
        self.view.dictation_io.request_id = None;
        self.view.dictation.cancel();
        self.set_hint(format!("dictation transcription failed: {message}"));
    }

    #[cfg(feature = "dictation")]
    fn handle_dictation_transcription_cancelled(&mut self, request_id: u64) {
        if self.view.dictation_io.request_id != Some(request_id) {
            return;
        }
        self.view.dictation_io.cancel = None;
        self.view.dictation_io.request_id = None;
        self.view.dictation.cancel();
        self.set_hint("dictation cancelled");
    }

    #[cfg(feature = "dictation")]
    fn worker_profile(&self) -> Option<DictationWorkerProfile> {
        let profile = self.view.dictation.profile()?;
        Some(DictationWorkerProfile {
            id: profile.id.clone(),
            model: profile.model_path.clone()?,
            language: profile.language.clone()?,
            vocabulary: profile.vocabulary.clone(),
        })
    }
}
