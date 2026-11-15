//! Own microphone capture and checked local transcription events on the terminal device.

#[cfg(feature = "dictation")]
use std::sync::Arc;

use super::super::view_state::ViewState;
#[cfg(feature = "dictation")]
use crate::dictation::capture::DictationCaptureSession;
#[cfg(feature = "dictation")]
use crate::dictation::events::DictationEvent;
#[cfg(feature = "dictation")]
use crate::dictation::worker::{DictationCancelToken, DictationWorker, DictationWorkerProfile};

impl ViewState {
    #[cfg(feature = "dictation")]
    pub(in crate::app) fn configure_dictation_event_sender(
        &mut self,
        tx: std::sync::mpsc::Sender<DictationEvent>,
    ) {
        self.dictation_io.worker = Some(DictationWorker::spawn(tx.clone()));
        self.dictation_io.events_tx = Some(tx);
    }

    #[cfg(feature = "dictation")]
    pub(in crate::app) fn handle_dictation_event(&mut self, event: DictationEvent) {
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
    pub(in crate::app) fn last_dictation_audio_len(&self) -> Option<usize> {
        self.dictation_io.last_audio.as_ref().map(Vec::len)
    }
}

impl ViewState {
    #[cfg(feature = "dictation")]
    pub(super) fn start_dictation_capture(&mut self) {
        self.dictation_io.last_audio = None;
        let Some(tx) = self.dictation_io.events_tx.clone() else {
            #[cfg(test)]
            {
                self.dictation_hint("dictation recording");
                return;
            }
            #[cfg(not(test))]
            {
                self.dictation.cancel();
                self.dictation_hint("dictation unavailable: event channel is not ready");
                return;
            }
        };
        match DictationCaptureSession::start_default_input(
            tx,
            self.dictation.auto_endpoint_enabled(),
        ) {
            Ok(session) => {
                self.dictation_io.capture = Some(session);
                self.dictation_hint("dictation recording");
            }
            Err(err) => {
                self.dictation.cancel();
                self.dictation_hint(format!("dictation capture unavailable: {err}"));
            }
        }
    }

    #[cfg(not(feature = "dictation"))]
    pub(super) fn start_dictation_capture(&mut self) {
        self.dictation_hint("dictation recording");
    }

    #[cfg(feature = "dictation")]
    pub(super) fn finish_dictation_capture(&mut self) {
        let Some(session) = self.dictation_io.capture.take() else {
            self.dictation_hint("dictation transcribing");
            return;
        };
        match session.finish() {
            Ok(samples) => self.request_dictation_transcription(samples),
            Err(err) => {
                self.dictation.cancel();
                self.dictation_hint(format!("dictation capture failed: {err}"));
            }
        }
    }

    #[cfg(not(feature = "dictation"))]
    pub(super) fn finish_dictation_capture(&mut self) {
        self.dictation_hint("dictation transcribing");
    }

    #[cfg(feature = "dictation")]
    pub(super) fn handle_dictation_auto_stop(&mut self, samples: Arc<Vec<f32>>) {
        if !self.dictation.stop_recording() {
            return;
        }
        if let Some(session) = self.dictation_io.capture.take() {
            session.cancel();
        }
        let samples = Arc::try_unwrap(samples).unwrap_or_else(|samples| (*samples).clone());
        self.request_dictation_transcription(samples);
    }

    #[cfg(feature = "dictation")]
    pub(super) fn handle_dictation_capture_error(&mut self, err: String) {
        if let Some(session) = self.dictation_io.capture.take() {
            session.cancel();
        }
        self.dictation.cancel();
        self.dictation_hint(format!("dictation capture failed: {err}"));
    }

    #[cfg(feature = "dictation")]
    fn request_dictation_transcription(&mut self, samples: Vec<f32>) {
        let sample_count = samples.len();
        let Some(profile) = self.worker_profile() else {
            self.dictation_io.last_audio = Some(samples);
            self.dictation_hint(format!("dictation transcribing ({sample_count} samples)"));
            return;
        };
        let Some(worker) = &self.dictation_io.worker else {
            self.dictation_io.last_audio = Some(samples);
            self.dictation_hint(format!("dictation transcribing ({sample_count} samples)"));
            return;
        };

        let request_id = self.dictation_io.next_request_id;
        self.dictation_io.next_request_id = self.dictation_io.next_request_id.saturating_add(1);
        let cancel = DictationCancelToken::new();
        match worker.transcribe(request_id, profile, samples, cancel.clone()) {
            Ok(()) => {
                self.dictation_io.cancel = Some(cancel);
                self.dictation_io.request_id = Some(request_id);
                self.dictation_hint("dictation transcribing");
            }
            Err(err) => {
                self.dictation.cancel();
                self.dictation_hint(format!("dictation worker unavailable: {err}"));
            }
        }
    }

    #[cfg(feature = "dictation")]
    pub(super) fn handle_dictation_transcription_final(&mut self, request_id: u64, text: String) {
        if self.dictation_io.request_id != Some(request_id) {
            return;
        }
        self.dictation_io.cancel = None;
        self.dictation_io.request_id = None;
        if self.dictation.finish_transcription().is_some() {
            if !text.is_empty() {
                self.insert_dictation_text(text);
            }
            self.dictation_hint("dictation inserted");
        }
    }

    #[cfg(feature = "dictation")]
    pub(super) fn handle_dictation_transcription_error(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        if self.dictation_io.request_id != Some(request_id) {
            return;
        }
        self.dictation_io.cancel = None;
        self.dictation_io.request_id = None;
        self.dictation.cancel();
        self.dictation_hint(format!("dictation transcription failed: {message}"));
    }

    #[cfg(feature = "dictation")]
    pub(super) fn handle_dictation_transcription_cancelled(&mut self, request_id: u64) {
        if self.dictation_io.request_id != Some(request_id) {
            return;
        }
        self.dictation_io.cancel = None;
        self.dictation_io.request_id = None;
        self.dictation.cancel();
        self.dictation_hint("dictation cancelled");
    }

    #[cfg(feature = "dictation")]
    fn worker_profile(&self) -> Option<DictationWorkerProfile> {
        let profile = self.dictation.profile()?;
        Some(DictationWorkerProfile {
            id: profile.id.clone(),
            model: profile.model_path.clone()?,
            language: profile.language.clone()?,
            vocabulary: profile.vocabulary.clone(),
        })
    }
}
