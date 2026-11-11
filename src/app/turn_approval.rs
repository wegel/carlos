//! Answers native and app-server approvals without creating synthetic user prompts.

use super::claude_question_view::default_no_hint;
use super::state::{AppState, ApprovalChoice, ApprovalRequestKind};
use crate::backend::BackendClient;

/// Return the selected answer through the backend native callback.
pub(super) fn respond_to_pending_approval(
    client: &dyn BackendClient,
    app: &mut AppState,
    choice: ApprovalChoice,
) {
    if app.codex.enabled
        && app
            .approval
            .pending
            .as_ref()
            .is_some_and(|pending| app.codex.callbacks.get(&pending.request_id).is_some())
    {
        super::codex_reply::respond(client, app, choice);
        return;
    }
    let Some(pending) = app.approval.pending.clone() else {
        return;
    };
    let Some(result) = build_answer(app, &pending, choice) else {
        return;
    };
    if confirm_default_no(app, &pending, choice) {
        return;
    }

    match client.respond(&pending.request_id, result) {
        Ok(_) => {
            super::claude_browser::finish_answer(app, &pending, choice);
            app.claude.confirm_allow = None;
            app.clear_pending_approval();
            let status = match choice {
                ApprovalChoice::Accept => "approval accepted",
                ApprovalChoice::AcceptForSession => "approval accepted for session",
                ApprovalChoice::Decline => "approval declined",
                ApprovalChoice::Cancel => "approval canceled",
            };
            app.set_status(status);
        }
        Err(err) => reply_error(
            app,
            &pending.request_id,
            format!("approval reply failed: {err}"),
        ),
    }
}

fn build_answer(
    app: &mut AppState,
    pending: &super::state::PendingApprovalRequest,
    choice: ApprovalChoice,
) -> Option<serde_json::Value> {
    let result = if choice == ApprovalChoice::Accept && is_native(pending.kind) {
        let draft = pending
            .request_id
            .as_str()
            .and_then(|id| app.claude.answers.drafts.get(id));
        match draft
            .zip(pending.requested_permissions.as_ref())
            .map(|(draft, payload)| draft.answer(payload))
        {
            Some(Ok(answer)) => Some(answer),
            Some(Err(error)) => {
                reply_error(app, &pending.request_id, error);
                return None;
            }
            None => pending.response_for_choice(choice),
        }
    } else {
        pending.response_for_choice(choice)
    };
    let Some(result) = result else {
        app.set_status("approval choice not available");
        return None;
    };
    Some(result)
}

fn confirm_default_no(
    app: &mut AppState,
    pending: &super::state::PendingApprovalRequest,
    choice: ApprovalChoice,
) -> bool {
    if choice == ApprovalChoice::Accept
        && pending
            .requested_permissions
            .as_ref()
            .is_some_and(|payload| payload["defaultToNo"] == true)
        && app.claude.confirm_allow.as_deref() != pending.request_id.as_str()
    {
        app.claude.confirm_allow = pending.request_id.as_str().map(str::to_string);
        let has_fields = pending
            .request_id
            .as_str()
            .and_then(|id| app.claude.answers.drafts.get(id))
            .is_some_and(|draft| !draft.fields.is_empty());
        let hint = default_no_hint(has_fields, true);
        if let Some(prompt) = app.approval.pending.as_mut() {
            prompt.detail_lines.insert(0, hint.to_string());
        }
        app.set_status(hint);
        return true;
    }

    false
}

fn is_native(kind: ApprovalRequestKind) -> bool {
    matches!(
        kind,
        ApprovalRequestKind::ClaudeNative | ApprovalRequestKind::ClaudeQuestion
    )
}

fn reply_error(app: &mut AppState, id: &serde_json::Value, error: String) {
    if let Some(draft) = id
        .as_str()
        .and_then(|id| app.claude.answers.drafts.get_mut(id))
    {
        draft.error = Some(super::claude_question_state::safe_text(&error));
        draft.viewport.scroll.set(0);
    }
    app.set_status(super::claude_question_state::safe_text(&error));
}
