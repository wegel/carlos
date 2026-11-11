//! Read native goals and send only explicit user-authorized root mutations.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::codex_goal_state::{GoalConfirm, GoalDraft, GoalKnowledge, GoalRequest};
use super::state::AppState;
use crate::backend::BackendClient;

/// Poll facts only; startup and ordinary chat never create, resume, pause or clear goals.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    if !app.codex.enabled || app.codex.closed || app.codex.goals.read.is_some() {
        return false;
    }
    let due = app.codex.goals.last_read.is_none()
        || app.codex.goals.show
            && app
                .codex
                .goals
                .last_read
                .is_some_and(|at| now.saturating_duration_since(at) >= Duration::from_secs(30));
    if !due {
        return false;
    }
    app.codex.goals.last_read = Some(now);
    let request = request(app, "thread/goal/get", json!({"threadId":app.thread_id}));
    let context = context(app, &request);
    app.codex.goals.read = Some(request);
    if let Err(error) = client.request_control_with_context(
        "thread/goal/get",
        json!({"threadId":app.thread_id}),
        context,
    ) {
        app.codex.goals.read = None;
        app.codex.goals.knowledge = GoalKnowledge::Unknown;
        super::codex_goals::reconcile_ralph(app);
        app.codex.goals.read_error = Some(format!("Could not read Codex goal: {error}"));
    }
    true
}

/// Add only an appropriate future timer to the idle wait; unavailable/closed goals do not spin.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    if !app.codex.enabled || app.codex.closed || app.codex.goals.read.is_some() {
        return None;
    }
    match app.codex.goals.last_read {
        None => Some(Duration::ZERO),
        Some(at) if app.codex.goals.show => {
            Some((at + Duration::from_secs(30)).saturating_duration_since(now))
        }
        _ => None,
    }
}

/// Recognize only the local command; the caller routes literal rewind edits first.
pub(super) fn command(app: &mut AppState, text: &str) -> bool {
    if !app.codex.enabled || text.trim() != "/goal" {
        return false;
    }
    app.clear_input();
    app.codex.goals.show = true;
    app.codex.goals.last_read = None;
    app.codex.goals.scroll = 0;
    true
}

/// Open a separate draft without assigning an objective from ordinary chat.
pub(super) fn edit(app: &mut AppState) {
    if app.codex.goals.mutation.is_some() {
        app.codex.goals.error =
            Some("Goal change pending; draft stays locked until the reply".into());
        return;
    }
    if app.codex.goals.knowledge != GoalKnowledge::Known {
        app.codex.goals.error =
            Some("Read the current Codex goal before editing; r refreshes".into());
        return;
    }
    if app.codex.goals.editor.is_none() {
        app.codex.goals.editor = Some(GoalDraft::new(&app.codex.goals.goal));
    }
    app.codex.goals.editing = true;
}

/// Prepare the user's exact saved draft; an active write still needs explicit confirmation.
pub(super) fn save(client: &dyn BackendClient, app: &mut AppState) {
    let Some(draft) = &app.codex.goals.editor else {
        return;
    };
    match draft.params(&app.thread_id) {
        Ok(mut params) => {
            if params.get("status").is_none() {
                let status = app.codex.goals.goal["status"].as_str().unwrap_or("paused");
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
                    app.codex.goals.error = Some(
                        "Unknown goal status; explicitly choose paused or active before saving"
                            .into(),
                    );
                    return;
                }
                params["status"] = json!(status);
            }
            prepare(client, app, params);
        }
        Err(error) => app.codex.goals.error = Some(error),
    }
}

/// Pause/resume only the explicitly selected root goal, never a child from F2.
pub(super) fn status(client: &dyn BackendClient, app: &mut AppState, status: &str) {
    if app.codex.goals.goal.is_null() {
        app.codex.goals.error = Some("No goal exists; e creates a paused draft".into());
        return;
    }
    prepare(
        client,
        app,
        json!({"threadId":app.thread_id,"origin":"user","status":status}),
    );
}

fn prepare(client: &dyn BackendClient, app: &mut AppState, params: Value) {
    let active = params["status"]
        .as_str()
        .or(app.codex.goals.goal["status"].as_str())
        == Some("active");
    if let Err(error) = can_mutate(app, active) {
        app.codex.goals.error = Some(error);
        return;
    }
    if active {
        app.codex.goals.scroll = 0;
        app.codex.goals.confirm = Some(GoalConfirm::Activate(params));
    } else {
        mutate(client, app, "thread/goal/set", params);
    }
}

