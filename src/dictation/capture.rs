//! CPAL microphone capture for prompt dictation.

// --- Imports ---

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, SupportedStreamConfig};

use super::audio::{
    f32_interleaved_to_mono, i16_interleaved_to_mono, resample_to_target_rate,
    u16_interleaved_to_mono, BoundedAudioBuffer, MAX_RECORDING_SAMPLES, TARGET_SAMPLE_RATE,
};
use super::events::DictationEvent;
use super::vad::{frame_len, VadDecision, VadGate, WebRtcVad, VAD_FRAME_MS};

// --- Types ---

pub(crate) struct DictationCaptureSession {
    stream: Stream,
    state: Arc<Mutex<CaptureState>>,
    stopped: Arc<AtomicBool>,
}

struct CaptureState {
    channels: usize,
    sample_rate: u32,
    buffer: BoundedAudioBuffer,
    completed: Option<Arc<Vec<f32>>>,
    vad_pending: Vec<f32>,
    vad_frame_samples: usize,
    vad: WebRtcVad,
    gate: VadGate,
}

// --- Public API ---

impl DictationCaptureSession {
    pub(crate) fn start_default_input(tx: mpsc::Sender<DictationEvent>) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("no default input device available")?;
        let supported_config = device
            .default_input_config()
            .context("failed to read default input config")?;
        let state = Arc::new(Mutex::new(CaptureState::new(&supported_config)?));
        let stopped = Arc::new(AtomicBool::new(false));
        let stream =
            build_input_stream(device, supported_config, state.clone(), stopped.clone(), tx)?;
        stream.play().context("failed to start input stream")?;
        Ok(Self {
            stream,
            state,
            stopped,
        })
    }

    pub(crate) fn finish(self) -> Result<Vec<f32>> {
        self.stopped.store(true, Ordering::SeqCst);
        drop(self.stream);
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("dictation capture state poisoned"))?;
        state.take_audio()
    }

    pub(crate) fn cancel(self) {
        self.stopped.store(true, Ordering::SeqCst);
        drop(self.stream);
    }
}

// --- Capture State ---

impl CaptureState {
    fn new(config: &SupportedStreamConfig) -> Result<Self> {
        let channels = usize::from(config.channels());
        if channels == 0 {
            bail!("input device reports zero channels");
        }
        Ok(Self {
            channels,
            sample_rate: config.sample_rate(),
            buffer: BoundedAudioBuffer::new(MAX_RECORDING_SAMPLES),
            completed: None,
            vad_pending: Vec::new(),
            vad_frame_samples: frame_len(TARGET_SAMPLE_RATE, VAD_FRAME_MS),
            vad: WebRtcVad::new_aggressive_16khz(),
            gate: VadGate::for_dictation(),
        })
    }

    fn push_f32_interleaved(&mut self, samples: &[f32]) -> Result<Option<Arc<Vec<f32>>>> {
        let mono = f32_interleaved_to_mono(samples, self.channels)?;
        self.push_mono_samples(mono)
    }

    fn push_i16_interleaved(&mut self, samples: &[i16]) -> Result<Option<Arc<Vec<f32>>>> {
        let mono = i16_interleaved_to_mono(samples, self.channels)?;
        self.push_mono_samples(mono)
    }

    fn push_u16_interleaved(&mut self, samples: &[u16]) -> Result<Option<Arc<Vec<f32>>>> {
        let mono = u16_interleaved_to_mono(samples, self.channels)?;
        self.push_mono_samples(mono)
    }

    fn push_mono_samples(&mut self, samples: Vec<f32>) -> Result<Option<Arc<Vec<f32>>>> {
        let target_samples = resample_to_target_rate(&samples, self.sample_rate)?;
        let reached_cap = self.buffer.push_samples(&target_samples);
        self.vad_pending.extend(target_samples);
        if reached_cap || self.observe_vad_frames()? {
            return self.complete_audio().map(Some);
        }
        Ok(None)
    }

