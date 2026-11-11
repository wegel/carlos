//! Verify native queue identity, freshness, draft recovery and human priority without model calls.

use serde_json::{json, Value};

use super::codex_question_support::{app, receive, Peer};
use crate::app::codex_queue::{allows_automatic, begin_edit, operate, save_edit, submit, tick};
use crate::app::codex_queue_state::editable_text;

fn entry(id: &str, text: &str) -> Value {
    json!({"id":id,"clientUserMessageId":format!("client-{id}"),"input":[{"type":"text","text":text,"text_elements":[]}]})
}

fn reply(peer: &Peer, index: usize, response: Value) -> Value {
    let controls = peer.controls.borrow();
    let (method, mut params) = controls[index].clone();
    if matches!(method.as_str(), "thread/queue/add" | "thread/queue/update") {
        params.as_object_mut().expect("params").remove("input");
    }
    json!({"method":"carlos/codex/controlReply","params":{"requestMethod":method,
        "requestParams":params,"requestContext":peer.contexts.borrow()[index],"response":response}})
}

#[test]
fn queued_human_enters_history_only_after_native_consumption_and_duplicate_sends_are_blocked() {
    let mut app = app();
    let peer = Peer::default();
    app.set_input_text("queued 日本語");
    submit(&peer, &mut app, "queued 日本語");
    assert!(app.input_is_empty());
    assert!(app.messages.is_empty());
    assert_eq!(app.submitted_turn_count(), 0);
    submit(&peer, &mut app, "must not duplicate");
    assert_eq!(peer.controls.borrow().len(), 1);
    let params = peer.controls.borrow()[0].1.clone();
    let native = json!({"id":"native-queue-1","clientUserMessageId":params["clientUserMessageId"],"input":params["input"]});
    receive(
        &mut app,
        reply(&peer, 0, json!({"result":{"queuedSubmission":native}})),
    );
    assert!(app.messages.is_empty());
    receive(
        &mut app,
        json!({"method":"item/completed","params":{"threadId":"root","turnId":"consumed-turn", "item":{"id":"native-user-item","clientId":params["clientUserMessageId"],"type":"userMessage","content":params["input"]}}}),
    );
    assert_eq!(
        app.messages
            .iter()
            .filter(|message| message.text == "queued 日本語")
            .count(),
        1
    );
    assert_eq!(app.submitted_turn_count(), 1);
}

#[test]
fn stale_and_old_epoch_pages_cannot_authorize_automatic_work_or_replace_a_complete_snapshot() {
    let mut app = app();
    let peer = Peer::default();
    assert!(!allows_automatic(&app));
    tick(&peer, &mut app);
    app.codex.queue.invalidate();
    receive(
        &mut app,
        reply(&peer, 0, json!({"result":{"data":[],"nextCursor":null}})),
    );
    assert!(!app.codex.queue.known);
    assert!(app.codex.queue.read.is_none());
    tick(&peer, &mut app);
    let old = reply(
        &peer,
        1,
        json!({"result":{"data":[entry("old", "old queued")],"nextCursor":null}}),
    );
    app.set_thread_id("forked");
    receive(&mut app, old);
    assert!(app.codex.queue.entries.is_empty());
    tick(&peer, &mut app);
    receive(
        &mut app,
        reply(&peer, 2, json!({"result":{"data":[],"nextCursor":null}})),
    );
    assert!(allows_automatic(&app));
    app.codex.queue.entries.push(entry("human", "human first"));
    assert!(!allows_automatic(&app));
}

#[test]
fn queue_failure_and_eof_preserve_main_and_submitted_drafts_without_a_retry() {
    let mut app = app();
    let peer = Peer::default();
    app.set_input_text("submitted first");
    submit(&peer, &mut app, "submitted first");
    app.set_input_text("new composer");
    receive(
        &mut app,
        reply(
            &peer,
            0,
            json!({"error":{"code":-32000,"message":"queue rejected"}}),
        ),
    );
    assert_eq!(app.input_text(), "new composer");
    assert_eq!(app.codex.queue.recovery.as_deref(), Some("submitted first"));
    submit(
        &peer,
        &mut app,
        "a second failed send must not replace recovery",
    );
    assert_eq!(app.codex.queue.recovery.as_deref(), Some("submitted first"));
    assert_eq!(peer.controls.borrow().len(), 1);
    assert_eq!(peer.controls.borrow().len(), 1);
    assert!(app
        .codex
        .queue
        .error
        .as_deref()
        .expect("visible error")
        .contains("recover"));
    app.codex.queue.recovery = None;
    app.clear_input();
    submit(&peer, &mut app, "keep after EOF");
    receive(
        &mut app,
        json!({"method":"carlos/codex/closed","params":{"message":"native EOF"}}),
    );
    assert_eq!(app.input_text(), "keep after EOF");
    assert!(app.codex.closed);
    assert!(app.codex.queue.operation.is_none());
}

