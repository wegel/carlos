//! Audio normalization helpers for dictation capture.

// --- Imports ---

use anyhow::{bail, Context, Result};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Async, FixedAsync, PolynomialDegree, Resampler};

// --- Constants ---

pub(crate) const TARGET_SAMPLE_RATE: u32 = 16_000;
pub(crate) const MAX_RECORDING_SECONDS: usize = 30;
pub(crate) const MAX_RECORDING_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * MAX_RECORDING_SECONDS;

const RESAMPLER_CHUNK_FRAMES: usize = 1024;

// --- Types ---

#[derive(Debug, Clone)]
pub(crate) struct BoundedAudioBuffer {
    samples: Vec<f32>,
    max_samples: usize,
}

// --- Public API ---

impl BoundedAudioBuffer {
    pub(crate) fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples.min(MAX_RECORDING_SAMPLES)),
            max_samples,
        }
    }

    pub(crate) fn push_samples(&mut self, samples: &[f32]) -> bool {
        let remaining = self.max_samples.saturating_sub(self.samples.len());
        self.samples.extend(samples.iter().take(remaining).copied());
        self.samples.len() >= self.max_samples
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.samples.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub(crate) fn finish(self) -> Vec<f32> {
        self.samples
    }
}

pub(crate) fn f32_interleaved_to_mono(samples: &[f32], channels: usize) -> Result<Vec<f32>> {
    interleaved_to_mono(samples, channels, |sample| sample.clamp(-1.0, 1.0))
}

pub(crate) fn i16_interleaved_to_mono(samples: &[i16], channels: usize) -> Result<Vec<f32>> {
    interleaved_to_mono(samples, channels, |sample| {
        (sample as f32 / 32_768.0).clamp(-1.0, 1.0)
    })
}

pub(crate) fn u16_interleaved_to_mono(samples: &[u16], channels: usize) -> Result<Vec<f32>> {
    interleaved_to_mono(samples, channels, |sample| {
        ((sample as f32 - 32_768.0) / 32_768.0).clamp(-1.0, 1.0)
    })
}

pub(crate) fn resample_to_target_rate(samples: &[f32], input_sample_rate: u32) -> Result<Vec<f32>> {
    if input_sample_rate == 0 {
        bail!("input sample rate must be non-zero");
    }
    if input_sample_rate == TARGET_SAMPLE_RATE {
        return Ok(samples.to_vec());
    }
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let input_frames = samples.len();
    let ratio = TARGET_SAMPLE_RATE as f64 / input_sample_rate as f64;
    let mut resampler = Async::<f32>::new_poly(
        ratio,
        1.05,
        PolynomialDegree::Septic,
        RESAMPLER_CHUNK_FRAMES,
        1,
        FixedAsync::Input,
    )
    .context("create dictation resampler")?;

    let input = InterleavedSlice::new(samples, 1, input_frames)
        .context("prepare dictation resampler input")?;
    let output_frames = resampler.process_all_needed_output_len(input_frames);
    let mut output_samples = vec![0.0; output_frames];
    let mut output = InterleavedSlice::new_mut(&mut output_samples, 1, output_frames)
        .context("prepare dictation resampler output")?;
    let (_used_input, used_output) = resampler
        .process_all_into_buffer(&input, &mut output, input_frames, None)
        .context("resample dictation audio")?;
    output_samples.truncate(used_output);
    Ok(output_samples)
}

// --- Private Helpers ---

fn interleaved_to_mono<T>(
    samples: &[T],
    channels: usize,
    convert: impl Fn(T) -> f32,
) -> Result<Vec<f32>>
where
    T: Copy,
{
    if channels == 0 {
        bail!("audio channel count must be non-zero");
    }
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    if samples.len() % channels != 0 {
        bail!("interleaved audio does not divide evenly by channel count");
    }

    let mut mono = Vec::with_capacity(samples.len() / channels);
    for frame in samples.chunks_exact(channels) {
        let sum: f32 = frame.iter().map(|sample| convert(*sample)).sum();
        mono.push((sum / channels as f32).clamp(-1.0, 1.0));
    }
    Ok(mono)
}

// --- Tests ---

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;
