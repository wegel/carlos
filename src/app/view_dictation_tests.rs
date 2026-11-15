//! Check shared device dictation without creating a backend or opening a microphone.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::dictation_state::{DictationPhase, DictationProfileState};
use crate::app::view_state::ViewState;
#[cfg(feature = "dictation")]
use crate::dictation::events::DictationEvent;
#[cfg(feature = "dictation")]
use crate::dictation::worker::DictationCancelToken;

fn profile(id: &str, name: &str) -> DictationProfileState {
    DictationProfileState {
        id: id.into(),
        name: name.into(),
        model_label: None,
        model_usable: true,
        #[cfg(feature = "dictation")]
        model_path: None,
        #[cfg(feature = "dictation")]
        language: None,
        #[cfg(feature = "dictation")]
        vocabulary: None,
    }
}

fn view() -> ViewState {
    let mut view = ViewState::new(false);
    view.configure_dictation(profile("en", "English"));
    view
}

fn key(view: &mut ViewState, code: KeyCode, modifiers: KeyModifiers, busy: bool) -> bool {
    view.dictation_key(KeyEvent::new(code, modifiers), busy)
}

#[test]
fn local_keys_stop_restart_and_cancel_without_submitting_private_text() {
    let mut view = view();
    view.input.insert_str("private draft");
    assert!(key(
        &mut view,
        KeyCode::Char('d'),
        KeyModifiers::CONTROL,
        false
    ));
    assert!(view.dictation_recording());
    assert!(!key(&mut view, KeyCode::Enter, KeyModifiers::SHIFT, false));
    assert!(
        view.dictation_recording(),
        "newline Enter does not stop recording"
    );
    assert!(key(&mut view, KeyCode::Enter, KeyModifiers::NONE, false));
    assert!(matches!(
        view.dictation_phase(),
        DictationPhase::Transcribing { .. }
    ));
    assert!(key(
        &mut view,
        KeyCode::Char('d'),
        KeyModifiers::CONTROL,
        false
    ));
    assert!(view.dictation_recording());
    assert!(key(&mut view, KeyCode::Esc, KeyModifiers::NONE, false));
    assert!(matches!(view.dictation_phase(), DictationPhase::Idle));
    assert!(!key(&mut view, KeyCode::Enter, KeyModifiers::NONE, false));
    assert!(!key(&mut view, KeyCode::Esc, KeyModifiers::NONE, false));
    assert_eq!(view.input.lines(), &["private draft"]);
}

#[test]
fn active_turn_guard_precedes_capture_and_preserves_the_local_draft() {
    let mut view = view();
    view.input.insert_str("unsent");
    assert!(key(
        &mut view,
        KeyCode::Char('D'),
        KeyModifiers::CONTROL,
        true
    ));
    assert!(matches!(view.dictation_phase(), DictationPhase::Idle));
    assert_eq!(
        view.status_hint.as_deref(),
        Some("dictation unavailable while turn is active")
    );
    assert_eq!(view.input.lines(), &["unsent"]);
}

#[test]
fn final_text_resets_escape_chords_and_preserves_a_rewind_draft() {
    let mut view = view();
    view.input_history.enter_rewind_mode("before rewind".into());
    view.input.insert_str("edited ");
    view.esc_armed_at = Some(Instant::now());
    view.esc_interrupted_at = Some(Instant::now());
    view.start_dictation_recording(false);
    view.stop_dictation_recording();
    view.commit_dictation_final("speech 🌍");
    assert_eq!(view.input.lines(), &["edited speech 🌍"]);
    assert!(view.esc_armed_at.is_none());
    assert!(view.esc_interrupted_at.is_none());
    assert_eq!(
        view.input_history.exit_rewind_mode_restore().as_deref(),
        Some("before rewind")
    );
}

#[cfg(feature = "dictation")]
#[test]
fn a_cancelled_request_cannot_insert_into_a_restarted_recording() {
    let mut view = view();
    view.start_dictation_recording(false);
    view.stop_dictation_recording();
    let cancel = DictationCancelToken::new();
    view.dictation_io.cancel = Some(cancel.clone());
    view.dictation_io.request_id = Some(7);
    key(&mut view, KeyCode::Char('d'), KeyModifiers::CONTROL, false);
    assert!(cancel.is_cancelled());
    view.handle_dictation_event(DictationEvent::TranscriptionFinal {
        request_id: 7,
        text: "stale".into(),
    });
    assert!(view.dictation_recording());
    assert!(view.input.is_empty());
}

#[cfg(feature = "dictation")]
#[test]
fn profile_changes_cancel_inference_and_keep_endpoint_choice() {
    let mut view = view();
    view.configure_dictation_profiles(
        vec![profile("en", "English"), profile("fr", "French")],
        "en",
    );
    key(&mut view, KeyCode::F(9), KeyModifiers::NONE, false);
    view.start_dictation_recording(false);
    view.stop_dictation_recording();
    let cancel = DictationCancelToken::new();
    view.dictation_io.cancel = Some(cancel.clone());
    view.dictation_io.request_id = Some(8);
    key(&mut view, KeyCode::F(7), KeyModifiers::NONE, false);
    assert!(cancel.is_cancelled());
    assert_eq!(view.dictation_endpoint_mode_label(), "manual");
    view.handle_dictation_event(DictationEvent::TranscriptionFinal {
        request_id: 8,
        text: "old profile".into(),
    });
    view.start_dictation_recording(false);
    assert_eq!(
        view.dictation_status_label().as_deref(),
        Some("DICTATING MANUAL [French]")
    );
    assert!(view.input.is_empty());
}

