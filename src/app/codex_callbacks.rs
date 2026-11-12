//! Queue native Codex callbacks before filtering child-thread output from the transcript.

use serde_json::Value;

use super::approval_parsing::pending_approval_from_request;
use super::claude_question_state::{safe_label, safe_text, NativeDraft};
use super::codex_state::{CodexCallback, CodexCallbacks};
use super::notifications::ServerRequestAction;
use super::state::{AppState, ApprovalRequestDraft};
use crate::session::approval_state::ApprovalQueue;

#[path = "codex_callback_route.rs"]
mod routing;

pub(super) use self::routing::{route, CallbackRoute};

/// Keep each callback's original ID, payload, draft and thread until explicitly settled.
pub(super) fn receive(
    app: &mut AppState,
    method: &str,
    frame: &Value,
) -> Option<ServerRequestAction> {
    let id = frame["id"].clone();
    if app.codex.callbacks.contains(&id) {
        return None;
    }
    if !supported_method(method) {
        return Some(reject(id, -32601, "Unsupported native request method"));
    }
    let Some(key) = CodexCallbacks::key(&id) else {
        return Some(reject(id, -32600, "Invalid callback ID"));
    };
    let params = &frame["params"];
    if !params.is_object() {
        return Some(reject(id, -32602, "Callback params must be an object"));
    }
    let bytes = params.to_string().len() + key.len();
    if exceeds_callback_limits(app, bytes) {
        return Some(reject(
            id,
            -32000,
            "Pending callback count or size limit exceeded",
        ));
    }
    let (pending, draft) = match build_request(method, params, id.clone()) {
        Ok(built) => built,
        Err(error) => return Some(reject(id, -32602, &error)),
    };
    let blocking =
        method != "item/tool/requestUserInput" || params["isBlocking"].as_bool().unwrap_or(true);
    app.codex.callbacks.entries.insert(
        key,
        CodexCallback {
            draft,
            params: params.clone(),
            blocking,
            revision: 0,
            candidate: None,
            candidate_card: None,
            checking: false,
            interrupting: false,
            bytes,
        },
    );
    activate(app, pending, blocking);
    if method == "mcpServer/elicitation/request" && params["mode"] == "form" {
        return Some(ServerRequestAction::ValidateCodexForm {
            id,
            schema: params["requestedSchema"].clone(),
            answer: None,
            revision: 0,
        });
    }
    None
}

fn exceeds_callback_limits(app: &AppState, bytes: usize) -> bool {
    let retained = app
        .codex
        .callbacks
        .entries
        .values()
        .map(|entry| entry.bytes)
        .sum::<usize>();
    app.codex.callbacks.entries.len() >= 64
        || bytes > 128 * 1024
        || retained + bytes > 4 * 1024 * 1024
}

fn supported_method(method: &str) -> bool {
    matches!(
        method,
        "item/tool/requestUserInput"
            | "mcpServer/elicitation/request"
            | "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/permissions/requestApproval"
            | "execCommandApproval"
            | "applyPatchApproval"
    )
}

fn build_request(
    method: &str,
    params: &Value,
    id: Value,
) -> Result<(ApprovalRequestDraft, NativeDraft), String> {
    validate_owner(method, params)?;
    let (mut pending, draft) = if matches!(
        method,
        "item/tool/requestUserInput" | "mcpServer/elicitation/request"
    ) {
        super::codex_questions::build(method, params, id).map_err(|error| error.to_string())?
    } else {
        let pending =
            pending_approval_from_request(method, params.as_object().expect("checked params"), id)
                .ok_or_else(|| "Malformed native approval".to_string())?;
        (pending, NativeDraft::from_payload("", &Value::Null))
    };
    if pending.kind != super::state::ApprovalRequestKind::Permissions
        && pending.requested_permissions.is_none()
    {
        pending.requested_permissions = Some(params.clone());
    }
    pending.title = safe_label(&pending.title);
    pending.detail_lines = pending
        .detail_lines
        .iter()
        .map(|line| safe_text(line))
        .collect();
    if let Some(thread) = params["threadId"]
        .as_str()
        .or(params["conversationId"].as_str())
    {
        if !pending
            .detail_lines
            .iter()
            .any(|line| line.starts_with("Thread:"))
        {
            pending
                .detail_lines
                .insert(0, format!("Thread: {}", safe_label(thread)));
        }
    }
    Ok((pending, draft))
}

