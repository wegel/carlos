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
    if app.session.codex_scope().enabled()
        && app.session.approval().pending().is_some_and(|pending| {
            app.session
                .codex_callbacks()
                .get(&pending.facts().request_id)
                .is_some()
        })
    {
        super::codex_reply::respond(client, app, choice);
        return;
    }
    let Some(card_id) = app.session.approval().pending().map(|p| p.entry_id()) else {
        return;
    };
    let Some(pending) = app.session.approval_answer(card_id) else {
        return;
    };
    let Some(result) = build_answer(app, &pending, choice) else {
        return;
    };
    if confirm_default_no(app, card_id, &pending, choice) {
        return;
    }

    match app
        .session
        .answer_session_approval(client, card_id, result, choice)
    {
        Ok(Some(answered)) => {
            app.apply_session_update(answered.transcript);
            app.view.confirm_allow = None;
            app.apply_cleared_approval(answered.cleared);
        }
        Ok(None) => {}
        Err(error) => reply_error(app, &pending.request_id, error, true),
    }
}

fn build_answer(
    app: &mut AppState,
    pending: &super::state::ApprovalRequestDraft,
    choice: ApprovalChoice,
) -> Option<serde_json::Value> {
    let result = if choice == ApprovalChoice::Accept && is_native(pending.kind) {
        let draft = pending
            .request_id
            .as_str()
            .and_then(|id| app.view.claude_answers.drafts.get(id));
        match draft
            .zip(pending.requested_permissions.as_ref())
            .map(|(draft, payload)| draft.answer(payload))
        {
            Some(Ok(answer)) => Some(answer),
            Some(Err(error)) => {
                reply_error(app, &pending.request_id, error, false);
                return None;
            }
            None => pending.response_for_choice(choice),
        }
    } else {
        pending.response_for_choice(choice)
    };
    let Some(result) = result else {
        app.set_hint("approval choice not available");
        return None;
    };
    Some(result)
}

fn confirm_default_no(
    app: &mut AppState,
    card_id: crate::session::entry_id::EntryId,
    pending: &super::state::ApprovalRequestDraft,
    choice: ApprovalChoice,
) -> bool {
    if choice == ApprovalChoice::Accept
        && pending
            .requested_permissions
            .as_ref()
            .is_some_and(|payload| payload["defaultToNo"] == true)
        && app.view.confirm_allow != Some(card_id)
    {
        app.view.confirm_allow = Some(card_id);
        let has_fields = pending
            .request_id
            .as_str()
            .and_then(|id| app.view.claude_answers.drafts.get(id))
            .is_some_and(|draft| !draft.fields.is_empty());
        let hint = default_no_hint(has_fields, true);
        app.prepend_approval_hint(hint);
        app.set_hint(hint);
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

fn reply_error(app: &mut AppState, id: &serde_json::Value, error: String, backend: bool) {
    if let Some(draft) = id
        .as_str()
        .and_then(|id| app.view.claude_answers.drafts.get_mut(id))
    {
        draft.error = Some(super::claude_question_state::safe_text(&error));
        draft.viewport.scroll.set(0);
    }
    let error = super::claude_question_state::safe_text(&error);
    if !backend {
        app.set_hint(error);
    }
}
