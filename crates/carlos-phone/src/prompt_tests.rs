//! Preserve an explicit send mode and exact original prompt identity through the phone facade.

use serde_json::json;

use crate::test_support::{info, reply, request, subscribed, SESSION};
use crate::{PhoneClient, PhoneError, PhoneOperationState, PhonePromptReply, PhoneSendMode};

const ENTRY: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-0000000000000001";
const PROMPT: &str = "cccccccccccccccccccccccccccccccc-0000000000000002";

fn with_user() -> std::sync::Arc<PhoneClient> {
    let client = subscribed("ready");
    let frame = client.subscribe(SESSION.into()).expect("fresh user row");
    reply(
        &client,
        &frame,
        json!({"kind":"snapshot","snapshot":{
            "info":info("ready"),"seq":"2","has_older":false,"approvals":[],
            "entries":[{"id":ENTRY,"kind":"user","text":"original 🌍","done":true,"tool":null,"operation":null}]
        }}),
    );
    client
}

#[test]
fn both_send_modes_capture_the_exact_text_without_submitting_twice() {
    for (mode, policy) in [
        (PhoneSendMode::Steer, "steer"),
        (PhoneSendMode::Queue, "queue"),
    ] {
        let client = with_user();
        let send = client
            .send_prompt(SESSION.into(), "private 日本語".into(), mode)
            .expect("explicit mode");
        let sent = request(&send.frame);
        assert_eq!(sent["params"]["policy"], policy);
        assert_eq!(sent["params"]["text"], "private 日本語");
        assert_eq!(client.operations().expect("one send").len(), 1);
    }
}

#[test]
fn opening_an_editor_reads_the_exact_record_and_text_without_starting_work() {
    let client = with_user();
    let selected = client
        .select_prompt_for_entry(SESSION.into(), ENTRY.into())
        .expect("choose user");
    assert_eq!(request(&selected)["params"]["direction"], "at_entry");
    let selected = reply(
        &client,
        &selected,
        json!({"session":SESSION,"source":"human",
        "record":{"id":PROMPT,"entry":ENTRY,"source":"human","bytes":"4"},"has_older":false,"has_newer":false}),
    );
    assert!(
        matches!(&selected.prompts[0], PhonePromptReply::Selected {prompt:Some(prompt),..}
        if prompt.id == PROMPT && prompt.entry.as_deref() == Some(ENTRY))
    );
    let text = client
        .read_prompt_text(SESSION.into(), PROMPT.into(), 0)
        .expect("read original");
    let read = reply(
        &client,
        &text,
        json!({"session":SESSION,"prompt":PROMPT,"offset":"0","total":"4","text":"🌍","next":null}),
    );
    assert!(
        matches!(&read.prompts[0], PhonePromptReply::Text {text,offset:0,next:None,..} if text == "🌍")
    );
    assert!(client.operations().expect("read-only editor").is_empty());
    assert!(client
        .select_prompt_for_entry(SESSION.into(), PROMPT.into())
        .is_err());
}

#[test]
fn a_failed_edited_branch_keeps_its_private_text_and_canonical_prompt_address() {
    let client = with_user();
    let edited = client
        .rewind_prompt(SESSION.into(), PROMPT.into(), "edited 🌍".into())
        .expect("explicit branch");
    let command = request(&edited.frame);
    assert_eq!(command["method"], "session/rewind");
    assert_eq!(command["params"]["entry"], PROMPT);
    assert_eq!(command["params"]["text"], "edited 🌍");
    client
        .write_failed(edited.frame.generation, edited.frame.request_id, true)
        .expect("uncertain write");
    let kept = client
        .operation(edited.operation.key)
        .expect("private result")
        .expect("retained edit");
    assert_eq!(kept.text.as_deref(), Some("edited 🌍"));
    assert!(matches!(kept.state, PhoneOperationState::Uncertain { .. }));
    assert!(matches!(
        client.rewind_prompt(SESSION.into(), PROMPT.into(), "retry".into()),
        Err(PhoneError::State { .. })
    ));
}
