//! Reconnect asynchronously and fence every native frame before it reaches UI state.

use serde_json::{json, Value};

use super::codex_callbacks::CallbackRoute;
use super::input::input_dispatch::dispatch_action;
use super::notifications::handle_server_message_line;
use super::state::AppState;
use crate::backend::BackendClient;

/// Start one explicit attempt against the current thread, including a forked thread ID.
pub(super) fn start(client: &dyn BackendClient, app: &mut AppState) {
    if let Some(retired) = app.session.reconnect_codex(client) {
        app.view.codex_callbacks = Default::default();
        app.drop_retired_approval_hints(retired);
    }
}

/// Unwrap the stable stream and retain exact charges for frames waiting on a snapshot.
pub(super) fn receive(client: &dyn BackendClient, app: &mut AppState, line: &str) -> bool {
    let Ok(frame) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    if frame["method"] != "carlos/codex/frame" {
        return false;
    }
    let params = &frame["params"];
    if params["generation"].as_u64() != Some(app.session.codex_connection().generation()) {
        client.acknowledge_event(line.len());
        return true;
    }
    let event = &params["event"];
    let method = event["method"].as_str().unwrap_or("");
    if method == "carlos/codex/closed" {
        app.session.drain_codex_replay(client);
        app.session.stop_codex_rebuild();
        super::codex_lifecycle::close(
            app,
            event["params"].as_object().unwrap_or(&Default::default()),
        );
    } else if method == "carlos/codex/ready" && app.session.codex_connection().rebuilding() {
        ready(client, app);
    } else if app.session.codex_connection().rebuilding() && !callback(event) {
        if app.session.hold_codex_replay(event.clone(), line.len()) {
            return true;
        }
        app.session.drain_codex_replay(client);
        app.session.stop_codex_rebuild();
        super::codex_lifecycle::close(
            app,
            json!({"message":"Codex reconnect replay exceeds12 MiB or2048 frames"})
                .as_object()
                .expect("object"),
        );
        client.abort_connection();
    } else if app.session.codex_connection().rebuilding() {
        if let CallbackRoute::Handled(Some(action)) =
            super::codex_callbacks::route(app, method, event)
        {
            dispatch_action(client, app, action);
        }
    } else if !app.session.codex_scope().closed() {
        apply(client, app, event);
    }
    client.acknowledge_event(line.len());
    true
}

fn callback(event: &Value) -> bool {
    event.get("method").is_some() && event.get("id").is_some()
        || matches!(
            event["method"].as_str(),
            Some("serverRequest/resolved" | "carlos/codex/formChecked")
        )
        || event["method"] == "carlos/codex/controlReply"
            && event["params"]["requestMethod"] == "turn/interrupt"
}

fn ready(client: &dyn BackendClient, app: &mut AppState) {
    let Some(snapshot) =
        client.take_reconnect_snapshot(app.session.codex_connection().generation())
    else {
        failed(client, app, "Codex reconnect snapshot is unavailable");
        return;
    };
    let active = app.session.codex_reconnect_active_turn(&snapshot.thread);
    let goal_changed = app.session.codex_reconnect_goal_changed();
    let installed =
        active.and_then(|active| super::codex_reconnect_install::install(app, snapshot, active));
    if let Err(error) = installed {
        failed(
            client,
            app,
            &format!("Could not rebuild Codex: {error}; draft kept"),
        );
        return;
    }
    while let Some(frame) = app.session.take_codex_replay_frame() {
        if !matches!(
            frame.event["method"].as_str(),
            Some("thread/settings/updated" | "thread/initialized" | "thread/name/updated")
        ) {
            apply(client, app, &frame.event);
        }
        client.acknowledge_event(frame.charged);
    }
    finish_ui(app, goal_changed);
}

fn failed(client: &dyn BackendClient, app: &mut AppState, message: &str) {
    app.session.drain_codex_replay(client);
    super::codex_lifecycle::close_with_guidance(
        app,
        json!({"message":message})
            .as_object()
            .expect("failure object"),
        "; F5 retries",
    );
    client.abort_connection();
}

fn finish_ui(app: &mut AppState, goal_changed: bool) {
    let changes = app.session.finish_codex_reconnect(goal_changed);
    app.apply_item_changes(changes);
}

fn apply(client: &dyn BackendClient, app: &mut AppState, event: &Value) {
    if super::codex_reconnect_streams::event(app, event) {
        return;
    }
    if let Some(action) = handle_server_message_line(app, &event.to_string()) {
        dispatch_action(client, app, action);
    }
}
