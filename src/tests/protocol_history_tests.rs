//! Verify complete turn/item snapshots and bounded failure before UI replacement.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

use crate::backend::{BackendClient, BackendKind};
use crate::protocol_history::load_thread_history;

struct Peer {
    responses: RefCell<VecDeque<(&'static str, Value)>>,
    calls: RefCell<Vec<(String, Value)>>,
}

impl Peer {
    fn new(responses: Vec<(&'static str, Value)>) -> Self {
        Self {
            responses: RefCell::new(responses.into()),
            calls: RefCell::default(),
        }
    }
    fn load(&self) -> Result<Value> {
        let metadata =
            json!({"result":{"thread":{"id":"root","name":"keep","turns":[]},"model":"native"}});
        let loaded = load_thread_history(self, &metadata.to_string(), Duration::from_secs(2))?;
        Ok(serde_json::from_str(&loaded).expect("loaded JSON"))
    }
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, method: &str, params: Value, _timeout: Duration) -> Result<String> {
        self.calls.borrow_mut().push((method.into(), params));
        let (expected, response) = self
            .responses
            .borrow_mut()
            .pop_front()
            .expect("planned request");
        assert_eq!(method, expected);
        Ok(response.to_string())
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        unreachable!()
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        unreachable!()
    }
    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        unreachable!()
    }
    fn stop(&mut self) {}
}

fn page(data: Vec<Value>, next: Value) -> Value {
    json!({"result":{"data":data,"nextCursor":next}})
}

fn turn(id: &str) -> Value {
    json!({"id":id,"status":"completed","items":[],"itemsView":"notLoaded","future":"retain"})
}

fn item(turn: &str, id: &str, text: &str) -> Value {
    json!({"turnId":turn,"item":{"type":"agentMessage","id":id,"text":text,"future":42}})
}

#[test]
fn overlapping_turn_and_item_pages_keep_order_identity_and_unknown_metadata() {
    let peer = Peer::new(vec![
        (
            "thread/turns/list",
            page(vec![turn("one")], json!("turns2")),
        ),
        (
            "thread/turns/list",
            page(vec![turn("one"), turn("two")], Value::Null),
        ),
        (
            "thread/items/list",
            page(vec![item("one", "a", "partial")], json!("items2")),
        ),
        (
            "thread/items/list",
            page(
                vec![item("one", "a", "final"), item("one", "b", "next")],
                Value::Null,
            ),
        ),
        (
            "thread/items/list",
            page(
                vec![item("two", "a", "same ID in another turn")],
                Value::Null,
            ),
        ),
    ]);
    let loaded = peer.load().expect("complete snapshot");
    let thread = &loaded["result"]["thread"];
    assert_eq!(thread["name"], "keep");
    assert_eq!(loaded["result"]["model"], "native");
    assert_eq!(thread["turns"][0]["future"], "retain");
    assert_eq!(thread["turns"][0]["items"][0]["text"], "final");
    assert_eq!(thread["turns"][0]["items"][1]["id"], "b");
    assert_eq!(
        thread["turns"][1]["items"][0]["text"],
        "same ID in another turn"
    );
    assert_eq!(thread["turns"][1]["items"][0]["future"], 42);
    assert_eq!(loaded["result"]["carlosHistorySource"], "paged");
    assert_eq!(peer.calls.borrow()[0].1["itemsView"], "full");
}

#[test]
fn legacy_item_reader_uses_public_full_turn_pages_without_deprecated_resume() {
    let mut full = turn("one");
    full["items"] = json!([item("one", "a", "legacy")["item"]]);
    full["itemsView"] = json!("full");
    let peer = Peer::new(vec![
        ("thread/turns/list", page(vec![turn("one")], Value::Null)),
        (
            "thread/items/list",
            json!({"error":{"code":-32601,"message":"not supported"}}),
        ),
        ("thread/turns/list", page(vec![full], Value::Null)),
    ]);
    let loaded = peer.load().expect("legacy public page");
    assert_eq!(
        loaded["result"]["thread"]["turns"][0]["items"][0]["text"],
        "legacy"
    );
    assert_eq!(loaded["result"]["carlosHistorySource"], "legacy");
    assert_eq!(peer.calls.borrow()[2].1["itemsView"], "full");
}

