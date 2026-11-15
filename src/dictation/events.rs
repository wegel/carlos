//! Typed dictation events delivered to the TUI loop.

use std::sync::Arc;

// --- Types ---

/// Identify the recording or transcription request that produced a local result.
#[derive(Debug)]
pub(crate) enum DictationEvent {
    CaptureAutoStopped {
        capture_id: u64,
        samples: Arc<Vec<f32>>,
    },
    CaptureError {
        capture_id: u64,
        message: String,
    },
    TranscriptionCancelled {
        request_id: u64,
    },
    TranscriptionError {
        request_id: u64,
        message: String,
    },
    TranscriptionFinal {
        request_id: u64,
        text: String,
    },
}