#[cfg(feature = "dictation")]
#[test]
fn local_event_drain_inserts_only_the_matching_final_into_this_view() {
    let mut view = view();
    let mut other = ViewState::new(false);
    other.input.insert_str("another private draft");
    view.input.insert_str("prefix ");
    view.start_dictation_recording(false);
    view.stop_dictation_recording();
    view.dictation_io.request_id = Some(9);
    let (tx, rx) = std::sync::mpsc::channel();
    view.dictation_io.local_events = Some(rx);
    tx.send(DictationEvent::TranscriptionFinal {
        request_id: 8,
        text: "wrong".into(),
    })
    .expect("old final");
    tx.send(DictationEvent::TranscriptionFinal {
        request_id: 9,
        text: "local 🌍".into(),
    })
    .expect("current final");
    assert!(view.drain_local_dictation_events());
    assert_eq!(view.input.lines(), &["prefix local 🌍"]);
    assert!(matches!(view.dictation_phase(), DictationPhase::Idle));
    assert!(!view.drain_local_dictation_events());
    assert_eq!(other.input.lines(), &["another private draft"]);
}

#[cfg(feature = "dictation")]
#[test]
fn local_event_drain_yields_between_bounded_batches() {
    let mut view = view();
    let (tx, rx) = std::sync::mpsc::channel();
    view.dictation_io.local_events = Some(rx);
    for index in 0..40 {
        tx.send(DictationEvent::CaptureError {
            capture_id: 0,
            message: index.to_string(),
        })
        .expect("capture error");
    }
    assert!(view.drain_local_dictation_events());
    let remaining = view
        .dictation_io
        .local_events
        .as_ref()
        .expect("local receiver")
        .try_iter()
        .count();
    assert!(
        remaining > 0 && remaining < 40,
        "one batch must leave time for terminal input"
    );
    assert!(
        view.status_hint.is_none(),
        "unowned capture errors cannot change the view"
    );
}

#[cfg(feature = "dictation")]
#[test]
fn dropping_the_device_cancels_its_inference_token() {
    let cancel = DictationCancelToken::new();
    {
        let mut view = view();
        view.start_dictation_recording(false);
        view.stop_dictation_recording();
        view.dictation_io.request_id = Some(10);
        view.dictation_io.cancel = Some(cancel.clone());
    }
    assert!(cancel.is_cancelled());
}

#[cfg(feature = "dictation")]
#[test]
fn an_old_auto_stop_cannot_stop_a_replacement_recording() {
    let mut view = view();
    view.start_dictation_recording(false);
    let old = view.dictation_io.capture_id.expect("old capture");
    view.cancel_dictation();
    view.start_dictation_recording(false);
    let replacement = view.dictation_io.capture_id.expect("replacement capture");
    assert_ne!(old, replacement);
    view.handle_dictation_event(DictationEvent::CaptureAutoStopped {
        capture_id: old,
        samples: std::sync::Arc::new(vec![0.1, 0.2]),
    });
    assert!(view.dictation_recording());
    assert_eq!(view.dictation_io.capture_id, Some(replacement));
    assert!(view.dictation_io.last_audio.is_none());
}

#[cfg(feature = "dictation")]
#[test]
fn an_old_capture_error_cannot_cancel_a_replacement_recording() {
    let mut view = view();
    view.start_dictation_recording(false);
    let old = view.dictation_io.capture_id.expect("old capture");
    view.cancel_dictation();
    view.start_dictation_recording(false);
    let replacement = view.dictation_io.capture_id.expect("replacement capture");
    view.handle_dictation_event(DictationEvent::CaptureError {
        capture_id: old,
        message: "old device".into(),
    });
    assert!(view.dictation_recording());
    assert_eq!(view.dictation_io.capture_id, Some(replacement));
    assert_eq!(view.status_hint.as_deref(), Some("dictation recording"));
}

#[cfg(feature = "dictation")]
#[test]
fn a_late_capture_error_cannot_cancel_the_matching_transcription() {
    let mut view = view();
    view.start_dictation_recording(false);
    let capture = view.dictation_io.capture_id.expect("capture");
    view.stop_dictation_recording();
    view.dictation_io.request_id = Some(11);
    view.handle_dictation_event(DictationEvent::CaptureError {
        capture_id: capture,
        message: "late device".into(),
    });
    assert!(matches!(
        view.dictation_phase(),
        DictationPhase::Transcribing { .. }
    ));
    view.handle_dictation_event(DictationEvent::TranscriptionFinal {
        request_id: 11,
        text: "current speech".into(),
    });
    assert_eq!(view.input.lines(), &["current speech"]);
}