#[test]
fn only_missing_public_turn_reader_enables_deprecated_full_read() {
    let peer = Peer::new(vec![
        (
            "thread/turns/list",
            json!({"error":{"code":-32601,"message":"not supported"}}),
        ),
        (
            "thread/read",
            json!({"result":{"thread":{"id":"root","name":"legacy","turns":[]}}}),
        ),
    ]);
    assert_eq!(
        peer.load().expect("old server")["result"]["thread"]["name"],
        "legacy"
    );
    assert_eq!(peer.calls.borrow()[1].1["includeTurns"], true);
    let error = Peer::new(vec![(
        "thread/turns/list",
        json!({"error":{"code":-32000,"message":"disk failed"}}),
    )]);
    assert!(error
        .load()
        .expect_err("real errors stay errors")
        .to_string()
        .contains("disk failed"));
    assert_eq!(error.calls.borrow().len(), 1);
}

#[test]
fn invalid_or_repeated_cursors_and_foreign_items_never_return_partial_history() {
    for bad in [json!({"opaque":"object"}), json!("repeat")] {
        let mut responses = vec![("thread/turns/list", page(vec![turn("one")], bad.clone()))];
        if bad.is_string() {
            responses.push(("thread/turns/list", page(vec![], bad)));
        }
        assert!(Peer::new(responses)
            .load()
            .expect_err("invalid pagination")
            .to_string()
            .contains("cursor"));
    }
    let foreign = Peer::new(vec![
        ("thread/turns/list", page(vec![turn("one")], Value::Null)),
        (
            "thread/items/list",
            page(vec![item("foreign", "x", "wrong")], Value::Null),
        ),
    ]);
    assert!(foreign
        .load()
        .expect_err("foreign owner")
        .to_string()
        .contains("another turn"));
}

#[test]
fn large_items_use_separate_pages_while_the_combined_snapshot_exceeds_one_wire_frame() {
    let text = "x".repeat(5 * 1024 * 1024);
    let peer = Peer::new(vec![
        ("thread/turns/list", page(vec![turn("one")], Value::Null)),
        (
            "thread/items/list",
            page(vec![item("one", "large1", &text)], json!("next")),
        ),
        (
            "thread/items/list",
            page(vec![item("one", "large2", &text)], Value::Null),
        ),
    ]);
    let loaded = peer.load().expect("two bounded large pages");
    assert_eq!(
        loaded["result"]["thread"]["turns"][0]["items"]
            .as_array()
            .expect("items")
            .len(),
        2
    );
}

#[test]
fn unrelated_capability_errors_never_enable_a_legacy_read_or_rewind_fallback() {
    let peer = Peer::new(vec![(
        "thread/turns/list",
        json!({"error":{"code":-32000,"message":"The current permission profile does not support reading this file"}}),
    )]);
    assert!(peer
        .load()
        .expect_err("permission failure")
        .to_string()
        .contains("permission profile"));
    assert_eq!(peer.calls.borrow().len(), 1);
}

#[test]
fn legacy_metadata_with_full_empty_turns_keeps_read_only_recovery_enabled() {
    let mut full = turn("one");
    full["itemsView"] = json!("full");
    let peer = Peer::new(vec![("thread/turns/list", page(vec![full], Value::Null))]);
    let metadata = json!({"result":{"thread":{"id":"root","historyMode":"legacy","turns":[]}}});
    let loaded = load_thread_history(&peer, &metadata.to_string(), Duration::from_secs(2))
        .expect("legacy full page");
    assert_eq!(
        serde_json::from_str::<Value>(&loaded).expect("JSON")["result"]["carlosHistorySource"],
        "legacy"
    );
}

#[test]
fn full_turn_pages_keep_complete_items_and_never_request_each_item_again() {
    let mut first = turn("one");
    first["itemsView"] = json!("full");
    first["items"] = json!([item("one", "same", "first")["item"]]);
    let mut second = turn("two");
    second["itemsView"] = json!("full");
    second["items"] = json!([item("two", "same", "second")["item"]]);
    let peer = Peer::new(vec![
        (
            "thread/turns/list",
            page(vec![first.clone()], json!("next")),
        ),
        ("thread/turns/list", page(vec![first, second], Value::Null)),
    ]);
    let loaded = peer.load().expect("full native pages");
    let turns = loaded["result"]["thread"]["turns"]
        .as_array()
        .expect("turns");
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0]["items"][0]["text"], "first");
    assert_eq!(turns[1]["items"][0]["text"], "second");
    assert_eq!(peer.calls.borrow().len(), 2);
    assert!(peer
        .calls
        .borrow()
        .iter()
        .all(|(method, params)| method == "thread/turns/list"
            && params["itemsView"] == "full"
            && params["limit"] == 100));
}

