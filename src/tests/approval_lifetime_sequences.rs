//! Compare every reachable card lifetime and callback kind with a HEAD-captured fixture.
// m4_approval_lifetimes.json is a frozen record of commit 2cbfedc behaviour,
// with only non-rendered status fields removed; it cannot be regenerated from
// this repository. target/m4-fix4/capture.rs used external pre-split adapters.

use crossterm::event::{Event, KeyCode, KeyModifiers};
use serde_json::{json, Value};

use super::approval_lifetime_support::{
    accept, app, callback, event, fill, key, payloads, request, server, snapshot, Client, METHODS,
};

use super::state::AppState;
use crate::backend::BackendKind;
use crate::event::UiEvent;

#[test]
fn approval_lifetimes_and_callback_guidance_match_head() {
    let actual = sequences();
    let expected: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/m4_approval_lifetimes.json"))
            .expect("HEAD approval fixtures");
    assert_eq!(actual.len(), 470, "complete consent case corpus");
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(&expected) {
        assert_eq!(actual, expected, "case {}", actual["case"]);
    }
}

pub(super) fn sequences() -> Vec<Value> {
    let mut cases = Vec::new();
    for (index, (kind, payload)) in payloads().into_iter().enumerate() {
        for default_no in [false, true] {
            for tail in [
                "switch",
                "fork",
                "native-reset",
                "clear",
                "failure-replace",
                "cancel-active",
                "cancel-queued",
                "promote",
                "history",
                "rewind-key",
                "focus",
                "arrival",
                "decline",
                "cancel",
                "other",
                "second",
                "completed",
                "failed",
                "interrupted",
            ] {
                cases.push(claude_case(index, kind, &payload, default_no, tail));
            }
        }
    }
    cases.extend(replacement_cases());
    cases.extend(server_cases());
    cases.extend(codex_guidance_cases());
    cases
}

fn claude_case(index: usize, kind: &str, payload: &Value, default_no: bool, tail: &str) -> Value {
    let client = Client::new(BackendKind::Claude);
    let mut app = app(BackendKind::Claude);
    server(
        &client,
        &mut app,
        json!({"method":"turn/started","params":{"turn":{"id":"turn"}}}),
    );
    let mut payload = payload.clone();
    payload["defaultToNo"] = json!(default_no);
    for id in ["one", "two", "three"] {
        callback(&client, &mut app, id, kind, payload.clone());
    }
    fill(&client, &mut app);
    let mut steps = vec![snapshot(&client, &app)];
    accept(&client, &mut app);
    steps.push(snapshot(&client, &app));
    claude_tail(&client, &mut app, tail);
    steps.push(snapshot(&client, &app));
    payload["input"]["command"] = json!("echo different request");
    callback(&client, &mut app, "one", kind, payload);
    fill(&client, &mut app);
    steps.push(snapshot(&client, &app));
    accept(&client, &mut app);
    steps.push(snapshot(&client, &app));
    accept(&client, &mut app);
    steps.push(snapshot(&client, &app));
    json!({"case":format!("claude/{index}/{default_no}/{tail}"),"steps":steps})
}