/// Confirm only a visible goal action, rechecking Ralph before native can start any work.
pub(super) fn confirm(client: &dyn BackendClient, app: &mut AppState) {
    let Some(confirm) = app.codex.goals.confirm.take() else {
        return;
    };
    match confirm {
        GoalConfirm::Activate(params) => {
            if let Err(error) = can_mutate(app, true) {
                app.codex.goals.error = Some(error);
                return;
            }
            mutate(client, app, "thread/goal/set", params);
        }
        GoalConfirm::Clear => {
            if let Err(error) = can_mutate(app, false) {
                app.codex.goals.error = Some(error);
                return;
            }
            mutate(
                client,
                app,
                "thread/goal/clear",
                json!({"threadId":app.thread_id,"origin":"user"}),
            );
        }
    }
}

fn can_mutate(app: &AppState, active: bool) -> Result<(), String> {
    if app.codex.closed {
        return Err("Codex is closed; the goal draft is kept".into());
    }
    if app.codex.goals.knowledge != GoalKnowledge::Known {
        return Err("Goal controls unavailable; r refreshes native state".into());
    }
    if app.codex.goals.direct_input == Some(false) {
        return Err("Codex does not allow direct goal changes for this thread".into());
    }
    if app.codex.goals.mutation.is_some() {
        return Err("A goal change is still pending; no automatic retry".into());
    }
    if active && (app.ralph_enabled() || app.ralph_runtime.toggle_pending()) {
        return Err(
            "Turn Ralph off with Ctrl+R and cancel any pending toggle before starting a Codex goal"
                .into(),
        );
    }
    Ok(())
}

fn mutate(client: &dyn BackendClient, app: &mut AppState, method: &str, params: Value) {
    let request = request(app, method, params.clone());
    let context = context(app, &request);
    app.codex.goals.mutation = Some(request);
    app.codex.goals.error = None;
    app.codex.goals.notice = "Goal change pending; Codex owns execution and budget checks".into();
    if let Err(error) = client.request_control_with_context(method, params, context) {
        if app
            .codex
            .goals
            .mutation
            .as_ref()
            .is_some_and(|request| request.may_activate)
        {
            app.codex.goals.knowledge = GoalKnowledge::Unknown;
            app.codex.goals.revision = app.codex.goals.revision.saturating_add(1);
            app.codex.goals.last_read = None;
            reconcile_ralph(app);
        }
        app.codex.goals.mutation = None;
        app.codex.goals.notice.clear();
        app.codex.goals.error = Some(format!(
            "Goal change was not confirmed: {error}. Draft kept; no automatic retry"
        ));
    }
}

fn request(app: &mut AppState, method: &str, params: Value) -> GoalRequest {
    app.codex.serial = app.codex.serial.saturating_add(1);
    let may_activate = method == "thread/goal/set"
        && params["status"]
            .as_str()
            .or(app.codex.goals.goal["status"].as_str())
            == Some("active");
    GoalRequest {
        token: app.codex.serial,
        method: method.into(),
        params,
        revision: app.codex.goals.revision,
        may_activate,
    }
}

fn context(app: &AppState, request: &GoalRequest) -> Value {
    json!({"operation":"goal","epoch":app.codex.epoch,"token":request.token,
        "timeoutMs":if request.method=="thread/goal/get" {5000} else {15000}})
}

/// Refuse a competing automatic driver until native facts establish a safe state.
pub(super) fn ralph_error(app: &AppState) -> Option<&'static str> {
    if !app.codex.enabled {
        return None;
    }
    if app.codex.closed {
        return Some("Codex is closed; restart before enabling Ralph");
    }
    if app.codex.goals.mutation.is_some() {
        return Some("Wait for the pending Codex goal change before enabling Ralph");
    }
    match app.codex.goals.knowledge {
        GoalKnowledge::Unknown => {
            Some("Codex goal state is unknown. Open /goal and refresh before enabling Ralph")
        }
        GoalKnowledge::Unavailable => None,
        GoalKnowledge::Known
            if app.codex.goals.goal.is_null()
                || matches!(
                    app.codex.goals.goal["status"].as_str(),
                    Some("paused" | "complete")
                ) =>
        {
            None
        }
        GoalKnowledge::Known => {
            Some("Pause or clear the Codex goal in /goal before enabling Ralph")
        }
    }
}

/// Stop only Carlos's automatic driver, retaining queued humans and native goal authority.
pub(super) fn reconcile_ralph(app: &mut AppState) {
    let may_drive = app.codex.goals.knowledge == GoalKnowledge::Unknown
        || app.codex.goals.knowledge == GoalKnowledge::Known
            && !app.codex.goals.goal.is_null()
            && !matches!(
                app.codex.goals.goal["status"].as_str(),
                Some("paused" | "complete")
            );
    if may_drive && (app.ralph_enabled() || app.ralph_runtime.toggle_pending()) {
        app.ralph_runtime.stop_after_failure();
        app.append_message(super::Role::System,"Carlos stopped Ralph because a Codex goal may drive this thread. /goal shows its state; queued human inputs remain.");
    }
}
