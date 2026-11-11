//! Checks native item identity and ordering before the Codex backend can fork.

use std::collections::VecDeque;
use std::sync::{mpsc, Mutex};
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

use crate::backend::{BackendClient, BackendKind, RewindForkRequest, RewindUserMessage};
use crate::protocol_rewind::fork_before_edited_turn;
use crate::protocol_rewind_guard::validate_rewind;

struct NativePeer {
    replies: Mutex<VecDeque<(&'static str, Value)>>,
    calls: Mutex<Vec<(String, Value)>>,
}

impl NativePeer {
    fn new(replies: Vec<(&'static str, Value)>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn methods(&self) -> Vec<String> {
        self.calls
            .lock()
            .expect("calls")
            .iter()
            .map(|(method, _)| method.clone())
            .collect()
    }
}

impl BackendClient for NativePeer {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, method: &str, params: Value, _timeout: Duration) -> Result<String> {
        self.calls
            .lock()
            .expect("calls")
            .push((method.to_string(), params));
        let (expected, reply) = self
            .replies
            .lock()
            .expect("replies")
            .pop_front()
            .expect("expected read-only call");
        assert_eq!(method, expected);
        Ok(reply.to_string())
    }
    fn respond(&self, _id: &Value, _result: Value) -> Result<()> {
        anyhow::bail!("unused")
    }
    fn respond_error(&self, _id: &Value, _code: i64, _message: &str) -> Result<()> {
        anyhow::bail!("unused")
    }
    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        anyhow::bail!("unused")
    }
    fn stop(&mut self) {}
}

fn user(id: &str, text: &str) -> Value {
    json!({"type":"userMessage","id":id,"content":[{"type":"text","text":text}]})
}
fn page(items: Vec<Value>, cursor: Value) -> Value {
    json!({"result":{"data":items.into_iter().map(|item|json!({"turnId":"shared","item":item})).collect::<Vec<_>>(),"nextCursor":cursor}})
}
fn request(id: &str, position: usize) -> RewindForkRequest {
    RewindForkRequest {
        keep_turns: 1,
        drop_turns: 1,
        before_turn_id: Some("shared".into()),
        before_user_item: Some(RewindUserMessage {
            item_id: id.into(),
            text: "same".into(),
            position,
        }),
    }
}
fn reject(peer: &NativePeer, request: RewindForkRequest, expected: &str) {
    let error = fork_before_edited_turn(peer, "source", request, Duration::from_secs(1))
        .expect_err("must not fork");
    assert!(error.to_string().contains(expected), "{error:#}");
    assert!(!peer.methods().iter().any(|method| method == "thread/fork"
        || method == "thread/rollback"
        || method == "turn/start"));
}

#[test]
fn repeated_text_keeps_exact_ids_and_paged_overlap_does_not_hide_a_steer() {
    let first = user("first", "same");
    let peer = NativePeer::new(vec![
        (
            "thread/items/list",
            page(vec![first.clone()], json!("next")),
        ),
        (
            "thread/items/list",
            page(vec![first, user("steer", "same")], Value::Null),
        ),
    ]);
    reject(&peer, request("steer", 1), "This message is a steer");
    assert_eq!(peer.calls.lock().expect("calls")[1].1["cursor"], "next");
}

#[test]
fn an_invisible_earlier_user_still_makes_the_selected_visible_message_a_steer() {
    let peer = NativePeer::new(vec![(
        "thread/items/list",
        page(
            vec![user("empty", ""), user("selected", "same")],
            Value::Null,
        ),
    )]);
    reject(&peer, request("selected", 0), "This message is a steer");
}

#[test]
fn the_first_user_remains_editable_when_later_steers_exist() {
    let peer = NativePeer::new(vec![
        (
            "thread/items/list",
            page(
                vec![user("first", "same"), user("steer", "same")],
                Value::Null,
            ),
        ),
        (
            "thread/fork",
            json!({"result":{"thread":{"id":"fork","turns":[]}}}),
        ),
        (
            "thread/turns/list",
            json!({"result":{"data":[],"nextCursor":null}}),
        ),
    ]);
    let response =
        fork_before_edited_turn(&peer, "source", request("first", 0), Duration::from_secs(1))
            .expect("exact first-message fork");
    assert_eq!(
        serde_json::from_str::<Value>(&response).expect("response")["result"]["thread"]["id"],
        "fork"
    );
    let calls = peer.calls.lock().expect("calls");
    assert_eq!(calls[0].1["turnId"], "shared");
    assert_eq!(calls[1].1["beforeTurnId"], "shared");
    assert_eq!(calls[1].1["threadId"], "source");
}

#[test]
fn unknown_changed_or_mismatched_anchors_never_create_a_fork() {
    for (item, selected, message) in [
        (
            user("other", "same"),
            request("missing", 0),
            "missing from native history",
        ),
        (
            user("first", "changed"),
            request("first", 0),
            "changed in native history",
        ),
        (
            user("first", "same"),
            request("first", 1),
            "native position changed",
        ),
    ] {
        reject(
            &NativePeer::new(vec![("thread/items/list", page(vec![item], Value::Null))]),
            selected,
            message,
        );
    }
    let peer = NativePeer::new(Vec::new());
    let mut missing = request("first", 0);
    missing.before_user_item = None;
    reject(&peer, missing, "no exact native item ID");
    assert!(peer.methods().is_empty());
}

#[test]
fn foreign_turns_repeated_cursors_and_read_errors_fail_without_mutation() {
    let foreign = json!({"result":{"data":[{"turnId":"foreign","item":user("first","same")}],"nextCursor":null}});
    reject(
        &NativePeer::new(vec![("thread/items/list", foreign)]),
        request("first", 0),
        "different turn",
    );
    let repeating = page(Vec::new(), json!("same-cursor"));
    let peer = NativePeer::new(vec![
        ("thread/items/list", repeating.clone()),
        ("thread/items/list", repeating),
    ]);
    reject(&peer, request("first", 0), "invalid item cursor");
    let peer = NativePeer::new(vec![(
        "thread/items/list",
        json!({"error":{"code":-32000,"message":"read failed"}}),
    )]);
    reject(&peer, request("first", 0), "read failed");
}

#[test]
fn older_item_api_falls_back_to_full_paged_authoritative_history_before_validating() {
    let peer = NativePeer::new(vec![
        (
            "thread/items/list",
            json!({"error":{"code":-32601,"message":"Unknown method"}}),
        ),
        (
            "thread/read",
            json!({"result":{"thread":{"id":"source","turns":[]}}}),
        ),
        (
            "thread/turns/list",
            json!({"result":{"data":[{"id":"older","items":[]}],"nextCursor":"turn-page"}}),
        ),
        (
            "thread/turns/list",
            json!({"result":{"data":[{"id":"shared","items":[user("first","same"),user("steer","same")]}],"nextCursor":null}}),
        ),
    ]);
    reject(&peer, request("steer", 1), "This message is a steer");
}

#[test]
fn unsupported_prompt_parts_are_not_silently_lost_when_editing() {
    let mut item = user("first", "same");
    item["content"]
        .as_array_mut()
        .expect("content")
        .push(json!({"type":"image","url":"local-fixture"}));
    let peer = NativePeer::new(vec![("thread/items/list", page(vec![item], Value::Null))]);
    assert!(validate_rewind(
        &peer,
        "source",
        &request("first", 0),
        Duration::from_secs(1)
    )
    .expect_err("cannot restore image")
    .to_string()
    .contains("attachments or bindings"));
}
