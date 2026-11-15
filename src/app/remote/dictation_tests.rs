//! Check microphone shortcuts on the socket device without a native session owner.

use carlos_proto::settings::SettingsFacts;
use serde_json::json;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::dictation_state::{DictationPhase, DictationProfileState};
use crate::app::remote::widget_fixture::device;
use crate::app::remote::Device;

fn configured() -> Device {
    let mut device = device(Vec::new(), false);
    device.view.configure_dictation(DictationProfileState {
        id: "en".into(),
        name: "English".into(),
        model_label: Some("/tmp/model.bin".into()),
        model_usable: true,
        #[cfg(feature = "dictation")]
        model_path: Some("/tmp/model.bin".into()),
        #[cfg(feature = "dictation")]
        language: Some("en".into()),
        #[cfg(feature = "dictation")]
        vocabulary: None,
    });
    device
}

fn key(device: &mut Device, code: KeyCode, modifiers: KeyModifiers) {
    device
        .event(None, Event::Key(KeyEvent::new(code, modifiers)))
        .expect("local key");
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn recording_keys_stop_restart_and_cancel_without_sending_the_private_composer() {
    let mut device = configured();
    device.view.input.insert_str("unsent draft");
    key(&mut device, KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert!(device.view.dictation_recording());
    key(&mut device, KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert!(matches!(
        device.view.dictation_phase(),
        DictationPhase::Transcribing { .. }
    ));
    key(&mut device, KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert!(device.view.dictation_recording());
    key(&mut device, KeyCode::Enter, KeyModifiers::NONE);
    assert!(matches!(
        device.view.dictation_phase(),
        DictationPhase::Transcribing { .. }
    ));
    key(&mut device, KeyCode::Esc, KeyModifiers::NONE);
    assert!(matches!(
        device.view.dictation_phase(),
        DictationPhase::Idle
    ));
    assert_eq!(device.view.input.lines(), &["unsent draft"]);
    assert!(device.history.rewind_draft.is_none());
}

#[test]
fn recording_escape_cancels_before_it_can_arm_prompt_rewind() {
    let mut device = configured();
    key(&mut device, KeyCode::Char('d'), KeyModifiers::CONTROL);
    key(&mut device, KeyCode::Esc, KeyModifiers::NONE);
    assert!(matches!(
        device.view.dictation_phase(),
        DictationPhase::Idle
    ));
    assert!(device.view.esc_armed_at.is_none());
    assert!(device.history.rewind_draft.is_none());
}

#[test]
fn endpoint_key_changes_the_recording_caption_without_starting_capture() {
    let mut device = configured();
    key(&mut device, KeyCode::F(9), KeyModifiers::NONE);
    assert_eq!(
        device.view.status_hint.as_deref(),
        Some("dictation endpoint: manual (Enter to stop)")
    );
    assert!(matches!(
        device.view.dictation_phase(),
        DictationPhase::Idle
    ));
    key(&mut device, KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert_eq!(
        device.view.dictation_status_label().as_deref(),
        Some("DICTATING MANUAL [English]")
    );
    key(&mut device, KeyCode::F(9), KeyModifiers::NONE);
    assert_eq!(
        device.view.status_hint.as_deref(),
        Some("dictation endpoint: auto")
    );
    assert_eq!(
        device.view.dictation_status_label().as_deref(),
        Some("DICTATING AUTO [English]")
    );
}

#[test]
fn settings_endpoint_field_changes_only_local_dictation_and_never_starts_recording() {
    let mut device = configured();
    let facts: SettingsFacts = serde_json::from_value(json!({
        "revision":"b".repeat(64), "current":{}, "pending":{},
        "supports_effort":true, "supports_summary":true, "model_count":0,
        "native_pending":false, "legacy_overrides":false,
        "label":"fixture", "summaries_enabled":false
    }))
    .expect("plain settings facts");
    device.settings.catalog = Some(crate::app::remote::settings::Catalog {
        facts,
        models: Vec::new(),
    });
    device.view.runtime_settings.show_model_settings = true;
    device.view.runtime_settings.model_settings_field =
        crate::app::runtime_settings_state::ModelSettingsField::DictationEndpoint;
    key(&mut device, KeyCode::Right, KeyModifiers::NONE);
    assert!(device.view.runtime_settings.show_model_settings);
    assert_eq!(
        device.view.status_hint.as_deref(),
        Some("dictation endpoint: manual (Enter to stop)")
    );
    assert!(matches!(
        device.view.dictation_phase(),
        DictationPhase::Idle
    ));
    device.view.start_dictation_recording(false);
    assert_eq!(
        device.view.dictation_status_label().as_deref(),
        Some("DICTATING MANUAL [English]")
    );
}
