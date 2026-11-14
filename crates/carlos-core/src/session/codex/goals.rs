//! Read native goals and send explicitly accepted plain goal changes.

use std::sync::{atomic::AtomicBool, Arc};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::goal_state::{GoalKnowledge, GoalRequest};
use super::scope::CodexScope;
use crate::backend::BackendClient;
use crate::session::models::Role;
use crate::session::state::SessionState;

/// Describe only the local effects of a native goal change.
#[derive(Default)]
pub struct GoalUpdate {
    pub outcome: Option<(
        carlos_proto::submissions::ClientOperation,
        carlos_proto::submissions::OperationState,
    )>,
    pub clear_error: bool,
    pub clear_editor: bool,
}

/// Read native facts when due; opening an editor never submits a goal.
pub fn tick(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: CodexScope,
    now: Instant,
    visible: bool,
) -> bool {
    if !scope.enabled || scope.closed || session.codex_goals.read.is_some() {
        return false;
    }
    let due = session.codex_goals.last_read.is_none()
        || visible
            && session
                .codex_goals
                .last_read
                .is_some_and(|at| now.saturating_duration_since(at) >= Duration::from_secs(30));
    if !due {
        return false;
    }
    session.codex_goals.last_read = Some(now);
    let request = request(
        session,
        "thread/goal/get",
        json!({"threadId":session.thread_id()}),
    );
    let context = context(scope, &request);
    session.codex_goals.read = Some(request);
    if let Err(error) = client.request_control_with_context(
        "thread/goal/get",
        json!({"threadId":session.thread_id()}),
        context,
    ) {
        session.codex_goals.read = None;
        session.codex_goals.knowledge = GoalKnowledge::Unknown;
        reconcile_ralph(session);
        session.codex_goals.read_error = Some(format!("Could not read Codex goal: {error}"));
    }
    true
}

/// Return the next native read deadline without storing client visibility.
pub fn wait(
    session: &SessionState,
    scope: CodexScope,
    now: Instant,
    visible: bool,
) -> Option<Duration> {
    if !scope.enabled || scope.closed || session.codex_goals.read.is_some() {
        return None;
    }
    match session.codex_goals.last_read {
        None => Some(Duration::ZERO),
        Some(at) if visible => Some((at + Duration::from_secs(30)).saturating_duration_since(now)),
        _ => None,
    }
}

/// Check native availability and competing automatic work before a goal change.
pub fn can_mutate(session: &SessionState, scope: CodexScope, active: bool) -> Result<(), String> {
    if scope.closed {
        return Err("Codex is closed; the goal draft is kept".into());
    }
    if session.codex_goals.knowledge != GoalKnowledge::Known {
        return Err("Goal controls unavailable; refresh native state".into());
    }
    if session.codex_goals.direct_input == Some(false) {
        return Err("Codex does not allow direct goal changes for this thread".into());
    }
    if session.codex_goals.mutation.is_some() {
        return Err("A goal change is still pending; no automatic retry".into());
    }
    if active && (session.ralph_enabled() || session.ralph_runtime().toggle_pending()) {
        return Err(
            "Turn Ralph off and cancel any pending toggle before starting a Codex goal".into(),
        );
    }
    Ok(())
}

/// Submit an already confirmed goal change with its native correlation token.
pub fn mutate(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut GoalUpdate,
    method: &str,
    params: Value,
) {
    let (context, _) = start_mutation(session, scope, update, method, &params, None);
    if let Err(error) = client.request_control_with_context(method, params, context) {
        fail_mutation(session, &error.to_string());
    }
}

/// Keep a remote mutation's operation and permission through the actual native write queue.
pub(crate) fn mutate_owned(
    commands: &crate::backend_commands::CommandHandle,
    session: &mut SessionState,
    scope: CodexScope,
    method: &str,
    params: Value,
    operation: carlos_proto::submissions::ClientOperation,
) {
    let (context, permission) = start_mutation(
        session,
        scope,
        &mut GoalUpdate::default(),
        method,
        &params,
        Some(operation),
    );
    if let Err(error) = commands
        .guarded(permission)
        .and_then(|commands| commands.request_control_with_context(method, params, context))
    {
        fail_mutation(session, &error.to_string());
    }
}

