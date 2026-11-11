//! Reconcile thread settings after replay without letting a delayed read overwrite a newer event.

use serde_json::{json, Value};

use super::codex_reconnect_state::MetadataRead;
use super::state::AppState;
use crate::backend::BackendClient;

/// Read native metadata only; reconnect never publishes Carlos defaults.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    if !app.codex.enabled
        || app.codex.closed
        || !app.codex.connection.runtime_refresh
        || app.codex.connection.runtime_read.is_some()
    {
        return false;
    }
    app.codex.serial = app.codex.serial.saturating_add(1);
    let token = app.codex.serial;
    app.codex.connection.runtime_refresh = false;
    app.codex.connection.runtime_read = Some(MetadataRead {
        token,
        settings_revision: app.codex.settings.revision,
        name_revision: app.codex.connection.name_revision,
    });
    if let Err(error) = client.request_control_with_context("thread/read",
        json!({"threadId":app.thread_id,"includeTurns":false}),
        json!({"operation":"reconnectRuntime","epoch":app.codex.epoch,"token":token,"timeoutMs":5000})) {
        app.codex.connection.runtime_read = None;
        app.set_status(format!("Could not refresh Codex thread settings: {error}"));
    }
    true
}

/// Keep later native settings events authoritative; retry only this read when its facts changed.
pub(super) fn reply(app: &mut AppState, params: &Value) -> bool {
    if params["requestContext"]["operation"] != "reconnectRuntime" {
        return false;
    }
    let Some(read) = app.codex.connection.runtime_read else {
        return true;
    };
    if params["requestContext"]["token"].as_u64() != Some(read.token)
        || params["requestContext"]["epoch"].as_u64() != Some(app.codex.epoch)
        || params["requestParams"]["threadId"] != app.thread_id
    {
        return true;
    }
    app.codex.connection.runtime_read = None;
    let thread = &params["response"]["result"]["thread"];
    if thread["id"] != app.thread_id {
        app.set_status(
            "Could not refresh Codex thread settings; native defaults still govern prompts",
        );
        return true;
    }
    if read.name_revision == app.codex.connection.name_revision {
        app.read_thread_name(thread);
    } else {
        app.codex.connection.runtime_refresh = true;
    }
    if read.settings_revision == app.codex.settings.revision {
        let settings = json!({"model":thread["model"],"effort":thread["reasoningEffort"],"summary":thread["summary"]});
        super::codex_settings::observed(
            app,
            &json!({"threadId":app.thread_id,"threadSettings":settings}),
        );
    } else {
        app.codex.connection.runtime_refresh = true;
    }
    true
}