#[test]
fn attachments_and_text_spans_are_never_lost_by_the_plain_text_editor() {
    let mut app = app();
    let peer = Peer::default();
    let mut attached = entry("attached", "text");
    attached["input"]
        .as_array_mut()
        .expect("inputs")
        .push(json!({"type":"image","url":"https://example.test/image"}));
    let mut spans = entry("span", "token");
    spans["input"][0]["text_elements"] =
        json!([{"byteRange":{"start":0,"end":5},"placeholder":"token"}]);
    for native in [attached, spans] {
        assert!(editable_text(&native).is_none());
        app.codex.queue.entries = vec![native.clone()];
        app.codex.queue.selected = Some(native["id"].as_str().expect("ID").into());
        app.codex.queue.known = true;
        begin_edit(&mut app);
        assert!(app.codex.queue.edit.is_none());
        assert_eq!(app.codex.queue.entries[0], native);
        operate(&peer, &mut app, "start");
        assert_eq!(
            peer.controls.borrow().last().expect("start").0,
            "thread/queue/start"
        );
        app.codex.queue.operation = None;
    }
}

#[test]
fn an_external_edit_or_consumption_preserves_the_draft_and_does_not_retarget_selection() {
    let mut app = app();
    let peer = Peer::default();
    app.codex.queue.entries = vec![entry("a", "first"), entry("b", "second")];
    app.codex.queue.selected = Some("a".into());
    app.codex.queue.known = true;
    begin_edit(&mut app);
    app.codex
        .queue
        .edit
        .as_mut()
        .expect("edit")
        .editor
        .insert_str("draft");
    app.codex.queue.entries[0] = entry("a", "external new value");
    save_edit(&peer, &mut app);
    assert!(peer.controls.borrow().is_empty());
    assert!(app.codex.queue.edit.is_some());
    app.codex.queue.entries.remove(0);
    operate(&peer, &mut app, "delete");
    assert!(peer.controls.borrow().is_empty());
    assert_eq!(app.codex.queue.selected.as_deref(), Some("a"));
    app.codex.queue.move_selection(1);
    operate(&peer, &mut app, "delete");
    assert_eq!(peer.controls.borrow()[0].1["queuedSubmissionId"], "b");
}

#[test]
fn invalid_queue_pages_keep_automatic_work_blocked_instead_of_installing_partial_history() {
    let mut app = app();
    let peer = Peer::default();
    tick(&peer, &mut app);
    let action = receive(
        &mut app,
        reply(
            &peer,
            0,
            json!({"result":{"data":[entry("a", "first")],"nextCursor":"next"}}),
        ),
    );
    assert!(action.is_some());
    assert!(!app.codex.queue.known);
    crate::app::codex_queue::page(&peer, &mut app, 1, json!("next"));
    receive(
        &mut app,
        reply(
            &peer,
            1,
            json!({"result":{"data":[entry("a", "first")],"nextCursor":"next"}}),
        ),
    );
    assert!(!allows_automatic(&app));
    assert!(app.codex.queue.entries.is_empty());
    assert!(app.codex.queue.error.is_some());
}

#[test]
fn a_fork_cannot_discard_an_unsettled_queue_submission_or_an_unrecovered_draft() {
    let mut app = app();
    let peer = Peer::default();
    submit(&peer, &mut app, "pending queue input");
    app.set_input_text("edited previous prompt");
    crate::app::turn_submit::submit_rewind_turn_text(
        &peer,
        &mut app,
        0,
        "edited previous prompt".into(),
    );
    assert_eq!(app.thread_id, "root");
    assert_eq!(app.input_text(), "edited previous prompt");
    assert_eq!(peer.controls.borrow().len(), 1);
    assert!(app.codex.queue.operation.is_some());
    app.codex.queue.operation = None;
    app.codex.queue.recovery = Some("unrecovered input".into());
    assert!(crate::app::codex_queue::reject_switch(&mut app));
    assert_eq!(
        app.codex.queue.recovery.as_deref(),
        Some("unrecovered input")
    );
}

#[test]
fn later_queue_pages_use_the_remaining_whole_snapshot_deadline() {
    let mut app = app();
    let peer = Peer::default();
    tick(&peer, &mut app);
    let read = app.codex.queue.read.as_mut().expect("snapshot");
    read.started = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(19))
        .expect("prior instant");
    crate::app::codex_queue::page(&peer, &mut app, 1, json!("next-page"));
    assert!(
        peer.contexts.borrow()[1]["timeoutMs"]
            .as_u64()
            .expect("remaining milliseconds")
            <= 1000
    );
    app.codex.queue.read.as_mut().expect("snapshot").started = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(21))
        .expect("expired instant");
    crate::app::codex_queue::page(&peer, &mut app, 1, json!("last-page"));
    assert!(app.codex.queue.read.is_none());
    assert_eq!(
        peer.controls.borrow().len(),
        2,
        "expired snapshot sends no further page"
    );
}
