//! Single-threaded Whisper transcription worker.

// --- Imports ---

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::events::DictationEvent;
use super::vocabulary::{vocabulary_prompt, DEFAULT_MAX_PROMPT_CHARS};

// --- Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DictationWorkerProfile {
    pub(crate) id: String,
    pub(crate) model: PathBuf,
    pub(crate) language: String,
    pub(crate) vocabulary: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub(crate) struct DictationCancelToken {
    cancelled: Arc<AtomicBool>,
}

pub(crate) struct DictationWorker {
    tx: mpsc::Sender<WorkerCommand>,
}

enum WorkerCommand {
    Transcribe {
        request_id: u64,
        profile: DictationWorkerProfile,
        audio: Vec<f32>,
        cancel: DictationCancelToken,
    },
    Shutdown,
}

struct LoadedModel {
    key: LoadedModelKey,
    ctx: WhisperContext,
    vocabulary_prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedModelKey {
    model: PathBuf,
    language: String,
    vocabulary: Option<PathBuf>,
}

// --- Public API ---

impl DictationCancelToken {
    pub(crate) fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl DictationWorker {
    pub(crate) fn spawn(ui_tx: mpsc::Sender<DictationEvent>) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_worker(rx, ui_tx));
        Self { tx }
    }

    pub(crate) fn transcribe(
        &self,
        request_id: u64,
        profile: DictationWorkerProfile,
        audio: Vec<f32>,
        cancel: DictationCancelToken,
    ) -> Result<()> {
        self.tx
            .send(WorkerCommand::Transcribe {
                request_id,
                profile,
                audio,
                cancel,
            })
            .context("dictation worker is not running")
    }
}

impl Drop for DictationWorker {
    fn drop(&mut self) {
        let _ = self.tx.send(WorkerCommand::Shutdown);
    }
}

impl DictationWorkerProfile {
    fn key(&self) -> LoadedModelKey {
        LoadedModelKey {
            model: self.model.clone(),
            language: self.language.clone(),
            vocabulary: self.vocabulary.clone(),
        }
    }
}

pub(crate) fn whisper_params<'a>(
    language: &'a str,
    initial_prompt: Option<&str>,
    cancel: DictationCancelToken,
) -> FullParams<'a, 'static> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(language));
    params.set_detect_language(false);
    params.set_translate(false);
    params.set_no_context(true);
    params.set_no_timestamps(true);
    params.set_single_segment(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_token_timestamps(false);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    if let Some(prompt) = initial_prompt.filter(|prompt| !prompt.is_empty()) {
        params.set_initial_prompt(prompt);
    }
    let abort: Box<dyn FnMut() -> bool> = Box::new(move || cancel.is_cancelled());
    params.set_abort_callback_safe::<_, Box<dyn FnMut() -> bool>>(Some(abort));
    params
}

// --- Worker Loop ---

fn run_worker(rx: mpsc::Receiver<WorkerCommand>, ui_tx: mpsc::Sender<DictationEvent>) {
    let mut loaded_model: Option<LoadedModel> = None;
    while let Ok(command) = rx.recv() {
        match command {
            WorkerCommand::Transcribe {
                request_id,
                profile,
                audio,
                cancel,
            } => {
                handle_transcription(
                    request_id,
                    profile,
                    audio,
                    cancel,
                    &mut loaded_model,
                    &ui_tx,
                );
            }
            WorkerCommand::Shutdown => break,
        }
    }
}

fn handle_transcription(
    request_id: u64,
    profile: DictationWorkerProfile,
    audio: Vec<f32>,
    cancel: DictationCancelToken,
    loaded_model: &mut Option<LoadedModel>,
    ui_tx: &mpsc::Sender<DictationEvent>,
) {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if cancel.is_cancelled() {
            return Ok(WorkerOutput::Cancelled);
        }
        let model = load_model_if_needed(loaded_model, &profile)?;
        transcribe_with_model(model, &profile.language, audio, cancel)
    }));

    let event = match result {
        Ok(Ok(WorkerOutput::Final(text))) => {
            DictationEvent::TranscriptionFinal { request_id, text }
        }
        Ok(Ok(WorkerOutput::Cancelled)) => DictationEvent::TranscriptionCancelled { request_id },
        Ok(Err(err)) => DictationEvent::TranscriptionError {
            request_id,
            message: err.to_string(),
        },
        Err(_) => DictationEvent::TranscriptionError {
            request_id,
            message: "Whisper inference panicked".to_string(),
        },
    };
    let _ = ui_tx.send(event);
}

enum WorkerOutput {
    Cancelled,
    Final(String),
}

fn load_model_if_needed<'a>(
    loaded_model: &'a mut Option<LoadedModel>,
    profile: &DictationWorkerProfile,
) -> Result<&'a mut LoadedModel> {
    let key = profile.key();
    if loaded_model.as_ref().map(|loaded| &loaded.key) != Some(&key) {
        *loaded_model = Some(load_model(profile)?);
    }
    Ok(loaded_model.as_mut().expect("loaded model was just set"))
}

fn load_model(profile: &DictationWorkerProfile) -> Result<LoadedModel> {
    let ctx = WhisperContext::new_with_params(&profile.model, WhisperContextParameters::default())
        .with_context(|| format!("failed to load Whisper model {}", profile.model.display()))?;
    let vocabulary_prompt = vocabulary_prompt(
        &profile.language,
        profile.vocabulary.as_deref(),
        DEFAULT_MAX_PROMPT_CHARS,
    )
    .context("failed to load dictation vocabulary")?;
    let vocabulary_prompt = (!vocabulary_prompt.is_empty()).then_some(vocabulary_prompt);
    Ok(LoadedModel {
        key: profile.key(),
        ctx,
        vocabulary_prompt,
    })
}

fn transcribe_with_model(
    model: &mut LoadedModel,
    language: &str,
    audio: Vec<f32>,
    cancel: DictationCancelToken,
) -> Result<WorkerOutput> {
    if cancel.is_cancelled() {
        return Ok(WorkerOutput::Cancelled);
    }
    let prompt = model.vocabulary_prompt.as_deref();
    let params = whisper_params(language, prompt, cancel.clone());
    let mut state = model
        .ctx
        .create_state()
        .context("failed to create Whisper state")?;
    state
        .full(params, &audio)
        .context("Whisper inference failed")?;
    if cancel.is_cancelled() {
        return Ok(WorkerOutput::Cancelled);
    }
    Ok(WorkerOutput::Final(collect_segments(&state)))
}

fn collect_segments(state: &whisper_rs::WhisperState) -> String {
    state
        .as_iter()
        .map(|segment| segment.to_string())
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string()
}

// --- Tests ---

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
