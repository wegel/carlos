//! Settle exact native settings scopes without allowing stale replies to replace new facts.

use serde_json::{json, Value};

use carlos_proto::submissions::OperationState;

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
    let mut error = params["response"].get("error").cloned();
    if params["requestMethod"] == "thread/settings/update"
        && error.is_none()
        && !params["response"]["result"].is_object()
    {
        error = Some(
            json!({"code":-32000,"message":"native settings reply did not confirm the future change"}),
        );
    }
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
    change.future_result = Some(future_outcome(accepted || fallback, error.as_ref()));
    if error.is_some() && !fallback {
        change.announce = true;
    }
    change.thread = Some(error.as_ref().map_or_else(
        || "Acknowledged".into(),
        |error| {
            let message = error_text(error);
            if fallback {
                format!("API unavailable: {message}; direct next-turn overrides saved locally")
            } else if error["code"] == -32000 {
                format!("Not confirmed: {message}; read current native settings before retrying")
            } else {
                format!("Rejected: {message}; previous future settings retained")
            }
        },
    ));
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

fn future_outcome(accepted: bool, error: Option<&Value>) -> OperationState {
    if accepted {
        OperationState::Accepted {
            entry: None,
            notice: None,
        }
    } else if error.is_some_and(|error| error["code"] == -32000) {
        OperationState::Uncertain {
            message: error_text(error.expect("error")),
            recovery: None,
        }
    } else {
        OperationState::Rejected {
            message: error_text(error.expect("rejection")),
            recovery: None,
        }
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
        |error| {
            if error["code"] == -32000 {
                format!(
                    "Not confirmed: {}; read live state before retrying",
                    error_text(&error)
                )
            } else {
                format!("Failed: {}; active turn unchanged", error_text(&error))
            }
        },
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
    let change = session
        .codex_settings
        .pending
        .take()
        .expect("settled change");
    if let Some(operation) = &change.request.operation {
        let mut state = change.future_result.expect("settled future scope");
        if let OperationState::Accepted { notice, .. } = &mut state {
            *notice = Some(message.clone());
        } else if let OperationState::Rejected {
            message: reason, ..
        }
        | OperationState::Uncertain {
            message: reason, ..
        } = &mut state
        {
            *reason = message.clone();
        }
        update.outcome = Some((operation.clone(), state));
    }
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