    fn observe_vad_frames(&mut self) -> Result<bool> {
        while self.vad_pending.len() >= self.vad_frame_samples {
            let is_voice = self
                .vad
                .is_voice_frame(&self.vad_pending[..self.vad_frame_samples])?;
            self.vad_pending.drain(..self.vad_frame_samples);
            if self.gate.observe(is_voice) == VadDecision::AutoStop {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn take_audio(&mut self) -> Result<Vec<f32>> {
        if let Some(samples) = self.completed.take() {
            return Ok(Arc::try_unwrap(samples).unwrap_or_else(|samples| (*samples).clone()));
        }
        let buffer = std::mem::replace(
            &mut self.buffer,
            BoundedAudioBuffer::new(MAX_RECORDING_SAMPLES),
        );
        if buffer.is_empty() {
            bail!("dictation captured no audio");
        }
        let samples = buffer.finish();
        Ok(samples)
    }

    fn complete_audio(&mut self) -> Result<Arc<Vec<f32>>> {
        if let Some(samples) = &self.completed {
            return Ok(samples.clone());
        }
        let samples = Arc::new(self.take_audio()?);
        self.completed = Some(samples.clone());
        Ok(samples)
    }
}

// --- Stream Construction ---

fn build_input_stream(
    device: cpal::Device,
    supported_config: SupportedStreamConfig,
    state: Arc<Mutex<CaptureState>>,
    stopped: Arc<AtomicBool>,
    tx: mpsc::Sender<DictationEvent>,
) -> Result<Stream> {
    let config = supported_config.config();
    match supported_config.sample_format() {
        SampleFormat::F32 => build_typed_input_stream(
            device,
            &config,
            state,
            stopped,
            tx,
            CaptureState::push_f32_interleaved,
        ),
        SampleFormat::I16 => build_typed_input_stream(
            device,
            &config,
            state,
            stopped,
            tx,
            CaptureState::push_i16_interleaved,
        ),
        SampleFormat::U16 => build_typed_input_stream(
            device,
            &config,
            state,
            stopped,
            tx,
            CaptureState::push_u16_interleaved,
        ),
        other => bail!("unsupported input sample format: {other:?}"),
    }
}

fn build_typed_input_stream<T>(
    device: cpal::Device,
    config: &cpal::StreamConfig,
    state: Arc<Mutex<CaptureState>>,
    stopped: Arc<AtomicBool>,
    tx: mpsc::Sender<DictationEvent>,
    push: fn(&mut CaptureState, &[T]) -> Result<Option<Arc<Vec<f32>>>>,
) -> Result<Stream>
where
    T: cpal::SizedSample + Send + 'static,
{
    let callback_state = state.clone();
    let callback_stopped = stopped.clone();
    let callback_tx = tx.clone();
    let data_callback = move |data: &[T], _: &cpal::InputCallbackInfo| {
        handle_input_data(data, &callback_state, &callback_stopped, &callback_tx, push);
    };
    let error_stopped = stopped;
    let error_callback = move |err| {
        if !error_stopped.swap(true, Ordering::SeqCst) {
            let _ = tx.send(DictationEvent::CaptureError(format!(
                "dictation stream error: {err}"
            )));
        }
    };
    device
        .build_input_stream(config, data_callback, error_callback, None)
        .context("failed to create input stream")
}

fn handle_input_data<T>(
    data: &[T],
    state: &Arc<Mutex<CaptureState>>,
    stopped: &Arc<AtomicBool>,
    tx: &mpsc::Sender<DictationEvent>,
    push: fn(&mut CaptureState, &[T]) -> Result<Option<Arc<Vec<f32>>>>,
) {
    if stopped.load(Ordering::SeqCst) {
        return;
    }
    let result = state
        .lock()
        .map_err(|_| anyhow::anyhow!("dictation capture state poisoned"))
        .and_then(|mut state| push(&mut state, data));
    match result {
        Ok(Some(samples)) => {
            if !stopped.swap(true, Ordering::SeqCst) {
                let _ = tx.send(DictationEvent::CaptureAutoStopped(samples));
            }
        }
        Ok(None) => {}
        Err(err) => {
            if !stopped.swap(true, Ordering::SeqCst) {
                let _ = tx.send(DictationEvent::CaptureError(err.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
