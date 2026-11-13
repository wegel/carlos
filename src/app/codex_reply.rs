//! Return Codex callback answers through native responses, with no synthetic turns.

use serde_json::Value;

use super::codex_callbacks::{apply_retired, reply_error, reply_hint};
use super::state::{AppState, ApprovalChoice, ApprovalRequestKind};
use crate::backend::BackendClient;
use crate::session::codex::callback_reply::{CallbackAnswered, CallbackInterrupt};
use crate::session::codex::callbacks::CallbackKey;
use crate::session::entry_id::EntryId;

/// Build only the selected callback's native response and retain drafts on failure.
pub(super) fn respond(client: &dyn BackendClient, app: &mut AppState, choice: ApprovalChoice) {
    let Some(card_id) = app.session.approval().pending().map(|p| p.entry_id()) else {
        return;
    };
    let Some(pending) = app.session.approval_answer(card_id) else {
        return;
    };
    let id = &pending.request_id;
    if choice == ApprovalChoice::Cancel && pending.kind == ApprovalRequestKind::CodexQuestion {
        submit_answer(client, app, card_id, id, Value::Null, choice);
        return;
    }
    let Some(entry) = app.session.codex_callbacks().get(id) else {
        return;
    };
    let Some(draft) =
        CallbackKey::new(id).and_then(|key| app.view.codex_callbacks.drafts.get(&key))
    else {
        return;
    };
    let answer = if choice == ApprovalChoice::Accept {
        match pending.kind {
            ApprovalRequestKind::CodexQuestion => super::codex_questions::question_answer(draft),
            ApprovalRequestKind::CodexForm => {
                super::codex_questions::form_answer(draft, entry.params())
            }
            _ => pending
                .response_for_choice(choice)
                .ok_or_else(|| "This choice is unavailable".into()),
        }
    } else {
        pending
            .response_for_choice(choice)
            .ok_or_else(|| "This choice is unavailable".into())
    };
    let answer = match answer {
        Ok(answer) => answer,
        Err(error) => {
            reply_hint(app, id, &error);
            return;
        }
    };
    submit_answer(client, app, card_id, id, answer, choice);
}

fn submit_answer(
    client: &dyn BackendClient,
    app: &mut AppState,
    card_id: EntryId,
    id: &Value,
    answer: Value,
    choice: ApprovalChoice,
) {
    match app
        .session
        .answer_codex_callback(client, card_id, id, answer, choice)
    {
        Ok(Some(CallbackAnswered::Retired(retired))) => apply_retired(app, retired),
        Ok(Some(CallbackAnswered::Interrupted(result))) => apply_interrupt(app, id, result),
        Ok(None) => {}
        Err(error) => reply_error(app, id, &error),
    }
}

/// Send a worker reply only through its original submitted card token.
pub(super) fn send(
    client: &dyn BackendClient,
    app: &mut AppState,
    card_id: EntryId,
    answer: Value,
) {
    let Some(pending) = app.session.submitted_approval_answer(card_id) else {
        return;
    };
    let id = pending.request_id.clone();
    match app.session.send_codex_callback(client, card_id, answer) {
        Ok(Some(retired)) => apply_retired(app, retired),
        Ok(None) => {}
        Err(error) => reply_error(app, &id, &error),
    }
}

/// Show local progress after the session interrupts the request's own turn.
pub(super) fn interrupt(client: &dyn BackendClient, app: &mut AppState) {
    let Some(id) = app
        .session
        .approval()
        .pending()
        .map(|p| p.facts().request_id.clone())
    else {
        return;
    };
    let result = app.session.interrupt_codex_callback(client, &id);
    apply_interrupt(app, &id, result);
}

fn apply_interrupt(app: &mut AppState, id: &Value, result: CallbackInterrupt) {
    match result {
        CallbackInterrupt::Ignored => {}
        CallbackInterrupt::NoTurn => reply_hint(
            app,
            id,
            "This request has no turn to interrupt; decline or cancel it instead",
        ),
        CallbackInterrupt::Failed(error) => reply_error(app, id, &error),
        CallbackInterrupt::Requested => {
            if let Some(draft) =
                CallbackKey::new(id).and_then(|key| app.view.codex_callbacks.drafts.get_mut(&key))
            {
                draft.notice = Some("Interrupt requested; waiting for native resolution".into());
            }
        }
    }
}
