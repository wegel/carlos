//! Publish future-thread and live-turn choices separately without rewriting native global config.

use serde_json::{json, Value};

use super::state::AppState;
use super::Role;
use crate::backend::BackendClient;

/// Keep acknowledged native choices separate from pending editor requests.
#[derive(Default)]
pub(super) struct SettingsState {
    pub(super) future: Value,
    pub(super) revision: u64,
    pub(super) pending: Option<Change>,
    pub(super) deferred: Option<Value>,
    pub(super) active_before: Option<(String, String)>,
    pub(super) editor_error: Option<String>,
    pub(super) rejected: Option<Value>,
    pub(super) errors: Value,
    pub(super) legacy_overrides: bool,
    pub(super) persist_next: bool,
    pub(super) announce_next: bool,
}

/// Settle future and current-turn writes independently for one requested choice.
pub(super) struct Change {
    token: u64,
    desired: Value,
    revision: u64,
    thread: Option<String>,
    turn: Option<String>,
    turn_id: Option<String>,
    persist: bool,
    announce: bool,
}

/// Keep requested choices local while a prior pair of native replies is still pending.
#[cfg(test)]
pub(super) fn save(client: &dyn BackendClient, app: &mut AppState) {
    let desired = super::codex_settings_draft::choices(app);
    save_values(client, app, desired);
}

/// Publish only the fields the settings editor actually chose.
pub(super) fn save_values(client: &dyn BackendClient, app: &mut AppState, desired: Value) {
    if let Err(error) = validate(app, &desired) {
        super::codex_settings_draft::reject(app, &desired, json!({"code":-32602,"message":error}));
        app.append_message(Role::System, format!("Codex settings not sent: {error}"));
        return;
    }
    app.codex.settings.rejected = None;
    app.codex.settings.editor_error = None;
    app.codex.settings.announce_next = true;
    app.codex.settings.deferred = Some(desired);
    tick(client, app);
}

/// Send both supported scopes asynchronously, with no synthetic model prompt.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    if !app.codex.enabled || app.codex.closed || app.codex.settings.pending.is_some() {
        return false;
    }
    let Some(desired) = app.codex.settings.deferred.take() else {
        return false;
    };
    app.codex.serial = app.codex.serial.saturating_add(1);
    let token = app.codex.serial;
    let turn_id = app.active_turn_id.clone();
    let mut change = Change {
        token,
        desired: desired.clone(),
        revision: app.codex.settings.revision,
        thread: None,
        turn: turn_id.is_none().then(|| "No live turn to update".into()),
        turn_id: turn_id.clone(),
        persist: std::mem::take(&mut app.codex.settings.persist_next),
        announce: std::mem::take(&mut app.codex.settings.announce_next),
    };
    let mut params = desired.clone();
    params["threadId"] = json!(app.thread_id);
    let context = json!({"epoch":app.codex.epoch,"token":token,"operation":"settings"});
    if let Err(error) = client.request_control_with_context(
        "thread/settings/update",
        params.clone(),
        context.clone(),
    ) {
        change.thread = Some(format!(
            "Failed to send: {error}; previous future settings remain"
        ));
        super::codex_settings_draft::reject(
            app,
            &desired,
            json!({"code":-32000,"message":error.to_string()}),
        );
    }
    if let Some(turn_id) = turn_id {
        params["turnId"] = json!(turn_id);
        if let Err(error) =
            client.request_control_with_context("turn/settings/update", params, context)
        {
            change.turn = Some(format!("Failed: {error}"));
        }
    }
    let announce = change.announce;
    app.codex.settings.pending = Some(change);
    if announce {
        app.append_message(Role::System, "Codex settings pending: future turns and any live turn are updated separately. Already running model requests keep their captured settings.");
    }
    finish(app);
    true
}

/// Apply authoritative external changes before considering an older empty acknowledgement.
pub(super) fn observed(app: &mut AppState, params: &Value) {
    if params["threadId"] != app.thread_id || !params["threadSettings"].is_object() {
        return;
    }
    let settings = &params["threadSettings"];
    app.codex.settings.revision = app.codex.settings.revision.saturating_add(1);
    app.codex.settings.legacy_overrides = false;
    app.codex.settings.future = settings.clone();
    if app.codex.settings.deferred.is_none() {
        app.runtime.pending_model = None;
        app.runtime.pending_effort = None;
        app.runtime.pending_summary = None;
    }
    apply_future(app, settings);
}

/// Settle each scope only for the exact epoch, token, thread and live turn.
pub(super) fn reply(app: &mut AppState, params: &Value) {
    if params["requestParams"]["threadId"] != app.thread_id
        || params["requestContext"]["epoch"].as_u64() != Some(app.codex.epoch)
        || params["requestContext"]["operation"] != "settings"
    {
        return;
    }
    let Some(change) = app.codex.settings.pending.as_ref() else {
        return;
    };
    if params["requestContext"]["token"].as_u64() != Some(change.token) {
        return;
    }
    let mut expected = change.desired.clone();
    expected["threadId"] = json!(app.thread_id);
    if params["requestMethod"] == "turn/settings/update" {
        expected["turnId"] = json!(change.turn_id);
    }
    if params["requestParams"] != expected {
        return;
    }
    let error = params["response"].get("error").cloned();
    match params["requestMethod"].as_str() {
        Some("thread/settings/update") if change.thread.is_none() => future_reply(app, error),
        Some("turn/settings/update") if change.turn.is_none() => live_reply(app, params, error),
        _ => return,
    }
    finish(app);
}

