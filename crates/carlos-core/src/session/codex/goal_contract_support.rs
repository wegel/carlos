//! Exercise actual native goal reads, acknowledgements and guards without an editor.
use super::goals::{self, GoalUpdate};
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;
use serde_json::{json, Value};
use std::time::Instant;
pub(super) fn session() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session
}
pub(super) fn goal(status: &str) -> Value {
    json!({"threadId":"root","objective":"Explicit native objective","status":status,
 "tokenBudget":1000,"tokensUsed":27,"timeUsedSeconds":3,"createdAt":100,"updatedAt":101})
}
pub(super) fn calls(peer: &TestBackend) -> Vec<(String, Value)> {
    peer.calls
        .lock()
        .expect("calls")
        .iter()
        .map(|(m, p, _)| (m.clone(), p.clone()))
        .collect()
}
pub(super) fn tick(peer: &TestBackend, session: &mut SessionState, now: Instant) -> bool {
    let scope = session.codex_scope();
    goals::tick(peer, session, scope, now, false)
}
pub(super) fn ralph_error(session: &SessionState) -> Option<&'static str> {
    goals::ralph_error(session, session.codex_scope())
}
pub(super) fn observed(session: &mut SessionState, value: Value) {
    super::goal_receive::observed(
        session,
        "thread/goal/updated",
        &json!({"threadId":"root","goal":value}),
    );
}
pub(super) fn receive(session: &mut SessionState, value: Value) {
    let peer = TestBackend::default();
    crate::session::backend_test_support::receive(session, &peer, &value.to_string());
}
pub(super) fn answer(
    peer: &TestBackend,
    session: &mut SessionState,
    index: usize,
    response: Value,
) {
    let (method, params, context) = peer.calls.lock().expect("calls")[index].clone();
    super::goal_receive::reply(
        session,
        session.codex_scope(),
        &mut GoalUpdate::default(),
        &json!({
 "requestMethod":method,"requestParams":params,"requestContext":context,"response":response}),
    );
}
pub(super) fn activate(peer: &TestBackend, session: &mut SessionState) {
    mutate(
        peer,
        session,
        "thread/goal/set",
        json!({"threadId":"root","status":"active","origin":"user"}),
    );
}
pub(super) fn mutate(peer: &TestBackend, session: &mut SessionState, method: &str, params: Value) {
    let scope = session.codex_scope();
    goals::can_mutate(session, scope, params["status"] == "active")
        .expect("native mutation permitted");
    goals::mutate(
        peer,
        session,
        scope,
        &mut GoalUpdate::default(),
        method,
        params,
    );
}
