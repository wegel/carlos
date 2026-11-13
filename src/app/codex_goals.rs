//! Read native goals and send only explicit user-authorized root mutations.

#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

use carlos_core::session::codex::goals::{self, GoalUpdate};

use serde_json::{json, Value};

use super::codex_goal_state::{GoalConfirm, GoalDraft};
use super::state::AppState;
use carlos_core::backend::BackendClient;
use carlos_core::session::codex::goal_state::GoalKnowledge;

/// Poll facts only; startup and ordinary chat never create, resume, pause or clear goals.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    let scope = super::codex_queue::scope(app);
    let mut update = GoalUpdate::default();
    let changed = goals::tick(
        client,
        &mut app.session,
        scope,
        &mut update,
        now,
        app.view.codex_goals.show,
    );
    apply(app, update);
    changed
}

/// Add only an appropriate future timer to the idle wait; unavailable/closed goals do not spin.
#[cfg(test)]
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    goals::wait(
        &app.session,
        super::codex_queue::scope(app),
        now,
        app.view.codex_goals.show,
    )
}

/// Recognize only the local command; the caller routes literal rewind edits first.
pub(super) fn command(app: &mut AppState, text: &str) -> bool {
    if !app.session.codex_scope().enabled() || text.trim() != "/goal" {
        return false;
    }
    app.clear_input();
    app.view.codex_goals.show = true;
    app.session.refresh_codex_goal();
    app.view.codex_goals.scroll = 0;
    true
}

/// Open a separate draft without assigning an objective from ordinary chat.
pub(super) fn edit(app: &mut AppState) {
    if app.session.codex_goals().pending() {
        app.view.codex_goals.error =
            Some("Goal change pending; draft stays locked until the reply".into());
        return;
    }
    if app.session.codex_goals().knowledge() != GoalKnowledge::Known {
        app.view.codex_goals.error =
            Some("Read the current Codex goal before editing; r refreshes".into());
        return;
    }
    if app.view.codex_goals.editor.is_none() {
        app.view.codex_goals.editor = Some(GoalDraft::new(app.session.codex_goals().goal()));
    }
    app.view.codex_goals.editing = true;
}

/// Prepare the user's exact saved draft; an active write still needs explicit confirmation.
pub(super) fn save(client: &dyn BackendClient, app: &mut AppState) {
    let Some(draft) = &app.view.codex_goals.editor else {
        return;
    };
    match draft.params(app.session.thread_id()) {
        Ok(mut params) => {
            if params.get("status").is_none() {
                let status = app.session.codex_goals().goal()["status"]
                    .as_str()
                    .unwrap_or("paused");
                if ![
                    "active",
                    "paused",
                    "blocked",
                    "usageLimited",
                    "budgetLimited",
                    "complete",
                ]
                .contains(&status)
                {
                    app.view.codex_goals.error = Some(
                        "Unknown goal status; explicitly choose paused or active before saving"
                            .into(),
                    );
                    return;
                }
                params["status"] = json!(status);
            }
            prepare(client, app, params);
        }
        Err(error) => app.view.codex_goals.error = Some(error),
    }
}

/// Pause/resume only the explicitly selected root goal, never a child from F2.
pub(super) fn status(client: &dyn BackendClient, app: &mut AppState, status: &str) {
    if app.session.codex_goals().goal().is_null() {
        app.view.codex_goals.error = Some("No goal exists; e creates a paused draft".into());
        return;
    }
    prepare(
        client,
        app,
        json!({"threadId":app.session.thread_id(),"origin":"user","status":status}),
    );
}

fn prepare(client: &dyn BackendClient, app: &mut AppState, params: Value) {
    let active = params["status"]
        .as_str()
        .or(app.session.codex_goals().goal()["status"].as_str())
        == Some("active");
    if let Err(error) = can_mutate(app, active) {
        app.view.codex_goals.error = Some(error);
        return;
    }
    if active {
        app.view.codex_goals.scroll = 0;
        app.view.codex_goals.confirm = Some(GoalConfirm::Activate(params));
    } else {
        mutate(client, app, "thread/goal/set", params);
    }
}

/// Confirm only a visible goal action, rechecking Ralph before native can start any work.
pub(super) fn confirm(client: &dyn BackendClient, app: &mut AppState) {
    let Some(confirm) = app.view.codex_goals.confirm.take() else {
        return;
    };
    match confirm {
        GoalConfirm::Activate(params) => {
            if let Err(error) = can_mutate(app, true) {
                app.view.codex_goals.error = Some(error);
                return;
            }
            mutate(client, app, "thread/goal/set", params);
        }
        GoalConfirm::Clear => {
            if let Err(error) = can_mutate(app, false) {
                app.view.codex_goals.error = Some(error);
                return;
            }
            mutate(
                client,
                app,
                "thread/goal/clear",
                json!({"threadId":app.session.thread_id(),"origin":"user"}),
            );
        }
    }
}

fn can_mutate(app: &AppState, active: bool) -> Result<(), String> {
    goals::can_mutate(&app.session, super::codex_queue::scope(app), active)
}

fn mutate(client: &dyn BackendClient, app: &mut AppState, method: &str, params: Value) {
    let scope = super::codex_queue::scope(app);
    let mut update = GoalUpdate::default();
    goals::mutate(client, &mut app.session, scope, &mut update, method, params);
    apply(app, update);
}

/// Refuse a competing automatic driver until native facts establish a safe state.
#[cfg(test)]
pub(super) fn ralph_error(app: &AppState) -> Option<&'static str> {
    goals::ralph_error(&app.session, super::codex_queue::scope(app))
}

/// Apply only the editor changes and transcript rows named by the native goal command.
pub(super) fn apply(app: &mut AppState, update: GoalUpdate) {
    if update.clear_error {
        app.view.codex_goals.error = None;
    }
    if update.clear_editor {
        app.view.codex_goals.editor = None;
        app.view.codex_goals.editing = false;
    }
    app.apply_session_update(update.transcript);
}

/// Keep the latest local draft error ahead of the last backend failure.
pub(super) fn error(app: &AppState) -> Option<&str> {
    app.view
        .codex_goals
        .error
        .as_deref()
        .or(app.session.codex_goals().error())
}
