//! Drive the core native task readers with fixed frames and no terminal owner.

use std::time::{Duration, Instant};

use carlos_proto::task_overview::TaskOverviewData;
use serde_json::{json, Value};

use super::panel;
use super::panel_scope::{PanelScope, PanelUpdate};
use super::panel_state::Process;
use super::{task_badge, task_status};
use crate::session::backend_test_support::action;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

pub(super) fn session() -> SessionState {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.enable_codex();
    core.enable_native_history();
    core
}

pub(super) fn scope(core: &SessionState) -> PanelScope<'static> {
    PanelScope {
        native: core.codex_scope(),
        generation: core.codex_connection().generation(),
        rebuilding: core.codex_connection().rebuilding(),
        visible: false,
        detail: false,
        selected: None,
    }
}

pub(super) fn feed(core: &mut SessionState, method: &str, params: Value) {
    let context = core.backend_context();
    core.handle_backend_line(
        &json!({"method":method,"params":params}).to_string(),
        &context,
    );
    core.publish();
}

pub(super) fn agent(id: &str, state: &str) -> Value {
    json!({"id":id,"parentThreadId":"root","status":{"type":state,"activeFlags":[]}})
}

pub(super) fn process(owner: &str, id: &str) -> Process {
    Process::new(
        owner.into(),
        json!({"processId":id,"itemId":format!("item-{id}"),
        "command":"sleep 30","cwd":"/tmp"}),
    )
}

pub(super) fn own(core: &mut SessionState) {
    core.verify_codex_agent_owners_for_test(core.codex_scope().epoch());
}

pub(super) fn observed(core: &mut SessionState, method: &str, params: &Value) {
    let scope = scope(core);
    task_status::observed(core, scope, method, params);
}

pub(super) fn hint(core: &mut SessionState, id: &str) {
    observed(
        core,
        "item/started",
        &json!({"threadId":"root",
        "item":{"type":"commandExecution","id":format!("item-{id}"),"processId":id}}),
    );
}

pub(super) fn answer(peer: &TestBackend, core: &mut SessionState, index: usize, response: Value) {
    let (method, params, context) = peer.calls.lock().expect("native calls")[index].clone();
    feed(
        core,
        "carlos/codex/controlReply",
        json!({"requestMethod":method,"requestParams":params,
        "requestContext":context,"response":response}),
    );
}

pub(super) fn count(core: &SessionState) -> usize {
    task_badge::count(core, scope(core))
}

pub(super) fn tick(peer: &TestBackend, core: &mut SessionState, now: Instant) -> bool {
    let scope = scope(core);
    task_status::tick(peer, core, scope, now)
}

pub(super) fn wait(core: &SessionState, now: Instant) -> Option<Duration> {
    task_status::wait(core, scope(core), now)
}

pub(super) fn panel_updated(core: &mut SessionState) {
    let scope = scope(core);
    task_status::panel_updated(core, scope);
}

pub(super) fn terminated(core: &mut SessionState, owner: &str, process: &str, item: Option<&str>) {
    task_status::terminated(core, owner, process, item);
}

pub(super) fn refresh(core: &mut SessionState) {
    let scope = scope(core);
    panel::refresh(core, scope, false);
}

pub(super) fn panel_tick(peer: &TestBackend, core: &mut SessionState) {
    let scope = scope(core);
    panel::tick(peer, core, scope, &mut PanelUpdate::default());
}

/// Drain the same native reconnect envelope and its checked actions as the owner.
pub(super) fn native(core: &mut SessionState, peer: &TestBackend, line: &str) {
    let context = core.backend_context();
    let mut batch = core
        .receive_native_line(peer, line, &context, None)
        .expect("native frame");
    loop {
        let context = core.backend_context();
        let Some(update) = batch.next(core, peer, &context) else {
            break;
        };
        if let Some(request) = action(update) {
            let context = core.backend_context();
            assert!(action(core.run_backend_action(peer, request, &context)).is_none());
        }
    }
}

/// Inspect the same bounded diagnostic fields the daemon terminal reads for F2.
pub(super) fn overview(core: &SessionState) -> String {
    let page = core
        .task_overview_page(None, None, 32)
        .expect("task overview");
    assert!(!page.has_more, "small fixed diagnostic fixture");
    page.entries
        .into_iter()
        .filter_map(|entry| match entry.data {
            TaskOverviewData::ProcessError { message, .. } => {
                assert!(message.complete, "small fixed error");
                Some(message.text)
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