fn claude_tail(client: &Client, app: &mut AppState, tail: &str) {
    match tail {
        "switch" | "fork" => {
            if tail == "fork" {
                super::claude_edit::begin(app, "edited prompt".into());
            }
            switch_helper(client, app);
        }
        "native-reset" => server(
            client,
            app,
            json!({"method":"claude/conversationReset",
            "params":{"generation":1,"new_conversation_id":"reset"}}),
        ),
        "failure-replace" => {
            server(
                client,
                app,
                json!({"method":"claude/closed","params":{"generation":1,"reason":"failure"}}),
            );
            switch_helper(client, app);
        }
        "cancel-active" | "cancel-queued" => server(
            client,
            app,
            json!({"method":"claude/callbackCancelled","params":{"generation":1,
                "callbackId":if tail == "cancel-active" { "one" } else { "two" }}}),
        ),
        "promote" | "decline" => key(client, app, KeyCode::Char('n'), KeyModifiers::ALT),
        "cancel" => key(client, app, KeyCode::Char('c'), KeyModifiers::ALT),
        "history" => history(app),
        "clear" => clear(client, app),
        "rewind-key" => key(client, app, KeyCode::Esc, KeyModifiers::NONE),
        "focus" => {
            for _ in 0..3 {
                key(client, app, KeyCode::F(4), KeyModifiers::NONE);
            }
        }
        "arrival" => callback(
            client,
            app,
            "four",
            "permission",
            json!({"toolName":"Bash","defaultToNo":true}),
        ),
        "other" => key(client, app, KeyCode::Char('x'), KeyModifiers::NONE),
        "second" => accept(client, app),
        "completed" | "failed" | "interrupted" => server(
            client,
            app,
            json!({"method":"turn/completed","params":{"turn":{"id":"turn","status":tail}}}),
        ),
        _ => panic!("unknown lifetime {tail}"),
    }
    app.assert_entry_invariants();
}

fn server_case(backend: BackendKind, method: &str, tail: &str) -> Value {
    let client = Client::new(backend);
    let mut app = app(backend);
    server(
        &client,
        &mut app,
        json!({"method":"turn/started","params":{"turn":{"id":"turn"}}}),
    );
    request(&client, &mut app, method, "one");
    request(&client, &mut app, method, "two");
    fill(&client, &mut app);
    let mut steps = vec![snapshot(&client, &app)];
    accept(&client, &mut app);
    steps.push(snapshot(&client, &app));
    server_tail(&client, &mut app, tail);
    steps.push(snapshot(&client, &app));
    request(&client, &mut app, method, "one");
    fill(&client, &mut app);
    accept(&client, &mut app);
    steps.push(snapshot(&client, &app));
    json!({"case":format!("{backend:?}/{method}/{tail}"),"steps":steps})
}

fn server_tail(client: &Client, app: &mut AppState, tail: &str) {
    match tail {
        "replace" => request(client, app, METHODS[0], "replacement"),
        "fork" => app.reset_for_forked_thread("fork"),
        "history" => history(app),
        "resolve" => server(
            client,
            app,
            json!({"method":"serverRequest/resolved",
            "params":{"threadId":"thread","requestId":"two"}}),
        ),
        "promote" | "decline" => key(client, app, KeyCode::Char('n'), KeyModifiers::ALT),
        "cancel" => key(client, app, KeyCode::Char('c'), KeyModifiers::ALT),
        "completed" | "failed" | "interrupted" => server(
            client,
            app,
            json!({"method":"turn/completed","params":{"turn":{"id":"turn","status":tail}}}),
        ),
        "closed" | "reconnect" => {
            server(
                client,
                app,
                json!({"method":"carlos/codex/closed","params":{"message":"failure"}}),
            );
            if tail == "reconnect" {
                key(client, app, KeyCode::F(5), KeyModifiers::NONE);
            }
        }
        "clear" => clear(client, app),
        "rewind-key" => key(client, app, KeyCode::Esc, KeyModifiers::NONE),
        "focus" => key(client, app, KeyCode::F(4), KeyModifiers::NONE),
        _ => panic!("unknown lifetime {tail}"),
    }
    app.assert_entry_invariants();
}

fn clear(client: &Client, app: &mut AppState) {
    event(
        client,
        app,
        UiEvent::Terminal(Event::Paste("/clear".into())),
    );
    key(client, app, KeyCode::Enter, KeyModifiers::NONE);
}

fn history(app: &mut AppState) {
    super::item_history::replace_history_from_thread(app, &json!({"id":"thread","turns":[]}))
        .expect("same-thread history");
}

