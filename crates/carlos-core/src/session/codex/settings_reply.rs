//! Settle exact native settings scopes without allowing stale replies to replace new facts.

use serde_json::{json, Value};

use super::scope::CodexScope;
use super::settings::{reject, SettingsEffect, SettingsUpdate};
use crate::session::state::SessionState;

/// Match native scope, epoch, request token and exact submitted values before settling.
pub fn reply(
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut SettingsUpdate,
    params: &Value,
) {
    if params["requestParams"]["threadId"] != *session.thread_id()
        || params["requestContext"]["epoch"].as_u64() != Some(scope.epoch)
        || params["requestContext"]["operation"] != "settings"
    {
        return;
    }
    let Some(change) = session.codex_settings.pending.as_ref() else {
        return;
    };
    if params["requestContext"]["token"].as_u64() != Some(change.token) {
        return;
    }
    let mut expected = change.desired.clone();
    expected["threadId"] = json!(session.thread_id());
    if params["requestMethod"] == "turn/settings/update" {
        expected["turnId"] = json!(change.turn_id);
    }
    if params["requestParams"] != expected {
        return;
    }
    let error = params["response"].get("error").cloned();
    match params["requestMethod"].as_str() {
        Some("thread/settings/update") if change.thread.is_none() => {
            future_reply(session, update, error)
        }
        Some("turn/settings/update") if change.turn.is_none() => live_reply(session, params, error),
        _ => return,
    }
    finish(session, update);
}

fn future_reply(session: &mut SessionState, update: &mut SettingsUpdate, error: Option<Value>) {
    let change = session
        .codex_settings
        .pending
        .as_mut()
        .expect("settings change");
    let accepted = error.is_none();
    let unsupported = error
        .as_ref()
        .is_some_and(crate::protocol_history::is_unsupported_method);
    let desired = change.desired.clone();
    let persist = change.persist;
    let fallback = unsupported && change.revision == session.codex_settings.revision;
    if error.is_some() && !fallback {
        change.announce = true;
    }
    change.thread = Some(error.as_ref().map_or_else(|| "Acknowledged".into(), |error| {
        let message = error_text(error);
        if fallback { format!("API unavailable: {message}; direct next-turn overrides saved locally") }
        else { format!("Rejected: {message}; previous future settings retained. Ctrl+M reopens the attempted choices") }
    }));
    let apply = accepted && change.revision == session.codex_settings.revision;
    if !unsupported {
        session.codex_settings.legacy_overrides = false;
    }
    if apply || fallback {
        session.codex_settings.legacy_overrides = fallback;
        if apply {
            for (key, value) in desired.as_object().expect("settings object") {
                session.codex_settings.future[key] = value.clone();
            }
        }
        session.apply_future_runtime_settings(&desired);
    }
    if let Some(error) = error {
        session.codex_settings.errors["future"] = error.clone();
        if !fallback {
            reject(session, update, &desired, error);
        }
    }
    if persist && (accepted || fallback) {
        update.effects.push(SettingsEffect::Persist(desired));
    }
}

fn live_reply(session: &mut SessionState, params: &Value, error: Option<Value>) {
    if let Some(error) = &error {
        session.codex_settings.errors["live"] = error.clone();
    }
    let change = session
        .codex_settings
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

pub(in crate::session) fn finish(session: &mut SessionState, update: &mut SettingsUpdate) {
    let Some(change) = &session.codex_settings.pending else {
        return;
    };
    let (Some(thread), Some(turn)) = (&change.thread, &change.turn) else {
        return;
    };
    let message = format!("Codex future turns: {thread}. Live turn: {turn}.");
    let announce = change.announce;
    session.codex_settings.pending = None;
    if announce {
        update.effects.push(SettingsEffect::Notice(message));
    }
}

fn error_text(error: &Value) -> String {
    crate::session::text::safe_text(
        error["message"]
            .as_str()
            .unwrap_or("Native settings update failed"),
    )
}
