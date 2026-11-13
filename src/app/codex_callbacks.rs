//! Apply local editor and focus changes reported by the Codex callback owner.

use serde_json::Value;

use super::claude_question_state::safe_text;
use super::state::AppState;
#[cfg(test)]
use crate::session::actions::ServerRequestAction;
use crate::session::approval_state::ApprovalQueue;
use crate::session::codex::callback_commands::CallbackRetired;
use crate::session::codex::callbacks::CallbackKey;

#[path = "codex_callback_route.rs"]
mod routing;

pub(super) use self::routing::{route, CallbackRoute};

/// Create exactly one editor after the session accepts the native callback.
#[cfg(test)]
pub(super) fn receive(
    app: &mut AppState,
    method: &str,
    frame: &Value,
) -> Option<ServerRequestAction> {
    let update = app
        .session
        .accept_codex_callback(method, frame, app.view.codex_callbacks.focused);
    routing::apply(app, update)
}

/// Match resolutions in the session before removing the exact local editor.
#[cfg(test)]
pub(super) fn resolved(app: &mut AppState, params: &Value) {
    if let Some(retired) = app.session.resolve_codex_callback(params) {
        apply_retired(app, retired);
    }
}

/// Remove only the answered or cancelled request's local editor.
pub(super) fn retire(app: &mut AppState, id: &Value) {
    let retired = app.session.retire_codex_callback(id);
    apply_retired(app, retired);
}

/// Recompute the old focus rule after the session promotes a queued card.
pub(super) fn apply_retired(app: &mut AppState, retired: CallbackRetired) {
    app.drop_retired_approval_hints(retired.cards);
    if let Some(key) = retired.key {
        app.view.codex_callbacks.drafts.remove(&key);
        app.view.codex_callbacks.opened.remove(&key);
    }
    if retired.active {
        app.view.codex_callbacks.focused =
            app.session.approval().pending().is_some_and(|pending| {
                app.session
                    .codex_callbacks()
                    .get(&pending.facts().request_id)
                    .is_some_and(|entry| entry.blocking())
                    || CallbackKey::new(&pending.facts().request_id)
                        .is_some_and(|key| app.view.codex_callbacks.opened.contains(&key))
            });
    }
}

/// Cycle between native requests; a hidden nonblocking question opens on the first F4.
pub(super) fn next(app: &mut AppState) {
    if app.view.codex_callbacks.focused
        && !app
            .session
            .approval()
            .queued(ApprovalQueue::Codex)
            .is_empty()
    {
        let id = app.session.approval().queued(ApprovalQueue::Codex)[0].entry_id();
        app.session.focus_approval(id, ApprovalQueue::Codex);
    }
    if let Some(key) = app
        .session
        .approval()
        .pending()
        .and_then(|pending| CallbackKey::new(&pending.facts().request_id))
    {
        app.view.codex_callbacks.opened.insert(key);
        app.view.codex_callbacks.focused = true;
    }
}

/// Restore the main composer only when the visible request is nonblocking.
pub(super) fn hide_nonblocking(app: &mut AppState) -> bool {
    let nonblocking = app
        .session
        .approval()
        .pending()
        .and_then(|pending| {
            app.session
                .codex_callbacks()
                .get(&pending.facts().request_id)
        })
        .is_some_and(|entry| !entry.blocking());
    if nonblocking {
        if let Some(key) = app
            .session
            .approval()
            .pending()
            .and_then(|pending| CallbackKey::new(&pending.facts().request_id))
        {
            app.view.codex_callbacks.opened.remove(&key);
        }
        app.view.codex_callbacks.focused = false;
        if let Some(index) = app
            .session
            .approval()
            .queued(ApprovalQueue::Codex)
            .iter()
            .position(|pending| {
                app.session
                    .codex_callbacks()
                    .get(&pending.facts().request_id)
                    .is_some_and(|entry| entry.blocking())
            })
        {
            let id = app.session.approval().queued(ApprovalQueue::Codex)[index].entry_id();
            app.session.focus_approval(id, ApprovalQueue::Codex);
            app.view.codex_callbacks.focused = true;
        }
    }
    nonblocking
}

/// Invalidate only the currently edited request's submitted candidate.
pub(super) fn edited(app: &mut AppState) {
    if let Some(id) = app
        .session
        .approval()
        .pending()
        .map(|p| p.facts().request_id.clone())
    {
        app.session.edit_codex_callback(&id);
    }
}

/// Apply the matching worker's editor changes and return its guarded native reply.
#[cfg(test)]
pub(super) fn checked(app: &mut AppState, params: &Value) -> Option<ServerRequestAction> {
    let update = app.session.checked_codex_callback(params);
    routing::apply(app, update)
}

/// Keep reply and worker failures beside their own answer field.
pub(super) fn reply_hint(app: &mut AppState, id: &Value, error: &str) {
    app.session.codex_callback_reply_failed(id);
    if let Some(draft) =
        CallbackKey::new(id).and_then(|key| app.view.codex_callbacks.drafts.get_mut(&key))
    {
        draft.error = Some(safe_text(error));
        draft.viewport.scroll.set(0);
    }
    app.set_hint(safe_text(error));
}

/// Keep a failed backend reply as a session fact as well as an editor error.
pub(super) fn reply_error(app: &mut AppState, id: &Value, error: &str) {
    reply_hint(app, id, error);
    app.set_status(safe_text(error));
}

/// Retire every old-session callback before the replacement transcript is activated.
pub(super) fn reset(app: &mut AppState) {
    for id in app.session.codex_callback_ids() {
        retire(app, &id);
    }
    app.view.codex_callbacks.focused = false;
}
