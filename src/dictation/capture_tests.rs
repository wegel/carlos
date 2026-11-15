//! Check captured audio ownership after endpoint detection.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::{
    frame_len, BoundedAudioBuffer, CaptureState, VadGate, WebRtcVad, MAX_RECORDING_SAMPLES,
    TARGET_SAMPLE_RATE, VAD_FRAME_MS,
};

fn test_capture_state() -> CaptureState {
    CaptureState {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        buffer: BoundedAudioBuffer::new(MAX_RECORDING_SAMPLES),
        completed: None,
        vad_pending: Vec::new(),
        vad_frame_samples: frame_len(TARGET_SAMPLE_RATE, VAD_FRAME_MS),
        vad: WebRtcVad::new_aggressive_16khz(),
        gate: VadGate::for_dictation(),
        auto_stop_enabled: Arc::new(AtomicBool::new(true)),
    }
}

#[test]
fn completed_audio_can_be_taken_after_auto_stop_drains_buffer() {
    let mut state = test_capture_state();
    state.buffer.push_samples(&[0.1, 0.2, 0.3]);

    let auto_stop_samples = state.complete_audio().expect("complete audio");
    let manual_stop_samples = state.take_audio().expect("take completed audio");

    assert_eq!(auto_stop_samples.as_slice(), &[0.1, 0.2, 0.3]);
    assert_eq!(manual_stop_samples, vec![0.1, 0.2, 0.3]);
}