fn codex_guidance_cases() -> Vec<Value> {
    let mut cases = Vec::new();
    for (method, params) in [
        (
            METHODS[5],
            json!({"threadId":"thread","turnId":"turn","itemId":"item",
            "isBlocking":false,"questions":[{"id":"q","header":"Header","question":"Color?",
            "options":[{"label":"Blue"}]}]}),
        ),
        (
            METHODS[5],
            json!({"threadId":"thread","turnId":"turn","itemId":"item",
            "questions":[{"id":"q","header":"Header","question":"Color?","options":[]}]}),
        ),
        (
            METHODS[6],
            json!({"threadId":"thread","serverName":"server","mode":"url",
            "url":"https://example.com/path","message":"Browser request"}),
        ),
        (
            METHODS[6],
            json!({"threadId":"thread","serverName":"server","mode":"url","url":"file:///bad"}),
        ),
        (
            METHODS[6],
            json!({"threadId":"thread","serverName":"server","mode":"form",
            "requestedSchema":{"type":"object","properties":{"nested":{"type":"object"}}}}),
        ),
        (
            METHODS[6],
            json!({"threadId":"thread","serverName":"server","mode":"unknown"}),
        ),
    ] {
        let client = Client::new(BackendKind::Codex);
        let mut app = app(BackendKind::Codex);
        server(
            &client,
            &mut app,
            json!({"method":method,"id":"one","params":params}),
        );
        key(&client, &mut app, KeyCode::F(4), KeyModifiers::NONE);
        fill(&client, &mut app);
        let mut steps = vec![snapshot(&client, &app)];
        accept(&client, &mut app);
        steps.push(snapshot(&client, &app));
        key(&client, &mut app, KeyCode::Char('n'), KeyModifiers::ALT);
        steps.push(snapshot(&client, &app));
        cases.push(json!({"case":format!("codex-guidance/{}", cases.len()),"steps":steps}));
    }
    cases
}

fn server_cases() -> Vec<Value> {
    let mut cases = Vec::new();
    for backend in [BackendKind::Codex, BackendKind::Grok] {
        for method in METHODS {
            if backend == BackendKind::Grok && method.contains("elicitation") {
                continue;
            }
            if backend == BackendKind::Grok && method.contains("requestUserInput") {
                continue;
            }
            for tail in [
                "replace",
                "fork",
                "history",
                "resolve",
                "promote",
                "completed",
                "failed",
                "interrupted",
                "clear",
                "rewind-key",
                "focus",
                "decline",
                "cancel",
            ] {
                cases.push(server_case(backend, method, tail));
            }
            if backend == BackendKind::Codex {
                for tail in ["closed", "reconnect"] {
                    cases.push(server_case(backend, method, tail));
                }
            }
        }
    }
    cases
}

fn replacement_cases() -> Vec<Value> {
    let mut cases = Vec::new();
    for (index, (kind, mut payload)) in payloads().into_iter().enumerate() {
        payload["defaultToNo"] = json!(true);
        for id in ["one", "different-id"] {
            for tail in ["switch", "failure-replace"] {
                let client = Client::new(BackendKind::Claude);
                let mut app = app(BackendKind::Claude);
                callback(&client, &mut app, "one", kind, payload.clone());
                fill(&client, &mut app);
                accept(&client, &mut app);
                let mut steps = vec![snapshot(&client, &app)];
                claude_tail(&client, &mut app, tail);
                callback(&client, &mut app, id, kind, payload.clone());
                fill(&client, &mut app);
                steps.push(snapshot(&client, &app));
                accept(&client, &mut app);
                steps.push(snapshot(&client, &app));
                accept(&client, &mut app);
                steps.push(snapshot(&client, &app));
                cases.push(json!({"case":format!("replace/{index}/{id}/{tail}"),"steps":steps}));
            }
        }
    }
    cases
}

fn switch_helper(client: &Client, app: &mut AppState) {
    server(
        client,
        app,
        json!({"method":"claude/sessionChanged",
        "params":{"generation":2,"sessionId":"new-thread"}}),
    );
}
