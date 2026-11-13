//! Apply native session reports between each frame of a synchronous reconnect drain.

use serde_json::{json, Value};

use super::input::input_dispatch::dispatch_action;
use super::state::AppState;
use carlos_core::backend::BackendClient;

/// Start one explicit attempt against the session's current native root.
pub(super) fn start(client: &dyn BackendClient, app: &mut AppState) {
    if let Some(retired) = app.session.reconnect_codex(client) {
        app.view.codex_callbacks = Default::default();
        app.drop_retired_approval_hints(retired);
    }
}

/// Apply each report and reply before the session acknowledges or advances its native drain.
pub(super) fn receive(client: &dyn BackendClient, app: &mut AppState, line: &str) -> bool {
    let editor = capture_editor(app);
    let context = super::backend_updates::context(&app.session, &app.view);
    let Some(mut batch) = app
        .session
        .receive_native_line(client, line, &context, editor)
    else {
        return false;
    };
    loop {
        let context = super::backend_updates::context(&app.session, &app.view);
        let Some(update) = batch.next(&mut app.session, client, &context) else {
            break;
        };
        if let Some(action) = app.apply_backend_update(update) {
            dispatch_action(client, app, action);
        }
    }
    true
}

fn capture_editor(app: &AppState) -> Option<Value> {
    (app.session.codex_connection().rebuilding() && app.view.runtime_settings.show_model_settings).then(|| {
        json!({"model":app.model_settings_model_value(), "effort":app.model_settings_effort_value(), "summary":app.model_settings_summary_value()})
    })
}