#[test]
fn batched_history_keeps_the_aggregate_cap_and_never_returns_a_partial_snapshot() {
    let mut first = turn("one");
    first["itemsView"] = json!("full");
    first["items"] = json!([item("one", "large", &"x".repeat(33 * 1024 * 1024))["item"]]);
    let mut second = first.clone();
    second["id"] = json!("two");
    let peer = Peer::new(vec![
        ("thread/turns/list", page(vec![first], json!("next"))),
        ("thread/turns/list", page(vec![second], Value::Null)),
    ]);
    let metadata = json!({"result":{"thread":{"id":"root","turns":[]}}}).to_string();
    let error =
        load_thread_history(&peer, &metadata, Duration::from_secs(10)).expect_err("aggregate cap");
    assert!(
        error.to_string().contains("history exceeds 64 MiB"),
        "{error}"
    );
    assert!(
        metadata.contains("\"turns\":[]"),
        "caller's source stays intact"
    );
}

#[test]
fn full_and_legacy_turns_reject_incomplete_or_unbounded_item_arrays() {
    for items in [
        Value::Null,
        json!({}),
        json!([{}]),
        json!([{"id":""}]),
        json!([{"id":9}]),
        json!(vec![json!({"id":"same"}); 500_001]),
    ] {
        for discriminator in [true, false] {
            let mut full = turn("one");
            if discriminator {
                full["itemsView"] = json!("full");
            } else {
                full.as_object_mut().expect("turn").remove("itemsView");
            }
            full["items"] = items.clone();
            let peer = Peer::new(vec![("thread/turns/list", page(vec![full], Value::Null))]);
            let error = peer.load().expect_err("incomplete full snapshot");
            assert!(error.to_string().contains("item"), "{error}");
        }
    }
}

#[test]
fn full_item_validation_retains_per_turn_identity_and_replaces_overlap_in_order() {
    let mut full = turn("one");
    full["itemsView"] = json!("full");
    full["items"] = json!([
        item("one", "a", "partial")["item"],
        item("one", "b", "middle")["item"],
        item("one", "a", "final")["item"]
    ]);
    let loaded = Peer::new(vec![("thread/turns/list", page(vec![full], Value::Null))])
        .load()
        .expect("valid overlap");
    let items = &loaded["result"]["thread"]["turns"][0]["items"];
    assert_eq!(items.as_array().expect("items").len(), 2);
    assert_eq!(items[0]["text"], "final");
    assert_eq!(items[1]["text"], "middle");
}

#[test]
fn native_error_message_stays_visible_without_copying_a_large_opaque_data_field() {
    let peer = Peer::new(vec![(
        "thread/turns/list",
        json!({"error":{"code":-32000,"data":"opaque".repeat(20000),"message":"History index is unavailable"}}),
    )]);
    let error = peer.load().expect_err("native failure").to_string();
    assert!(error.contains("History index is unavailable"));
    assert!(error.contains("-32000"));
    assert!(!error.contains("opaque"));
    assert!(error.len() < 256);
}

#[test]
fn reconnect_diagnostics_bound_multiline_unicode_and_terminal_controls() {
    let diagnostic = crate::protocol_events::bounded_diagnostic(&format!(
        "History failed:\n\t{}\x1b[31m",
        "日本語\n".repeat(1000)
    ));
    assert!(diagnostic.starts_with("History failed: 日本語"));
    assert!(diagnostic.len() < 550);
    assert!(!diagnostic.chars().any(char::is_control));
    assert!(diagnostic.ends_with("[details truncated]"));
    assert_eq!(
        crate::protocol_events::bounded_diagnostic("line1\nline2\x1b"),
        "line1 line2\u{fffd}"
    );
}
