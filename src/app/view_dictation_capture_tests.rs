//! Keep capture successes and errors on the local view and preserve private text.

#[cfg(feature = "dictation")]
use std::sync::Arc;

use super::{profile, view};
use crate::app::dictation_state::DictationPhase;
#[cfg(feature = "dictation")]
use crate::dictation::events::DictationEvent;

#[test]
fn missing_model_reports_the_exact_path_without_starting_recording() {
    let mut view = view();
    let mut missing = profile("en", "English");
    missing.model_label = Some("/tmp/missing-model.bin".into());
    missing.model_usable = false;
    view.configure_dictation(missing);
    view.start_dictation_recording(false);
    assert!(matches!(view.dictation_phase(), DictationPhase::Idle));
    assert_eq!(
        view.status_hint.as_deref(),
        Some("dictation model unavailable: /tmp/missing-model.bin")
    );
}

#[cfg(feature = "dictation")]
#[test]
fn matching_auto_stop_retains_audio_and_enters_transcription() {
    let mut view = view();
    view.start_dictation_recording(false);
    view.handle_dictation_event(DictationEvent::CaptureAutoStopped {
        capture_id: view.dictation_io.capture_id.expect("active capture"),
        samples: Arc::new(vec![0.1, 0.2, 0.3]),
    });
    assert!(matches!(
        view.dictation_phase(),
        DictationPhase::Transcribing { .. }
    ));
    assert_eq!(view.last_dictation_audio_len(), Some(3));
    assert_eq!(
        view.status_hint.as_deref(),
        Some("dictation transcribing (3 samples)")
    );
}

#[cfg(feature = "dictation")]
#[test]
fn matching_capture_error_returns_to_idle_and_reports_the_cause() {
    let mut view = view();
    view.input.insert_str("private draft");
    view.start_dictation_recording(false);
    view.handle_dictation_event(DictationEvent::CaptureError {
        capture_id: view.dictation_io.capture_id.expect("active capture"),
        message: "device disconnected".into(),
    });
    assert!(matches!(view.dictation_phase(), DictationPhase::Idle));
    assert_eq!(
        view.status_hint.as_deref(),
        Some("dictation capture failed: device disconnected")
    );
    assert_eq!(view.input.lines(), &["private draft"]);
}

#[test]
fn final_replaces_partial_text_and_returns_to_idle() {
    let mut view = view();
    view.start_dictation_recording(false);
    view.stop_dictation_recording();
    view.apply_dictation_partial("draft text");
    view.commit_dictation_final("final text");
    assert!(matches!(view.dictation_phase(), DictationPhase::Idle));
    assert_eq!(view.input.lines(), &["final text"]);
    assert_eq!(view.status_hint.as_deref(), Some("dictation inserted"));
}

#[test]
fn profile_picker_state_belongs_to_the_local_view() {
    let mut view = view();
    view.open_dictation_profile_picker();
    assert!(view.dictation_profile_picker_open());
    view.close_dictation_profile_picker();
    assert!(!view.dictation_profile_picker_open());
}
