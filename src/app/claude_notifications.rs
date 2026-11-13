//! Pass normalized Claude frames to the session and apply its local reports.

#[cfg(test)]
use carlos_core::session::actions::ServerRequestAction;
#[cfg(test)]
use serde_json::{Map, Value};

use super::state::AppState;

/// Distinguish a normalized Claude fact from another backend's shared message.
#[cfg(test)]
pub(super) enum ClaudeRoute {
    Continue,
    Handled(Option<ServerRequestAction>),
}

/// Supply borrowed unsent facts without letting the session inspect a widget.
#[cfg(test)]
pub(super) fn route(app: &mut AppState, method: &str, facts: &Value) -> ClaudeRoute {
    let Some(update) = app.session.receive_claude_message(method, facts) else {
        return ClaudeRoute::Continue;
    };
    let context = app.session.backend_context();
    let update = app.session.finish_claude_update(update, &context);
    ClaudeRoute::Handled(app.apply_backend_update(update))
}

/// Attach a submitted UUID before its existing local acceptance rule.
#[cfg(test)]
pub(super) fn record_submitted_prompt(app: &mut AppState, response: &Value) {
    let update = app.session.receive_claude_submission(response);
    let _ = app.apply_claude_update(update);
}

/// Drive a named fixture through the same production reducer.
#[cfg(test)]
pub(super) fn handle(
    app: &mut AppState,
    method: &str,
    params: &Map<String, Value>,
) -> Option<ServerRequestAction> {
    let mut update = Default::default();
    app.session
        .receive_named_claude(method, &Value::Object(params.clone()), &mut update);
    let context = app.session.backend_context();
    let update = app.session.finish_claude_update(update, &context);
    app.apply_backend_update(update)
}

/// Recover submitted text and apply only returned local editor changes.
#[cfg(test)]
pub(super) fn fail_session(app: &mut AppState) {
    let mut update = Default::default();
    app.session.fail_claude(&mut update);
    let _ = app.apply_claude_update(update);
}
