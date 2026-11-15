//! Exercise bounded native panel reads with explicit borrowed targets and no terminal owner.
use super::panel;
use super::panel_receive;
use super::panel_scope::{PanelScope, PanelUpdate};
use super::panel_state::PanelId;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;
use serde_json::{json, Value};
pub(super) fn session() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session
}
pub(super) fn scope(selected: Option<&PanelId>) -> PanelScope<'_> {
    PanelScope {
        native: super::scope::CodexScope::for_test(true, false, 1),
        generation: 1,
        rebuilding: false,
        visible: true,
        detail: true,
        selected,
    }
}
pub(super) fn calls(peer: &TestBackend) -> Vec<(String, Value)> {
    peer.calls
        .lock()
        .expect("calls")
        .iter()
        .map(|(m, p, _)| (m.clone(), p.clone()))
        .collect()
}
pub(super) fn contexts(peer: &TestBackend) -> Vec<Value> {
    peer.calls
        .lock()
        .expect("calls")
        .iter()
        .map(|(_, _, c)| c.clone())
        .collect()
}
pub(super) fn refresh(session: &mut SessionState, older: bool) {
    panel::refresh(session, scope(None), older);
}
pub(super) fn tick(peer: &TestBackend, session: &mut SessionState) -> bool {
    panel::tick(peer, session, scope(None), &mut PanelUpdate::default())
}
pub(super) fn stop(session: &mut SessionState, selected: Option<&PanelId>) {
    panel::stop(session, scope(selected));
}
pub(super) fn details(session: &mut SessionState, selected: Option<&PanelId>, older: bool) {
    panel::details(session, scope(selected), &mut PanelUpdate::default(), older);
}
pub(super) fn answer(peer: &TestBackend, session: &mut SessionState, index: usize, result: Value) {
    let (method, params, context) = peer.calls.lock().expect("calls")[index].clone();
    assert!(panel_receive::reply(
        session,
        scope(None),
        &mut PanelUpdate::default(),
        &json!({
 "requestMethod":method,"requestParams":params,"requestContext":context,"response":{"result":result}})
    ));
}
pub(super) fn receive(session: &mut SessionState, value: Value) {
    let peer = TestBackend::default();
    crate::session::backend_test_support::receive(session, &peer, &value.to_string());
}
pub(super) fn snapshot(peer: &TestBackend, session: &mut SessionState, agents: Value) {
    let mut next = calls(peer).len();
    refresh(session, false);
    for _ in 0..20 {
        if !session.codex_panel().has_outgoing() {
            break;
        }
        tick(peer, session);
        while next < calls(peer).len() {
            let method = calls(peer)[next].0.clone();
            let data = if method == "thread/list" {
                agents.clone()
            } else {
                json!([])
            };
            answer(peer, session, next, json!({"data":data,"nextCursor":null}));
            next += 1;
        }
    }
    assert!(session.codex_panel().snapshot().is_none());
}
