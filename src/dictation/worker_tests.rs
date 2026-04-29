use std::path::PathBuf;

use super::{
    load_model, transcribe_with_model, DictationCancelToken, DictationWorkerProfile, WorkerOutput,
};
use crate::dictation::vocabulary::{vocabulary_prompt, DEFAULT_MAX_PROMPT_CHARS};

#[test]
fn cancellation_token_tracks_cancel_state() {
    let token = DictationCancelToken::new();

    assert!(!token.is_cancelled());
    token.cancel();
    assert!(token.is_cancelled());
}

#[test]
fn worker_profile_key_tracks_model_language_and_vocabulary() {
    let profile = DictationWorkerProfile {
        id: "fr".to_string(),
        model: PathBuf::from("/models/fr.bin"),
        language: "fr".to_string(),
        vocabulary: Some(PathBuf::from("/vocab/fr.txt")),
    };

    let key = profile.key();

    assert_eq!(key.model, PathBuf::from("/models/fr.bin"));
    assert_eq!(key.language, "fr");
    assert_eq!(key.vocabulary, Some(PathBuf::from("/vocab/fr.txt")));
}

#[test]
fn default_vocabulary_prompt_is_available_for_worker_params() {
    let prompt = vocabulary_prompt("fr", None, DEFAULT_MAX_PROMPT_CHARS).unwrap();

    assert!(prompt.contains("TypeScript"));
    assert!(prompt.contains("Rust"));
}

#[test]
#[ignore = "requires CARLOS_DICTATION_TEST_MODEL and CARLOS_DICTATION_TEST_AUDIO_F32"]
fn ignored_integration_transcribes_raw_f32_fixture_with_tiny_model() {
    let model_path = std::env::var_os("CARLOS_DICTATION_TEST_MODEL")
        .map(PathBuf::from)
        .expect("set CARLOS_DICTATION_TEST_MODEL to ggml-tiny.bin");
    let audio_path = std::env::var_os("CARLOS_DICTATION_TEST_AUDIO_F32")
        .map(PathBuf::from)
        .expect("set CARLOS_DICTATION_TEST_AUDIO_F32 to raw little-endian f32 mono 16 kHz");

    let bytes = std::fs::read(audio_path).expect("read raw f32 fixture");
    let audio = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect::<Vec<_>>();

    assert!(!audio.is_empty());
    let profile = DictationWorkerProfile {
        id: "test-en".to_string(),
        model: model_path,
        language: "en".to_string(),
        vocabulary: None,
    };
    let mut model = load_model(&profile).expect("load Whisper model");
    let output = transcribe_with_model(&mut model, "en", audio, DictationCancelToken::new())
        .expect("transcribe fixture");
    let WorkerOutput::Final(text) = output else {
        panic!("fixture should not be cancelled");
    };
    assert!(!text.is_empty());
}
