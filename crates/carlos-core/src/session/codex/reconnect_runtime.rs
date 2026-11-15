//! Reconcile thread settings after replay without letting a delayed read overwrite a newer event.

use serde_json::{json, Value};

use super::connection_state::MetadataRead;
use crate::backend::BackendClient;
use crate::session::state::SessionState;

/// Read native metadata only; reconnect never publishes Carlos defaults.
pub fn tick(client: &dyn BackendClient, session: &mut SessionState) -> bool {
    if !session.codex_scope().enabled()
        || session.codex_scope().closed()
        || !session.codex_connection.runtime_refresh
        || session.codex_connection.runtime_read.is_some()
        || session.codex_settings.pending.is_some()
        || session.codex_settings.deferred.is_some()
    {
        return false;
    }
    session.advance_codex_serial();
    let token = session.codex_serial();
    session.codex_connection.runtime_refresh = false;
    session.codex_connection.runtime_read = Some(MetadataRead {
        token,
        settings_revision: session.codex_settings().revision(),
        name_revision: session.codex_name_revision(),
    });
    if let Err(error) = client.request_control_with_context("thread/read",
        json!({"threadId":session.thread_id(),"includeTurns":false}),
        json!({"operation":"reconnectRuntime","epoch":session.codex_scope().epoch(),"token":token,"timeoutMs":5000})) {
        session.codex_connection.runtime_read = None;
        session.set_status(format!("Could not refresh Codex thread settings: {error}"));
    }
    true
}

/// Keep later native settings events authoritative; retry only this read when its facts changed.
pub fn reply(session: &mut SessionState, params: &Value) -> bool {
    if params["requestContext"]["operation"] != "reconnectRuntime" {
        return false;
    }
    let Some(read) = session.codex_connection.runtime_read else {
        return true;
    };
    if params["requestContext"]["token"].as_u64() != Some(read.token)
        || params["requestContext"]["epoch"].as_u64() != Some(session.codex_scope().epoch())
        || params["requestParams"]["threadId"] != *session.thread_id()
    {
        return true;
    }
    session.codex_connection.runtime_read = None;
    let thread = &params["response"]["result"]["thread"];
    if thread["id"] != *session.thread_id() {
        session.set_status(
            "Could not refresh Codex thread settings; native defaults still govern prompts",
        );
        return true;
    }
    if read.name_revision == session.codex_name_revision() {
        session.observe_thread_name(thread);
    } else {
        session.codex_connection.runtime_refresh = true;
    }
    // A legacy server cannot report the explicit next-turn overrides that Carlos keeps locally.
    if session.codex_settings.legacy_overrides {
        return true;
    }
    if read.settings_revision == session.codex_settings().revision() {
        let settings = json!({"model":thread["model"],"effort":thread["reasoningEffort"],"summary":thread["summary"]});
        super::settings::observed(
            session,
            &json!({"threadId":session.thread_id(),"threadSettings":settings}),
        );
    } else {
        session.codex_connection.runtime_refresh = true;
    }
    true
}
