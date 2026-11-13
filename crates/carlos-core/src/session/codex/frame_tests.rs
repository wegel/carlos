//! Prove native replay order and exact acknowledgment points without a terminal client.

use std::sync::atomic::Ordering;

use serde_json::{json, Value};

use super::connection_test_support::snapshot;
use super::frame_batch::NativeBatch;
use crate::session::backend_test_support::context;
use crate::session::backend_update::{BackendChange, BackendUpdate};
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

#[test]
fn held_frames_and_ready_are_acknowledged_only_after_their_reports() {
    let mut session = rebuilding(true);
    let client = TestBackend {
        snapshot: std::sync::Mutex::new(Some(snapshot())),
        ..Default::default()
    };
    let item = frame(
        7,
        json!({"method":"item/completed","params":{"threadId":"root","turnId":"turn","item":{"id":"item","type":"agentMessage","text":"replayed"}}}),
    );
    let name = frame(
        7,
        json!({"method":"thread/name/updated","params":{"threadId":"root","name":"skipped"}}),
    );
    for line in [&item, &name] {
        let mut batch = session
            .receive_native_line(&client, line, &context(&session), None)
            .expect("wrapped");
        assert!(advance(&mut batch, &mut session, &client).is_none());
    }
    assert!(
        acks(&client).is_empty(),
        "held native charges remain outstanding"
    );
    let stale = frame(6, json!({"method":"carlos/codex/ready"}));
    drain(&mut session, &client, &stale);
    assert!(client.snapshot.lock().expect("snapshot").is_some());
    let ready = frame(7, json!({"method":"carlos/codex/ready"}));
    let mut batch = session
        .receive_native_line(
            &client,
            &ready,
            &context(&session),
            Some(json!({"model":"unsent"})),
        )
        .expect("ready");
    let installed = advance(&mut batch, &mut session, &client).expect("installed report");
    match installed.change {
        BackendChange::Installed(report) => {
            assert_eq!(report.editor.expect("captured editor")["model"], "unsent")
        }
        _ => panic!("snapshot must precede replay"),
    }
    assert_eq!(session.messages()[0].text, "original");
    assert_eq!(acks(&client), [stale.len()]);
    advance(&mut batch, &mut session, &client).expect("replayed item report");
    assert_eq!(session.messages()[0].text, "replayed");
    assert_eq!(acks(&client), [stale.len()]);
    advance(&mut batch, &mut session, &client).expect("finished report");
    assert_eq!(acks(&client), [stale.len(), item.len(), name.len()]);
    assert!(!session.codex_connection().rebuilding());
    assert!(advance(&mut batch, &mut session, &client).is_none());
    assert!(advance(&mut batch, &mut session, &client).is_none());
    assert_eq!(
        acks(&client),
        [stale.len(), item.len(), name.len(), ready.len()]
    );
    assert!(client.snapshot.lock().expect("snapshot").is_none());
}

#[test]
fn callbacks_during_rebuild_ignore_the_normal_enabled_and_closed_guards() {
    let mut session = rebuilding(false);
    let client = TestBackend::default();
    let line = frame(
        7,
        json!({"id":"request","method":"item/commandExecution/requestApproval",
        "params":{"threadId":"child","turnId":"turn","itemId":"item","command":"pwd"}}),
    );
    drain(&mut session, &client, &line);
    assert_eq!(
        session
            .approval()
            .pending()
            .expect("callback accepted")
            .facts()
            .request_id,
        "request"
    );
    assert!(session.codex_scope().closed());
    assert!(!session.codex_scope().enabled());
    assert_eq!(acks(&client), [line.len()]);
    assert!(
        session.take_codex_replay_frame().is_none(),
        "callback was never journaled"
    );
}

#[test]
fn failed_ready_keeps_the_old_duplicate_close_rebuild_rule() {
    let mut session = rebuilding(true);
    session.close_codex_native(json!({"message":"closed"}).as_object().expect("object"), "");
    session.begin_initial_codex_rebuild();
    let client = TestBackend::default();
    let ready = frame(7, json!({"method":"carlos/codex/ready"}));
    drain(&mut session, &client, &ready);
    assert!(
        session.codex_connection().rebuilding(),
        "duplicate failure does not stop rebuild"
    );
    assert!(session.codex_scope().closed());
    assert_eq!(client.aborted.load(Ordering::SeqCst), 1);
    assert_eq!(acks(&client), [ready.len()]);
}

#[test]
fn explicit_native_close_stops_rebuild_even_after_a_duplicate_close() {
    let mut session = rebuilding(true);
    session.close_codex_native(json!({"message":"closed"}).as_object().expect("object"), "");
    session.begin_initial_codex_rebuild();
    assert!(session.hold_codex_replay(json!({"method":"held"}), 19));
    let client = TestBackend::default();
    let closed = frame(
        7,
        json!({"method":"carlos/codex/closed","params":{"message":"closed again"}}),
    );
    drain(&mut session, &client, &closed);
    assert!(!session.codex_connection().rebuilding());
    assert_eq!(acks(&client), [19, closed.len()]);
    assert_eq!(client.aborted.load(Ordering::SeqCst), 0);
}

fn rebuilding(enabled: bool) -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    if enabled {
        session.enable_codex();
    }
    session.configure_codex_connection(7, None);
    session.begin_initial_codex_rebuild();
    session
}

fn frame(generation: u64, event: Value) -> String {
    json!({"method":"carlos/codex/frame","params":{"generation":generation,"event":event}})
        .to_string()
}

fn drain(session: &mut SessionState, client: &TestBackend, line: &str) {
    let mut batch: NativeBatch = session
        .receive_native_line(client, line, &context(session), None)
        .expect("native frame");
    while advance(&mut batch, session, client).is_some() {}
}

fn acks(client: &TestBackend) -> Vec<usize> {
    client.acknowledgements.lock().expect("acks").clone()
}

fn advance(
    batch: &mut NativeBatch,
    session: &mut SessionState,
    client: &TestBackend,
) -> Option<BackendUpdate> {
    let scope = context(session);
    batch.next(session, client, &scope)
}
