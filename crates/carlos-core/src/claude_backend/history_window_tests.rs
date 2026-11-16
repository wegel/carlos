//! Reject partial or foreign archive windows before the owner can adopt any prefix.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use anyhow::Result;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};

use super::history_window::{older_window, HistoryCursor};
use crate::backend::{BackendClient, BackendKind, EventReceiver};

struct Peer {
    replies: RefCell<VecDeque<Value>>,
    calls: RefCell<Vec<Value>>,
}

impl Peer {
    fn new(replies: Vec<Value>) -> Self {
        Self {
            replies: RefCell::new(replies.into()),
            calls: RefCell::default(),
        }
    }
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }
    fn call(&self, method: &str, params: Value, _: Duration) -> Result<String> {
        assert_eq!(method, "sessions.read");
        assert_eq!(params["sessionId"], "native-session");
        assert_eq!(params["snapshotId"], "snapshot");
        self.calls.borrow_mut().push(params);
        Ok(
            json!({"result":self.replies.borrow_mut().pop_front().expect("planned archive reply")})
                .to_string(),
        )
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        panic!("read cannot grant permissions")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        panic!("read cannot answer callbacks")
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        panic!("read owns no event stream")
    }
    fn stop(&mut self) {}
}

fn cursor(end: u64) -> HistoryCursor {
    HistoryCursor {
        thread: "native-session".into(),
        snapshot: "snapshot".into(),
        end,
    }
}

fn record() -> Value {
    json!({"source":"human", "message":{"type":"user", "session_id":"native-session",
        "uuid":"native-uuid", "message":{"role":"user", "content":"exact human 🌍"},
        "opaque":{"retained":true}}})
}

fn page(offset: u64, next: Value) -> Value {
    json!({"snapshotId":"snapshot", "windowStart":0, "windowEnd":2, "olderOffset":null,
        "offset":offset, "nextOffset":next, "messages":[record()]})
}

#[test]
fn complete_chunked_window_keeps_native_records_and_original_snapshot() {
    let bytes = serde_json::to_vec(&record()).expect("native record");
    let split = bytes
        .iter()
        .position(|byte| *byte >= 128)
        .expect("Unicode byte")
        + 1;
    let make = |start: usize, end: usize, next: Value, chunk: Value| {
        json!({
            "snapshotId":"snapshot", "windowStart":0, "windowEnd":1, "olderOffset":null,
            "offset":0, "nextOffset":next, "chunkOffset":chunk, "messages":[],
            "chunk":{"offset":start, "totalBytes":bytes.len(), "data":STANDARD.encode(&bytes[start..end])}
        })
    };
    let peer = Peer::new(vec![
        make(0, split, json!(0), json!(split)),
        make(split, bytes.len(), Value::Null, Value::Null),
    ]);
    let result =
        older_window(&peer, &cursor(1), Duration::from_secs(1), 0).expect("whole native record");
    assert!(result.older.is_none());
    let facts: Vec<Value> = result
        .lines
        .iter()
        .map(|line| serde_json::from_str(line).expect("fact"))
        .collect();
    assert_eq!(facts[0]["params"]["record"], record()["message"]);
    assert_eq!(facts[1]["params"]["text"], "exact human 🌍");
    assert_eq!(peer.calls.borrow()[1]["chunkOffset"], split);
}

#[test]
fn archive_scope_holes_and_premature_exhaustion_are_read_errors() {
    let valid = page(0, json!(1));
    for (field, value) in [
        ("snapshotId", json!("foreign")),
        ("windowEnd", json!(3)),
        ("offset", json!(1)),
        ("olderOffset", json!(1)),
        ("nextOffset", Value::Null),
        ("nextOffset", json!(2)),
        ("messages", json!([])),
        ("chunkOffset", json!(1)),
    ] {
        let mut result = valid.clone();
        result[field] = value;
        let peer = Peer::new(vec![result]);
        assert!(
            older_window(&peer, &cursor(2), Duration::from_secs(1), 0).is_err(),
            "invalid {field} accepted"
        );
        assert_eq!(peer.calls.borrow().len(), 1);
    }
}

#[test]
fn a_late_foreign_snapshot_discards_the_whole_unadopted_window() {
    let mut second = page(1, Value::Null);
    second["snapshotId"] = json!("foreign");
    let peer = Peer::new(vec![page(0, json!(1)), second]);
    let error = older_window(&peer, &cursor(2), Duration::from_secs(1), 0)
        .err()
        .expect("late scope failure");
    assert!(error.to_string().contains("snapshot changed"));
    assert_eq!(peer.calls.borrow().len(), 2);
}

#[test]
fn older_prompt_reads_keep_the_original_native_byte_allowance() {
    let peer = Peer::new(vec![page(0, Value::Null)]);
    let error = older_window(
        &peer,
        &cursor(1),
        Duration::from_secs(1),
        crate::protocol_history::MAX_HISTORY_BYTES - 1,
    )
    .err()
    .expect("prior windows consumed the original request allowance");
    assert!(error.to_string().contains("64 MiB"));
    assert_eq!(peer.calls.borrow().len(), 1);
}