fn validate_owner(method: &str, params: &Value) -> Result<(), String> {
    let keys: &[&str] = if method == "mcpServer/elicitation/request" {
        &["threadId", "serverName"]
    } else if matches!(method, "execCommandApproval" | "applyPatchApproval") {
        &["conversationId", "callId"]
    } else {
        &["threadId", "turnId", "itemId"]
    };
    for key in keys {
        if params[key].as_str().is_none_or(|value| value.is_empty()) {
            return Err(format!("Callback {key} is missing or invalid"));
        }
    }
    if method == "item/permissions/requestApproval" && !params["permissions"].is_object() {
        return Err("Requested permissions are missing".into());
    }
    if let Some(kind) = params
        .get("kind")
        .filter(|_| method == "item/commandExecution/requestApproval")
    {
        if !matches!(kind.as_str(), Some("command" | "stdin" | "humanCommand")) {
            return Err("Unsupported approval kind".into());
        }
    }
    Ok(())
}

fn activate(app: &mut AppState, pending: ApprovalRequestDraft, blocking: bool) {
    let id = app.session.enqueue_approval(pending, ApprovalQueue::Codex);
    if app.session.approval().pending().is_none() {
        app.session.focus_approval(id, ApprovalQueue::Codex);
        app.codex.callbacks.focused = blocking;
    } else if blocking && !app.codex.callbacks.focused {
        app.session.focus_approval(id, ApprovalQueue::Codex);
        app.codex.callbacks.focused = true;
    }
    app.set_status("Codex has a pending request; F4 answers it");
}

/// Match native resolutions by exact ID and owning thread, including before UI focus.
pub(super) fn resolved(app: &mut AppState, params: &Value) {
    let id = &params["requestId"];
    if let Some(entry) = app.codex.callbacks.get(id) {
        if entry.params["threadId"] != params["threadId"] {
            return;
        }
    }
    retire(app, id);
}

/// Remove an answered/cancelled callback without discarding another request's draft.
pub(super) fn retire(app: &mut AppState, id: &Value) {
    let active = app
        .session
        .approval()
        .pending()
        .is_some_and(|pending| pending.facts().request_id == *id);
    if active {
        app.forget_pending_approval_hints();
    }
    for pending in app.session.approval().queued(ApprovalQueue::Codex) {
        if &pending.facts().request_id == id {
            app.view.approval_hints.remove(pending.entry_id());
        }
    }
    let retired: Vec<_> = app
        .session
        .approval()
        .queued(ApprovalQueue::Codex)
        .iter()
        .filter(|p| p.facts().request_id == *id)
        .filter_map(|p| p.entry_id())
        .collect();
    for entry in retired {
        app.session.retire_approval(entry);
    }
    app.codex.callbacks.retire(id);
    if active {
        app.session.promote_approval(ApprovalQueue::Codex);
        app.codex.callbacks.focused = app.session.approval().pending().is_some_and(|pending| {
            app.codex
                .callbacks
                .get(&pending.facts().request_id)
                .is_some_and(|entry| entry.blocking)
                || CodexCallbacks::key(&pending.facts().request_id)
                    .is_some_and(|key| app.codex.callbacks.opened.contains(&key))
        });
    }
}

/// Cycle between native requests; a hidden nonblocking question opens on the first F4.
pub(super) fn next(app: &mut AppState) {
    if app.codex.callbacks.focused
        && !app
            .session
            .approval()
            .queued(ApprovalQueue::Codex)
            .is_empty()
    {
        let id = app.session.approval().queued(ApprovalQueue::Codex)[0]
            .entry_id()
            .expect("queued card");
        app.session.focus_approval(id, ApprovalQueue::Codex);
    }
    if let Some(key) = app
        .session
        .approval()
        .pending()
        .and_then(|pending| CodexCallbacks::key(&pending.facts().request_id))
    {
        app.codex.callbacks.opened.insert(key);
        app.codex.callbacks.focused = true;
    }
}

