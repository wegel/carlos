//! Drive the actual native frame batch with recorded transport replies and no terminal owner.

use serde_json::{json, Value};

use crate::codex_snapshot::CodexSnapshot;
use crate::session::backend_test_support::context;
use crate::session::backend_update::BackendUpdate;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

pub(super) fn session() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.enable_native_history();
    session.configure_codex_connection(2, None);
    session.close_codex();
    session.set_codex_rebuilding_for_test(true);
    session
}

pub(super) fn receive(
    peer: &TestBackend,
    session: &mut SessionState,
    generation: u64,
    event: Value,
) -> (usize, Vec<BackendUpdate>) {
    let line =
        json!({"method":"carlos/codex/frame","params":{"generation":generation,"event":event}})
            .to_string();
    let mut batch = session
        .receive_native_line(peer, &line, &context(session), None)
        .expect("actual native frame");
    let mut updates = Vec::new();
    while let Some(update) = batch.next(session, peer, &context(session)) {
        updates.push(update);
    }
    (line.len(), updates)
}

pub(super) fn ready(peer: &TestBackend, session: &mut SessionState, snapshot: CodexSnapshot) {
    *peer.snapshot.lock().expect("native snapshot") = Some(snapshot);
    receive(
        peer,
        session,
        2,
        json!({"method":"carlos/codex/ready","params":{"generation":2,"threadId":"root"}}),
    );
}

pub(super) fn snapshot(text: &str, status: &str) -> CodexSnapshot {
    let mut snapshot = super::connection_test_support::snapshot();
    snapshot.thread = json!({"id":"root","status":{"type":status},"turns":[{
        "id":"turn","status":"inProgress","items":[{"id":"answer","type":"agentMessage","text":text}]}]});
    snapshot.runtime =
        json!({"result":{"thread":{"id":"root"},"model":"a","reasoningEffort":"low"}}).to_string();
    snapshot.catalog = json!({"data":[{"id":"a","model":"a","displayName":"a","isDefault":true}]});
    snapshot.queue = Some(json!([]));
    snapshot.goal = Some(json!({"goal":null}));
    snapshot.panel = json!({"agents":[],"processes":[],"older":null});
    snapshot
}

pub(super) fn delta(text: &str) -> Value {
    json!({"method":"item/agentMessage/delta","params":{"threadId":"root","turnId":"turn","itemId":"answer","delta":text}})
}

pub(super) fn complete(text: &str) -> Value {
    json!({"method":"item/completed","params":{"threadId":"root","turnId":"turn","item":{"id":"answer","type":"agentMessage","text":text}}})
}

pub(super) fn question(id: Value, owner: &str, blocking: bool) -> Value {
    json!({"id":id,"method":"item/tool/requestUserInput","params":{"threadId":owner,"turnId":"request-turn","itemId":"question-tool","isBlocking":blocking,
        "questions":[{"id":"native-q-7","header":"Native question","question":"Do not use this prose as the answer key","isSecret":false,"options":null}]}})
}
