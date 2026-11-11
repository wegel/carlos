//! Reconnect asynchronously and fence every native frame before it reaches UI state.

use serde_json::{json, Value};

use super::codex_callbacks::CallbackRoute;
use super::codex_reconnect_state::ReplayFrame;
use super::input::input_dispatch::dispatch_action;
use super::notifications::handle_server_message_line;
use super::state::AppState;
use crate::backend::BackendClient;

/// Start one explicit attempt against the current thread, including a forked thread ID.
pub(super) fn start(client: &dyn BackendClient, app: &mut AppState) {
    if !app.codex.closed || app.codex.connection.rebuilding {
        return;
    }
    match client.reconnect(&app.thread_id) {
        Ok(generation) => {
            drain(client, app);
            app.codex.connection.generation = generation;
            app.codex.connection.rebuilding = true;
            app.codex.connection.initial_rebuild = false;
            app.codex.epoch = app.codex.epoch.saturating_add(1);
            app.codex.callbacks = Default::default();
            app.approval.pending = None;
            app.ralph_runtime.stop_after_failure();
            app.set_status(
                "Reconnecting Codex; draft kept. Messages wait for the complete native snapshot",
            );
        }
        Err(error) => app.set_status(format!("Could not reconnect Codex: {error}")),
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
    if params["generation"].as_u64() != Some(app.codex.connection.generation) {
        client.acknowledge_event(line.len());
        return true;
    }
    let event = &params["event"];
    let method = event["method"].as_str().unwrap_or("");
    if method == "carlos/codex/closed" {
        drain(client, app);
        app.codex.connection.rebuilding = false;
        super::codex_lifecycle::close(
            app,
            event["params"].as_object().unwrap_or(&Default::default()),
        );
    } else if method == "carlos/codex/ready" && app.codex.connection.rebuilding {
        ready(client, app);
    } else if app.codex.connection.rebuilding && !callback(event) {
        if hold(app, event.clone(), line.len()) {
            return true;
        }
        drain(client, app);
        app.codex.connection.rebuilding = false;
        super::codex_lifecycle::close(
            app,
            json!({"message":"Codex reconnect replay exceeds12 MiB or2048 frames"})
                .as_object()
                .expect("object"),
        );
        client.abort_connection();
    } else if app.codex.connection.rebuilding {
        if let CallbackRoute::Handled(Some(action)) =
            super::codex_callbacks::route(app, method, event)
        {
            dispatch_action(client, app, action);
        }
    } else if !app.codex.closed {
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

fn hold(app: &mut AppState, event: Value, charged: usize) -> bool {
    let connection = &mut app.codex.connection;
    if connection.bytes + charged > 12 * 1024 * 1024 || connection.journal.len() >= 2048 {
        return false;
    }
    connection.bytes += charged;
    connection.journal.push_back(ReplayFrame { event, charged });
    true
}

fn ready(client: &dyn BackendClient, app: &mut AppState) {
    let Some(snapshot) = client.take_reconnect_snapshot(app.codex.connection.generation) else {
        failed(
            client,
            app,
            "Codex reconnect snapshot is unavailable; F5 retries",
        );
        return;
    };
    let active =
        super::codex_reconnect_replay::active_turn(&snapshot.thread, &app.codex.connection.journal);
    let goal_changed = app.codex.connection.journal.iter().any(|frame| {
        frame.event["params"]["threadId"] == app.thread_id
            && matches!(
                frame.event["method"].as_str(),
                Some("thread/goal/updated" | "thread/goal/cleared")
            )
    });
    let installed =
        active.and_then(|active| super::codex_reconnect_install::install(app, snapshot, active));
    if let Err(error) = installed {
        failed(
            client,
            app,
            &format!("Could not rebuild Codex: {error}; draft kept; F5 retries"),
        );
        return;
    }
    while let Some(frame) = app.codex.connection.journal.pop_front() {
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
    drain(client, app);
    super::codex_lifecycle::close(
        app,
        json!({"message":message})
            .as_object()
            .expect("failure object"),
    );
    client.abort_connection();
}

fn finish_ui(app: &mut AppState, goal_changed: bool) {
    app.codex.connection.bytes = 0;
    app.codex.connection.rebuilding = false;
    app.codex.connection.runtime_refresh = true;
    if let Some(choices) = app.codex.connection.initial_choices.take() {
        app.codex.settings.deferred = Some(choices);
    }
    if goal_changed {
        app.codex.goals.knowledge = super::codex_goal_state::GoalKnowledge::Unknown;
        app.codex.goals.last_read = None;
    }
    if !app.codex.connection.initial_rebuild {
        app.append_message(
            super::Role::System,
            "Codex reconnected; native work restored. Unconfirmed input was not resent.",
        );
    }
    if app.codex.connection.unreplayed_children > 0 {
        app.append_message(super::Role::System, "Codex reattached this thread. The server replays this thread’s requests; unanswered child requests may need the client that owns that child. F2 shows its state.");
        app.codex.connection.unreplayed_children = 0;
    }
    app.set_status("Codex reconnected; native work restored. Unconfirmed input was not resent");
}

fn apply(client: &dyn BackendClient, app: &mut AppState, event: &Value) {
    if super::codex_reconnect_streams::event(app, event) {
        return;
    }
    if let Some(action) = handle_server_message_line(app, &event.to_string()) {
        dispatch_action(client, app, action);
    }
}

fn drain(client: &dyn BackendClient, app: &mut AppState) {
    for frame in app.codex.connection.journal.drain(..) {
        client.acknowledge_event(frame.charged);
    }
    app.codex.connection.bytes = 0;
}