/// Restore the main composer only when the visible request is nonblocking.
pub(super) fn hide_nonblocking(app: &mut AppState) -> bool {
    let nonblocking = app
        .session
        .approval()
        .pending()
        .and_then(|pending| app.codex.callbacks.get(&pending.facts().request_id))
        .is_some_and(|entry| !entry.blocking);
    if nonblocking {
        if let Some(key) = app
            .session
            .approval()
            .pending()
            .and_then(|pending| CodexCallbacks::key(&pending.facts().request_id))
        {
            app.codex.callbacks.opened.remove(&key);
        }
        app.codex.callbacks.focused = false;
        if let Some(index) = app
            .session
            .approval()
            .queued(ApprovalQueue::Codex)
            .iter()
            .position(|pending| {
                app.codex
                    .callbacks
                    .get(&pending.facts().request_id)
                    .is_some_and(|entry| entry.blocking)
            })
        {
            let id = app.session.approval().queued(ApprovalQueue::Codex)[index]
                .entry_id()
                .expect("queued card");
            app.session.focus_approval(id, ApprovalQueue::Codex);
            app.codex.callbacks.focused = true;
        }
    }
    nonblocking
}

/// Fence an answer that the user edited while its worker was validating it.
pub(super) fn edited(app: &mut AppState) {
    let Some(id) = app
        .session
        .approval()
        .pending()
        .map(|pending| pending.facts().request_id.clone())
    else {
        return;
    };
    if let Some(entry) = app.codex.callbacks.get_mut(&id) {
        entry.revision = entry.revision.saturating_add(1);
        entry.candidate = None;
        entry.candidate_card = None;
        entry.checking = false;
    }
}

/// Consume a worker result only for the still-live callback and exact answer revision.
pub(super) fn checked(app: &mut AppState, params: &Value) -> Option<ServerRequestAction> {
    let id = &params["requestId"];
    let entry = app.codex.callbacks.get_mut(id)?;
    let revision = params["revision"].as_u64()?;
    if revision == 0 {
        if !entry.checking {
            entry.draft.notice = None;
        }
        if params["ok"] != true {
            entry.draft.unsupported = Some(safe_text(
                params["error"]
                    .as_str()
                    .unwrap_or("Unsupported form schema"),
            ));
        }
        return None;
    }
    if revision != entry.revision {
        return None;
    }
    entry.checking = false;
    entry.draft.notice = None;
    if params["ok"] != true {
        let error = params["error"].as_str().unwrap_or("Form validation failed");
        if revision == 0 {
            entry.draft.unsupported = Some(safe_text(error));
        } else {
            entry.draft.error = Some(safe_text(error));
        }
        entry.candidate = None;
        entry.candidate_card = None;
        return None;
    }
    let result = entry.candidate.take()?;
    let card_id = entry.candidate_card.take()?;
    Some(ServerRequestAction::ReplyNative { card_id, result })
}

/// Keep reply/worker failures beside their own answer field.
pub(super) fn reply_hint(app: &mut AppState, id: &Value, error: &str) {
    if let Some(entry) = app.codex.callbacks.get_mut(id) {
        entry.checking = false;
        entry.draft.error = Some(safe_text(error));
        entry.draft.viewport.scroll.set(0);
    }
    app.set_hint(safe_text(error));
}

/// Keep a failed backend reply as a session fact as well as an editor error.
pub(super) fn reply_error(app: &mut AppState, id: &Value, error: &str) {
    reply_hint(app, id, error);
    app.set_status(safe_text(error));
}

fn reject(id: Value, code: i64, message: &str) -> ServerRequestAction {
    ServerRequestAction::ReplyError {
        request_id: id,
        code,
        message: message.into(),
    }
}

/// Retire every old-session callback before a replacement transcript is activated.
pub(super) fn reset(app: &mut AppState) {
    let ids = app
        .codex
        .callbacks
        .entries
        .keys()
        .map(|key| serde_json::from_str::<Value>(key).expect("typed callback ID"))
        .collect::<Vec<_>>();
    for id in ids {
        retire(app, &id);
    }
    app.codex.callbacks.focused = false;
}