fn future_reply(app: &mut AppState, error: Option<Value>) {
    let change = app
        .codex
        .settings
        .pending
        .as_mut()
        .expect("settings change");
    let accepted = error.is_none();
    let unsupported = error
        .as_ref()
        .is_some_and(crate::protocol_history::is_unsupported_method);
    let desired = change.desired.clone();
    let persist = change.persist;
    let fallback = unsupported && change.revision == app.codex.settings.revision;
    if error.is_some() && !fallback {
        change.announce = true;
    }
    change.thread = Some(error.as_ref().map_or_else(|| "Acknowledged".into(), |error| {
        let message = error_text(error);
        if fallback { format!("API unavailable: {message}; direct next-turn overrides saved locally") }
        else { format!("Rejected: {message}; previous future settings retained. Ctrl+M reopens the attempted choices") }
    }));
    let update = accepted && change.revision == app.codex.settings.revision;
    if !unsupported {
        app.codex.settings.legacy_overrides = false;
    }
    if update || fallback {
        app.codex.settings.legacy_overrides = fallback;
        if update {
            for (key, value) in desired.as_object().expect("settings object") {
                app.codex.settings.future[key] = value.clone();
            }
        }
        apply_future(app, &desired);
    }
    if let Some(error) = error {
        app.codex.settings.errors["future"] = error.clone();
        if !fallback {
            super::codex_settings_draft::reject(app, &desired, error);
        }
    }
    if persist && (accepted || fallback) {
        super::codex_settings_draft::persist(app, &desired);
    }
}

fn live_reply(app: &mut AppState, params: &Value, error: Option<Value>) {
    if let Some(error) = &error {
        app.codex.settings.errors["live"] = error.clone();
    }
    let change = app
        .codex
        .settings
        .pending
        .as_mut()
        .expect("settings change");
    change.turn = Some(error.map_or_else(
        || match params["response"]["result"]["status"].as_str() {
            Some("applied") => {
                "Published for later steps only; in-flight requests are unchanged".into()
            }
            Some("targetUnavailable") => {
                "Turn is no longer available; no live change was confirmed".into()
            }
            _ => "Unrecognized native reply; no live change was confirmed".into(),
        },
        |error| format!("Failed: {}; active turn unchanged", error_text(&error)),
    ));
}

/// Preserve an attempted settings draft when the owned native process closes.
pub(super) fn closed(app: &mut AppState) {
    let desired = app.codex.settings.deferred.take().or_else(|| {
        app.codex
            .settings
            .pending
            .take()
            .map(|change| change.desired)
    });
    if let Some(desired) = desired {
        super::codex_settings_draft::reject(
            app,
            &desired,
            json!({"code":-32000,"message":"Codex closed before settings settled"}),
        );
    }
    app.codex.settings.pending = None;
}

/// Capture starting choices rather than relabeling an already running model request.
pub(super) fn started(app: &mut AppState, turn_id: &str) {
    let label = [
        app.runtime.current_model.as_deref().unwrap_or("model?"),
        app.runtime.current_effort.as_deref().unwrap_or(""),
        app.runtime.current_summary.as_deref().unwrap_or(""),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("/");
    app.codex.settings.active_before = Some((turn_id.into(), label));
}

/// Identify current-turn versus future choices when the user changes settings while busy.
pub(super) fn label(app: &AppState) -> String {
    let future = app.runtime_settings_label();
    if let Some((turn_id, previous)) = &app.codex.settings.active_before {
        if app.active_turn_id.as_ref() == Some(turn_id) && *previous != future {
            return format!("{previous} → future {future}");
        }
    }
    future
}

fn apply_future(app: &mut AppState, settings: &Value) {
    for (key, current, pending) in [
        (
            "model",
            &mut app.runtime.current_model,
            &mut app.runtime.pending_model,
        ),
        (
            "effort",
            &mut app.runtime.current_effort,
            &mut app.runtime.pending_effort,
        ),
        (
            "summary",
            &mut app.runtime.current_summary,
            &mut app.runtime.pending_summary,
        ),
    ] {
        if let Some(value) = settings.get(key) {
            *current = value.as_str().map(str::to_string);
            if pending.as_deref() == value.as_str() {
                *pending = None;
            }
        }
    }
}

fn finish(app: &mut AppState) {
    let Some(change) = &app.codex.settings.pending else {
        return;
    };
    let (Some(thread), Some(turn)) = (&change.thread, &change.turn) else {
        return;
    };
    let message = format!("Codex future turns: {thread}. Live turn: {turn}.");
    let announce = change.announce;
    app.codex.settings.pending = None;
    if announce {
        app.append_message(Role::System, message);
    }
}

/// Accept only a model and effort supplied by the current native catalog.
pub(super) fn validate(app: &AppState, desired: &Value) -> Result<(), String> {
    if desired["summary"]
        .as_str()
        .is_some_and(|value| !["auto", "concise", "detailed", "none"].contains(&value))
    {
        return Err("Choose a supported summary setting".into());
    }
    if !app.runtime.available_models.is_empty() {
        let model = app
            .runtime
            .available_models
            .iter()
            .find(|model| {
                desired["model"]
                    .as_str()
                    .or(app.runtime.current_model.as_deref())
                    == Some(model.model.as_str())
            })
            .ok_or("Choose a model advertised by Codex")?;
        if !model.supported_efforts.is_empty()
            && desired["effort"].as_str().is_some_and(|effort| {
                !model
                    .supported_efforts
                    .iter()
                    .any(|option| option == effort)
            })
        {
            return Err("Choose an effort advertised for the selected model".into());
        }
    }
    Ok(())
}

fn error_text(error: &Value) -> String {
    super::claude_question_state::safe_text(
        error["message"]
            .as_str()
            .unwrap_or("Native settings update failed"),
    )
}
