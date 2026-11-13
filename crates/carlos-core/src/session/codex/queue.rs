//! Read and change the native queue using plain session commands.

use std::collections::HashSet;
use std::time::Instant;

use serde_json::{json, Value};

use super::queue_state::{QueueOperation, QueueRead};
use crate::backend::BackendClient;
use crate::session::state::SessionState;
use crate::session::text::safe_text;

pub use super::scope::CodexScope;

/// Tell a client which local queue effects to apply once.
#[derive(Default)]
pub struct QueueUpdate {
    pub show: bool,
    pub select_first: bool,
    pub clear_edit: bool,
    pub clear_error: bool,
    pub recovery: Option<String>,
}

/// Start a bounded read only when the current native queue needs one.
pub fn tick(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut QueueUpdate,
    blocked: bool,
) -> bool {
    if blocked
        || !scope.enabled
        || scope.closed
        || session.codex_queue.known
        || session.codex_queue.read.is_some()
        || session.codex_queue.error.is_some()
    {
        return false;
    }
    session.advance_codex_serial();
    let token = session.codex_serial();
    session.codex_queue.read = Some(QueueRead {
        token,
        revision: session.codex_queue.revision,
        started: Instant::now(),
        entries: Vec::new(),
        cursors: HashSet::new(),
        bytes: 0,
    });
    page(client, session, scope, update, token, Value::Null);
    true
}

/// Request one page within the original whole-snapshot deadline.
pub fn page(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut QueueUpdate,
    token: u64,
    cursor: Value,
) {
    let Some(read) = session
        .codex_queue
        .read
        .as_ref()
        .filter(|read| read.token == token)
    else {
        return;
    };
    let remaining = std::time::Duration::from_secs(20).saturating_sub(read.started.elapsed());
    if remaining.is_zero() {
        session.codex_queue.read = None;
        session.codex_queue.error = Some("Queue read exceeded its 20 second deadline".into());
        update.clear_error = true;
        return;
    }
    let params = json!({"threadId":session.thread_id(),"cursor":cursor,"limit":1});
    let mut local = context(scope, token);
    local["timeoutMs"] = json!(remaining.as_millis().min(15000) as u64);
    if let Err(error) = client.request_control_with_context("thread/queue/list", params, local) {
        session.codex_queue.read = None;
        fail(
            session,
            update,
            &format!("Could not read Codex queue: {error}"),
        );
    }
}

/// Retain a submitted write until its exact native acknowledgment arrives.
pub fn mutate(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut QueueUpdate,
    method: &str,
    params: Value,
    submitted: Option<String>,
) -> bool {
    if scope.closed || session.codex_queue.operation.is_some() {
        fail(
            session,
            update,
            "A queue request is still pending, or Codex is closed",
        );
        return false;
    }
    session.advance_codex_serial();
    let token = session.codex_serial();
    let mut identity = params.clone();
    identity
        .as_object_mut()
        .expect("queue params")
        .remove("input");
    let result = client.request_control_with_context(method, params, context(scope, token));
    if let Err(error) = result {
        fail(
            session,
            update,
            &format!("Could not send queue request: {error}"),
        );
        return false;
    }
    session.codex_queue.operation = Some(QueueOperation {
        token,
        method: method.into(),
        params: identity,
        submitted,
    });
    session.codex_queue.error = None;
    update.clear_error = true;
    session.codex_queue.notice = "Queue request pending; Codex has not acknowledged it".into();
    session.codex_queue.invalidate();
    true
}

/// Require an authoritative empty queue before Carlos starts automatic work.
pub fn allows_automatic(session: &SessionState, scope: CodexScope) -> bool {
    !scope.enabled
        || (session.codex_queue.known
            && session.codex_queue.entries.is_empty()
            && session.codex_queue.operation.is_none()
            && !scope.closed)
}

pub(in crate::session) fn fail(
    session: &mut SessionState,
    update: &mut QueueUpdate,
    message: &str,
) {
    session.codex_queue.error = Some(safe_text(message));
    update.clear_error = true;
    update.show = true;
}

fn context(scope: CodexScope, token: u64) -> Value {
    json!({"epoch":scope.epoch,"token":token})
}
