//! Typed dictation events delivered to the TUI loop.

use std::sync::Arc;

// --- Types ---

#[derive(Debug)]
pub(crate) enum DictationEvent {
    CaptureAutoStopped(Arc<Vec<f32>>),
    CaptureError(String),
    TranscriptionCancelled { request_id: u64 },
    TranscriptionError { request_id: u64, message: String },
    TranscriptionFinal { request_id: u64, text: String },
}
