//! Keep newest-first reads complete, bounded and separate from explicit ancestry reads.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

use super::{newest_window, older_window};
use crate::backend::{BackendClient, BackendKind, EventReceiver};

struct Peer {
    replies: RefCell<VecDeque<(&'static str, Value)>>,
    calls: RefCell<Vec<(String, Value)>>,
}

impl Peer {
    fn new(replies: Vec<(&'static str, Value)>) -> Self {
        Self {
            replies: RefCell::new(replies.into()),
            calls: RefCell::default(),
        }
    }
    fn newest(&self) -> Result<super::HistoryPage> {
        newest_window(
            self,
            &json!({"result":{"thread":{"id":"root","name":"kept","turns":[]}}}).to_string(),
            Duration::from_secs(5),
        )
    }
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, method: &str, params: Value, _: Duration) -> Result<String> {
        self.calls.borrow_mut().push((method.into(), params));
        let (expected, response) = self
            .replies
            .borrow_mut()
            .pop_front()
            .expect("planned native read");
        assert_eq!(method, expected);
        Ok(response.to_string())
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        panic!("history cannot answer permissions")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        panic!("history cannot answer permissions")
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        panic!("history owns no native event stream")
    }
    fn stop(&mut self) {}
}

fn turn(id: &str) -> Value {
    json!({"id":id,"status":"completed","itemsView":"full","items":[
        {"id":"same-item-id-in-each-turn","type":"agentMessage","text":format!("answer {id}")}
    ]})
}
fn page(turns: Vec<Value>, next: Value) -> Value {
    json!({"result":{"data":turns,"nextCursor":next}})
}

#[test]
fn older_prompt_reads_keep_the_bytes_already_spent_on_earlier_windows() {
    let peer = Peer::new(vec![(
        "thread/turns/list",
        page(vec![turn("one")], Value::Null),
    )]);
    let cursor = super::HistoryCursor {
        thread: "root".into(),
        next: "older".into(),
    };
    let error = older_window(
        &peer,
        &cursor,
        Duration::from_secs(5),
        crate::protocol_history::MAX_HISTORY_BYTES - 1,
    )
    .err()
    .expect("original request byte allowance is exhausted");
    assert!(error.to_string().contains("64 MiB"));
    assert_eq!(peer.calls.borrow().len(), 1);
}

#[test]
fn newest_window_reads_once_and_older_window_uses_the_original_native_cursor() {
    let peer = Peer::new(vec![
        (
            "thread/turns/list",
            page(vec![turn("four"), turn("three")], json!("older")),
        ),
        (
            "thread/turns/list",
            page(vec![turn("two"), turn("one")], Value::Null),
        ),
    ]);
    let newest = peer.newest().expect("newest complete window");
    assert_eq!(peer.calls.borrow().len(), 1);
    assert_eq!(newest.thread["name"], "kept");
    assert_eq!(newest.thread["turns"][0]["id"], "three");
    let cursor = newest.older.expect("original older continuation");
    let older =
        older_window(&peer, &cursor, Duration::from_secs(5), 0).expect("older complete window");
    assert_eq!(older.thread["turns"][0]["id"], "one");
    assert!(older.older.is_none());
    let calls = peer.calls.borrow();
    assert_eq!(calls[0].1["sortDirection"], "desc");
    assert_eq!(calls[1].1["cursor"], "older");
    assert!(calls
        .iter()
        .all(|(_, params)| params["itemsView"] == "full" && params["limit"] == 1));
}

#[test]
fn a_summary_turn_is_hydrated_completely_before_the_newest_window_is_returned() {
    let mut summary = turn("latest");
    summary["items"] = json!([]);
    summary["itemsView"] = json!("notLoaded");
    let peer = Peer::new(vec![
        ("thread/turns/list", page(vec![summary], Value::Null)),
        (
            "thread/items/list",
            page(
                vec![
                    json!({"turnId":"latest","item":{"id":"a","type":"agentMessage","text":"full"}}),
                ],
                Value::Null,
            ),
        ),
    ]);
    let newest = peer.newest().expect("complete item body");
    assert_eq!(newest.thread["turns"][0]["items"][0]["text"], "full");
    assert_eq!(peer.calls.borrow()[1].1["turnId"], "latest");
}

#[test]
fn legacy_projections_keep_the_complete_reader_instead_of_silently_dropping_older_history() {
    let mut legacy = turn("two");
    legacy.as_object_mut().expect("turn").remove("itemsView");
    let peer = Peer::new(vec![
        ("thread/turns/list", page(vec![legacy], json!("older"))),
        (
            "thread/turns/list",
            page(vec![turn("one"), turn("two")], Value::Null),
        ),
    ]);
    let newest = peer.newest().expect("complete compatibility read");
    assert!(newest.older.is_none());
    assert_eq!(newest.thread["turns"].as_array().expect("turns").len(), 2);
    assert_eq!(peer.calls.borrow()[1].1["sortDirection"], "asc");
}

#[test]
fn invalid_items_and_repeated_native_continuations_fail_without_a_partial_page() {
    let mut invalid = turn("one");
    invalid["items"] = Value::Null;
    assert!(Peer::new(vec![(
        "thread/turns/list",
        page(vec![invalid], Value::Null)
    )])
    .newest()
    .is_err());
    let peer = Peer::new(vec![
        (
            "thread/turns/list",
            page(vec![turn("two")], json!("repeated")),
        ),
        (
            "thread/turns/list",
            page(vec![turn("one")], json!("repeated")),
        ),
    ]);
    let cursor = peer.newest().expect("newest").older.expect("cursor");
    let error = older_window(&peer, &cursor, Duration::from_secs(5), 0)
        .err()
        .expect("repeated cursor");
    assert!(error.to_string().contains("cursor"));
}
