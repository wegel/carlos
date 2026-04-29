use super::{
    f32_frame_to_pcm16, frame_len, VadDecision, VadGate, WebRtcVad, SILENCE_AUTO_STOP_MS,
    VAD_FRAME_MS,
};
use crate::dictation::audio::{MAX_RECORDING_SAMPLES, TARGET_SAMPLE_RATE};

#[test]
fn frame_len_calculates_webrtc_supported_window() {
    assert_eq!(frame_len(16_000, 20), 320);
}

#[test]
fn pcm_conversion_clamps_to_i16_range() {
    let pcm = f32_frame_to_pcm16(&[-2.0, -1.0, 0.0, 0.5, 1.0, 2.0]);

    assert_eq!(pcm, vec![-32_768, -32_768, 0, 16_384, 32_767, 32_767]);
}

#[test]
fn vad_gate_waits_for_voice_before_silence_autostop() {
    let mut gate = VadGate::for_dictation();
    let silence_frames = SILENCE_AUTO_STOP_MS / VAD_FRAME_MS;

    for _ in 0..silence_frames {
        assert_eq!(gate.observe(false), VadDecision::Continue);
    }

    assert_eq!(gate.observe(true), VadDecision::Continue);
    for _ in 0..silence_frames - 1 {
        assert_eq!(gate.observe(false), VadDecision::Continue);
    }
    assert_eq!(gate.observe(false), VadDecision::AutoStop);
}

#[test]
fn vad_gate_autostops_at_recording_cap() {
    let frame_samples = frame_len(TARGET_SAMPLE_RATE, VAD_FRAME_MS);
    let max_frames = MAX_RECORDING_SAMPLES / frame_samples;
    let mut gate = VadGate::for_dictation();

    for _ in 0..max_frames - 1 {
        assert_eq!(gate.observe(true), VadDecision::Continue);
    }
    assert_eq!(gate.observe(true), VadDecision::AutoStop);
}

#[test]
fn webrtc_vad_accepts_silent_frame() {
    let mut vad = WebRtcVad::new_aggressive_16khz();
    let frame = vec![0.0; frame_len(TARGET_SAMPLE_RATE, VAD_FRAME_MS)];

    assert!(!vad.is_voice_frame(&frame).unwrap());
}

#[test]
fn webrtc_vad_rejects_wrong_frame_size() {
    let mut vad = WebRtcVad::new_aggressive_16khz();

    assert!(vad.is_voice_frame(&[0.0; 10]).is_err());
}
