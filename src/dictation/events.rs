//! Typed dictation events delivered to the TUI loop.

// --- Types ---

#[derive(Debug, Clone)]
pub(crate) enum DictationEvent {
    CaptureAutoStopped(Vec<f32>),
    CaptureError(String),
    TranscriptionCancelled { request_id: u64 },
    TranscriptionError { request_id: u64, message: String },
    TranscriptionFinal { request_id: u64, text: String },
}
