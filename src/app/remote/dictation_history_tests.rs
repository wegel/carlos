//! Guard private dictated text against older asynchronous prompt-history reads.

#![cfg(feature = "dictation")]

use std::sync::mpsc::{self, Sender};

use carlos_proto::bytes::ByteLength;
use carlos_proto::commands::RequestId;
use carlos_proto::prompts::{PromptRecord, PromptSelection, PromptSource};
use serde_json::json;

use crate::app::dictation_state::DictationProfileState;
use crate::app::remote::widget_fixture::device;
use crate::app::remote::Device;
use crate::dictation::events::DictationEvent;

fn waiting_prompt() -> (Device, Sender<DictationEvent>, RequestId) {
    let mut device = device(Vec::new(), false);
    device.view.configure_dictation(DictationProfileState {
        id: "en".into(),
        name: "English".into(),
        model_label: None,
        model_usable: true,
        model_path: None,
        language: None,
        vocabulary: None,
    });
    device.view.start_dictation_recording(false);
    device.view.stop_dictation_recording();
    device.view.dictation_io.request_id = Some(7);
    let (sender, events) = mpsc::channel();
    device.view.dictation_io.local_events = Some(events);
    let pending: RequestId = serde_json::from_value(json!(17)).expect("read request identity");
    device.history.pending = Some(pending.clone());
    device.history.moves.push_back(true);
    device.history.navigation_draft = Some("original composer".into());
    (device, sender, pending)
}

#[test]
fn matching_final_cancels_old_prompt_reads_without_changing_an_edit_target() {
    for rewinding in [false, true] {
        let (mut device, sender, _) = waiting_prompt();
        let selected = PromptSelection {
            session: device.session.clone(),
            source: PromptSource::Human,
            record: Some(PromptRecord {
                id: format!("{}-0000000000000001", "a".repeat(32))
                    .parse()
                    .expect("prompt identity"),
                entry: None,
                source: PromptSource::Human,
                bytes: ByteLength::new(15),
            }),
            has_older: true,
            has_newer: false,
        };
        device.history.selection = Some(selected.clone());
        if rewinding {
            device.history.rewind_draft = Some("before rewind".into());
        }
        sender
            .send(DictationEvent::TranscriptionFinal {
                request_id: 7,
                text: "dictated 日本語".into(),
            })
            .expect("matching final");
        assert!(device.drain_dictation());
        assert_eq!(device.view.input.lines(), &["dictated 日本語"]);
        assert!(
            device.history.pending.is_none(),
            "an old prompt read must not overwrite dictated text"
        );
        assert!(device.history.moves.is_empty());
        assert_eq!(
            device.history.selection.as_ref(),
            rewinding.then_some(&selected)
        );
        assert_eq!(
            device.history.rewind_draft.as_deref(),
            rewinding.then_some("before rewind")
        );
        assert_eq!(
            device.history.navigation_draft.as_deref(),
            rewinding.then_some("original composer")
        );
    }
}

#[test]
fn a_stale_final_does_not_cancel_current_prompt_navigation() {
    let (mut device, sender, pending) = waiting_prompt();
    sender
        .send(DictationEvent::TranscriptionFinal {
            request_id: 6,
            text: "stale private text".into(),
        })
        .expect("old final");
    assert!(device.drain_dictation());
    assert!(device.view.input.is_empty());
    assert_eq!(device.history.pending, Some(pending));
    assert_eq!(device.history.moves.len(), 1);
    assert_eq!(
        device.history.navigation_draft.as_deref(),
        Some("original composer")
    );
}
