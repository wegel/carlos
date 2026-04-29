//! Voice activity detection helpers for bounded dictation recording.

// --- Imports ---

use anyhow::{bail, Result};
use webrtc_vad::{SampleRate, Vad, VadMode};

use super::audio::{MAX_RECORDING_SAMPLES, TARGET_SAMPLE_RATE};

// --- Constants ---

pub(crate) const VAD_FRAME_MS: u32 = 20;
pub(crate) const SILENCE_AUTO_STOP_MS: u32 = 800;

// --- Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VadDecision {
    Continue,
    AutoStop,
}

#[derive(Debug, Clone)]
pub(crate) struct VadGate {
    silence_frames_to_stop: usize,
    max_frames: usize,
    total_frames: usize,
    trailing_silence_frames: usize,
    heard_voice: bool,
}

pub(crate) struct WebRtcVad {
    inner: Vad,
}

// SAFETY: WebRtcVad owns one fvad instance. The wrapper exposes only &mut self
// processing, and CaptureState keeps it behind a Mutex before moving it to CPAL.
unsafe impl Send for WebRtcVad {}

// --- Public API ---

impl VadGate {
    pub(crate) fn for_dictation() -> Self {
        let frame_samples = frame_len(TARGET_SAMPLE_RATE, VAD_FRAME_MS);
        Self::new(SILENCE_AUTO_STOP_MS, MAX_RECORDING_SAMPLES, frame_samples)
    }

    pub(crate) fn new(silence_auto_stop_ms: u32, max_samples: usize, frame_samples: usize) -> Self {
        let silence_frames_to_stop = frames_for_duration(silence_auto_stop_ms, VAD_FRAME_MS);
        let max_frames = max_samples.div_ceil(frame_samples.max(1));
        Self {
            silence_frames_to_stop,
            max_frames,
            total_frames: 0,
            trailing_silence_frames: 0,
            heard_voice: false,
        }
    }

    pub(crate) fn observe(&mut self, is_voice: bool) -> VadDecision {
        self.total_frames = self.total_frames.saturating_add(1);
        if is_voice {
            self.heard_voice = true;
            self.trailing_silence_frames = 0;
        } else if self.heard_voice {
            self.trailing_silence_frames = self.trailing_silence_frames.saturating_add(1);
        }

        if self.total_frames >= self.max_frames {
            return VadDecision::AutoStop;
        }
        if self.heard_voice && self.trailing_silence_frames >= self.silence_frames_to_stop {
            return VadDecision::AutoStop;
        }
        VadDecision::Continue
    }
}

impl WebRtcVad {
    pub(crate) fn new_aggressive_16khz() -> Self {
        Self {
            inner: Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive),
        }
    }

    pub(crate) fn is_voice_frame(&mut self, frame: &[f32]) -> Result<bool> {
        let expected = frame_len(TARGET_SAMPLE_RATE, VAD_FRAME_MS);
        if frame.len() != expected {
            bail!("VAD frame must contain {expected} samples at {TARGET_SAMPLE_RATE} Hz");
        }
        let pcm = f32_frame_to_pcm16(frame);
        self.inner
            .is_voice_segment(&pcm)
            .map_err(|()| anyhow::anyhow!("WebRTC VAD rejected audio frame"))
    }
}

pub(crate) fn frame_len(sample_rate: u32, frame_ms: u32) -> usize {
    (sample_rate as usize * frame_ms as usize) / 1000
}

pub(crate) fn f32_frame_to_pcm16(frame: &[f32]) -> Vec<i16> {
    frame.iter().map(|sample| f32_to_pcm16(*sample)).collect()
}

// --- Private Helpers ---

fn frames_for_duration(duration_ms: u32, frame_ms: u32) -> usize {
    (duration_ms as usize).div_ceil(frame_ms as usize)
}

fn f32_to_pcm16(sample: f32) -> i16 {
    let sample = sample.clamp(-1.0, 1.0);
    if sample < 0.0 {
        (sample * 32_768.0).round() as i16
    } else {
        (sample * 32_767.0).round() as i16
    }
}

// --- Tests ---

#[cfg(test)]
#[path = "vad_tests.rs"]
mod tests;
