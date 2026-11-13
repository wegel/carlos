//! Keeps the native 16 MiB record allowance compatible with the bounded normalized receiver.

use std::time::Duration;

use serde_json::{json, Value};

use super::event_test_support::{client, SESSION};
use super::fact_transfer::FactTransfer;
use crate::backend::BackendClient;

#[test]
fn a_history_record_larger_than_the_event_budget_remains_complete_and_paced() {
    let size = 9 * 1024 * 1024;
    let (client, receiver) = client(Vec::new(), json!({"_historyBytes":size}), 1024);
    let mut assembly = FactTransfer::default();
    let mut params = json!({"sessionId":SESSION});
    let mut records = Vec::new();
    loop {
        client
            .request_native_control("sessions.read", params.clone())
            .expect("native chunk read");
        let result = read_page(&client, &receiver, &mut assembly, &mut records);
        if result["nextOffset"].is_null() {
            break;
        }
        params["offset"] = result["nextOffset"].clone();
        params["chunkOffset"] = result["chunkOffset"].clone();
    }
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["method"], "claude/historyRecord");
    assert_eq!(
        records[0]["params"]["record"]["message"]["content"]
            .as_str()
            .expect("native content"),
        "x".repeat(size)
    );
    assert_eq!(records[1]["method"], "claude/historyPrompt");
    assert_eq!(
        records[1]["params"]["text"]
            .as_str()
            .expect("prompt text")
            .len(),
        size
    );
    assert_eq!(
        records[1]["params"]["sourceKey"],
        json!({"sessionId":SESSION,"kind":"user","id":"large-user"})
    );
    assert_eq!(client.events_tx.charged_bytes(), 0);
}

fn read_page(
    client: &super::ClaudeClient,
    receiver: &std::sync::mpsc::Receiver<String>,
    assembly: &mut FactTransfer,
    records: &mut Vec<Value>,
) -> Value {
    loop {
        let line = receiver
            .recv_timeout(Duration::from_secs(3))
            .expect("history progress");
        assert!(
            line.len() < 1024 * 1024,
            "every delivered line fits the event budget"
        );
        client.acknowledge_event(line.len());
        let value: Value = serde_json::from_str(&line).expect("normalized JSON");
        match value["method"].as_str() {
            Some("claude/historyFactPart") => {
                if let Some(record) = assembly.receive(&value["params"]).expect("normalized part") {
                    records.push(record);
                }
                assert!(
                    receiver.try_recv().is_err(),
                    "one part waits for one client request"
                );
                client
                    .request_native_control("history.continue", json!({}))
                    .expect("next normalized piece");
            }
            Some("claude/controlReply") => return value["params"]["result"].clone(),
            _ => records.push(value),
        }
    }
}
