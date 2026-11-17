//! Compare complete native replay with newest-first adoption at the shared client boundary.

use std::time::Duration;

use carlos_proto::bytes::ByteOffset;
use carlos_proto::prompts::{PromptDirection, PromptSource};
use carlos_proto::session::Backend;
use serde_json::{json, Value};

use super::item_history::replace_history_from_thread;
use super::state::SessionState;
use super::test_backend::TestBackend;
use crate::claude_backend::history_window::{older_window, HistoryCursor};
use crate::claude_backend::FixtureNormalizer;
use crate::protocol_history::{HistoryCursor as CodexCursor, HistoryPage as CodexPage};

fn session(backend: Backend) -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.set_client_identity(backend, "local".into());
    match backend {
        Backend::Codex => {
            session.enable_codex();
            session.enable_native_history();
        }
        Backend::Claude => session.enable_claude(),
        Backend::Grok => unreachable!("Grok has no native window reader"),
    }
    session
}

fn snapshot(session: &mut SessionState) -> Value {
    let mut value = serde_json::to_value(session.publish().snapshot(1000)).expect("snapshot");
    // Owners allocate distinct opaque IDs and publish at different times. Compare all facts.
    value["info"]["id"] = json!("logical-session");
    value
        .as_object_mut()
        .expect("snapshot object")
        .remove("seq");
    // Panel revisions include the owner's random namespace, independently of their facts.
    for panel in ["goal", "queue", "settings", "tasks"] {
        if let Some(panel) = value.get_mut(panel).and_then(Value::as_object_mut) {
            panel.remove("revision");
            panel.remove("overview_revision");
        }
    }
    let ids: Vec<_> = value["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|entry| entry["id"].clone())
        .collect();
    for (index, entry) in value["entries"]
        .as_array_mut()
        .expect("entries")
        .iter_mut()
        .enumerate()
    {
        entry["id"] = json!(index);
        if let Some(call) = entry["tool"].get_mut("call") {
            *call = json!(ids
                .iter()
                .position(|id| id == call)
                .expect("original tool call"));
        }
    }
    value
}

fn prompts(session: &SessionState, source: PromptSource) -> Vec<Value> {
    let mut selected = session
        .select_prompt(source, None, PromptDirection::Latest)
        .expect("latest");
    let mut records = Vec::new();
    while let Some(record) = selected.record {
        let text = session
            .prompt_text(&record.id, ByteOffset::new(0), 65536)
            .expect("prompt source");
        let position = record.entry.as_ref().map(|id| {
            session
                .messages()
                .iter()
                .position(|row| row.id.to_wire() == *id)
                .expect("prompt row")
        });
        records.push(
            json!({"source":source,"entry":position,"bytes":record.bytes,
            "text":text.text,"older":selected.has_older,"newer":selected.has_newer}),
        );
        if !selected.has_older {
            break;
        }
        selected = session
            .select_prompt(source, Some(&record.id), PromptDirection::Older)
            .expect("older");
    }
    records
}

fn assert_same(full: &mut SessionState, paged: &mut SessionState) {
    assert_eq!(snapshot(full), snapshot(paged));
    for source in [PromptSource::Human, PromptSource::Unknown] {
        assert_eq!(prompts(full, source), prompts(paged, source));
    }
    assert_eq!(full.turn_status_by_id(), paged.turn_status_by_id());
    let turns = |session: &SessionState| {
        (0..session.input_history().history_len())
            .map(|index| {
                session
                    .input_history()
                    .entry_id(index)
                    .and_then(|id| session.input_history().turn_id(id))
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        turns(full),
        turns(paged),
        "rewind anchors remain native turns or UUIDs"
    );
    full.messages().assert_invariants();
    paged.messages().assert_invariants();
}

fn codex_turn(index: usize) -> Value {
    let mut turn = json!({"id":format!("turn-{index}"),"status":"completed","itemsView":"full","items":[
        {"type":"userMessage","id":format!("user-{index}"),"content":[{"type":"text","text":format!("human {index}")}]},
        {"type":"userMessage","id":format!("steer-{index}"),"content":[{"type":"text","text":format!("steer {index}")}]},
        {"type":"reasoning","id":format!("thinking-{index}"),"summary":["visible thought"]},
        {"type":"commandExecution","id":format!("tool-{index}"),"command":"printf hello","status":"completed","exitCode":0,"aggregatedOutput":"hello\nworld"},
        {"type":"fileChange","id":format!("patch-{index}"),"status":"completed","changes":[{"path":"hello.rs","kind":{"type":"update"},"diff":"@@ -1 +1 @@\n-old\n+new"}]},
        {"type":"collabAgentToolCall","id":format!("report-{index}"),"agentsStates":{
            "same-child":{"status":"completed","message":format!("report {index}")}}},
        {"type":"agentMessage","id":format!("answer-{index}"),"text":format!("answer {index}")}
    ]});
    if index < 2 {
        turn["items"][5]["agentsStates"]["only-older"] =
            json!({"status":"completed","message":format!("older report {index}")});
    }
    turn
}

#[test]
fn codex_complete_and_windowed_history_export_the_same_session_facts() {
    let turns = vec![codex_turn(0), codex_turn(1), codex_turn(2)];
    let mut full = session(Backend::Codex);
    replace_history_from_thread(&mut full, &json!({"id":"root","turns":turns}))
        .expect("full replay");
    let mut paged = session(Backend::Codex);
    let newest = json!({"id":"root","turns":[turns[2]]});
    replace_history_from_thread(&mut paged, &newest).expect("newest replay");
    let cursor = CodexCursor {
        thread: "root".into(),
        next: "older".into(),
    };
    paged.install_native_history(&newest, Some((cursor.clone(), vec!["older".into()])));
    paged
        .accept_older_native_history(
            &cursor,
            CodexPage {
                thread: json!({"id":"root","turns":turns[..2]}),
                older: None,
                visited: Vec::new(),
                read_bytes: 0,
            },
        )
        .expect("whole prefix");
    assert_same(&mut full, &mut paged);
    assert_eq!(full.codex_panel.reports, paged.codex_panel.reports);
    assert_eq!(
        paged.codex_panel.reports["same-child"]["message"],
        "report 2"
    );
    assert_eq!(
        paged.codex_panel.reports["only-older"]["message"],
        "older report 1"
    );
}

fn claude_records(index: usize) -> Vec<Value> {
    vec![
        json!({"source":"human","inputKind":"steer","message":{"type":"user","session_id":"root","uuid":format!("human-{index}"),"message":{"role":"user","content":format!("human {index}")}}}),
        json!({"source":"unknown","message":{"type":"user","session_id":"root","uuid":format!("unknown-{index}"),"message":{"role":"user","content":format!("unknown {index}")}}}),
        json!({"source":"system","message":{"type":"assistant","session_id":"root","uuid":format!("call-{index}"),"message":{"role":"assistant","content":[
            {"type":"thinking","thinking":"visible thinking"},
            {"type":"tool_use","id":format!("tool-{index}"),"name":"Bash","input":{"command":"printf hello"}}
        ]}}}),
        json!({"source":"system","message":{"type":"user","session_id":"root","uuid":format!("result-{index}"),"message":{"role":"user","content":[
            {"type":"tool_result","tool_use_id":format!("tool-{index}"),"content":"hello\nworld","is_error":false}
        ]}}}),
        json!({"source":"system","message":{"type":"assistant","session_id":"root","uuid":format!("answer-{index}"),"message":{"role":"assistant","content":[{"type":"text","text":format!("answer {index}")}]}}}),
    ]
}

fn replay_claude(session: &mut SessionState, records: &[Value]) {
    let mut decoder = FixtureNormalizer::default();
    let envelope = json!({"method":"claude/controller","params":{"requestMethod":"sessions.read",
        "frame":{"type":"reply","ok":true,"result":{"messages":records}}}});
    for line in decoder.lines(&envelope, "root", session.claude().generation()) {
        let value: Value = serde_json::from_str(&line).expect("normalized fact");
        if value["method"] != "claude/controlReply" {
            session.receive_claude_message(
                value["method"].as_str().expect("method"),
                &value["params"],
            );
        }
    }
}

#[test]
fn claude_complete_and_windowed_history_export_the_same_session_facts() {
    let older = claude_records(0);
    let newer = claude_records(1);
    let mut full = session(Backend::Claude);
    replay_claude(&mut full, &[older.clone(), newer.clone()].concat());
    let mut paged = session(Backend::Claude);
    replay_claude(&mut paged, &newer);
    paged
        .claude_history_control(
            "sessions.read",
            &json!({"windowStart":5,"windowEnd":10,
        "olderOffset":5,"nextOffset":null,"snapshotId":"snapshot"}),
        )
        .expect("newest window");
    let peer = TestBackend::default();
    peer.reads.lock().expect("replies").push_back((
        "sessions.read".into(),
        json!({"result":{
        "snapshotId":"snapshot","windowStart":0,"windowEnd":5,"olderOffset":null,
        "offset":0,"nextOffset":null,"messages":older}}),
    ));
    let cursor = HistoryCursor {
        thread: "root".into(),
        snapshot: "snapshot".into(),
        end: 5,
    };
    let page =
        older_window(&peer, &cursor, Duration::from_secs(1), 0).expect("real native decoder");
    paged
        .accept_older_claude_history(&cursor, page)
        .expect("prefix adoption");
    assert_same(&mut full, &mut paged);
    assert_eq!(
        full.claude_history().records(),
        paged.claude_history().records()
    );
}