fn start_mutation(
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut GoalUpdate,
    method: &str,
    params: &Value,
    operation: Option<carlos_proto::submissions::ClientOperation>,
) -> (Value, crate::dispatch_guard::DispatchGuard) {
    let mut request = request(session, method, params.clone());
    request.operation = operation;
    let context = context(scope, &request);
    let permission = crate::dispatch_guard::DispatchGuard::from_flag(request.live.clone());
    session.codex_goals.mutation = Some(request);
    session.codex_goals.error = None;
    update.clear_error = true;
    session.codex_goals.notice =
        "Goal change pending; Codex owns execution and budget checks".into();
    (context, permission)
}

fn fail_mutation(session: &mut SessionState, error: &str) {
    if session
        .codex_goals
        .mutation
        .as_ref()
        .is_some_and(|request| request.may_activate)
    {
        session.codex_goals.knowledge = GoalKnowledge::Unknown;
        session.codex_goals.revision = session.codex_goals.revision.saturating_add(1);
        session.codex_goals.last_read = None;
        reconcile_ralph(session);
    }
    session.codex_goals.mutation = None;
    session.codex_goals.notice.clear();
    session.codex_goals.error = Some(format!(
        "Goal change was not confirmed: {error}. Draft kept; no automatic retry"
    ));
}

fn request(session: &mut SessionState, method: &str, params: Value) -> GoalRequest {
    session.advance_codex_serial();
    let may_activate = method == "thread/goal/set"
        && params["status"]
            .as_str()
            .or(session.codex_goals.goal["status"].as_str())
            == Some("active");
    GoalRequest {
        live: Arc::new(AtomicBool::new(true)),
        operation: None,
        token: session.codex_serial(),
        method: method.into(),
        params,
        revision: session.codex_goals.revision,
        may_activate,
    }
}

fn context(scope: CodexScope, request: &GoalRequest) -> Value {
    json!({"operation":"goal","epoch":scope.epoch,"token":request.token,
        "timeoutMs":if request.method=="thread/goal/get" {5000} else {15000}})
}

/// Refuse Ralph while a native goal may drive this thread.
pub fn ralph_error(session: &SessionState, scope: CodexScope) -> Option<&'static str> {
    if !scope.enabled {
        return None;
    }
    if scope.closed {
        return Some("Codex is closed; restart before enabling Ralph");
    }
    if session.codex_goals.mutation.is_some() {
        return Some("Wait for the pending Codex goal change before enabling Ralph");
    }
    match session.codex_goals.knowledge {
        GoalKnowledge::Unknown => {
            Some("Codex goal state is unknown; refresh it before enabling Ralph")
        }
        GoalKnowledge::Unavailable => None,
        GoalKnowledge::Known
            if session.codex_goals.goal.is_null()
                || matches!(
                    session.codex_goals.goal["status"].as_str(),
                    Some("paused" | "complete")
                ) =>
        {
            None
        }
        GoalKnowledge::Known => Some("Pause or clear the Codex goal before enabling Ralph"),
    }
}

/// Stop Carlos automatic work while retaining queued human messages.
pub(crate) fn reconcile_ralph(session: &mut SessionState) {
    let may_drive = session.codex_goals.knowledge == GoalKnowledge::Unknown
        || session.codex_goals.knowledge == GoalKnowledge::Known
            && !session.codex_goals.goal.is_null()
            && !matches!(
                session.codex_goals.goal["status"].as_str(),
                Some("paused" | "complete")
            );
    if may_drive && (session.ralph_enabled() || session.ralph_runtime().toggle_pending()) {
        session.ralph_stop_after_failure();
        session.append_message(Role::System,"Carlos stopped Ralph because a Codex goal may drive this thread. Read the goal state; queued human inputs remain.");
    }
}
