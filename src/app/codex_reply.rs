//! Return Codex callback answers through native responses, with no synthetic turns.

use serde_json::{json, Value};

use super::codex_callbacks::{reply_error, retire};
use super::state::{AppState, ApprovalChoice, ApprovalRequestKind};
use crate::backend::BackendClient;

/// Build only the selected callback's native response and retain drafts on failure.
pub(super) fn respond(client: &dyn BackendClient, app: &mut AppState, choice: ApprovalChoice) {
    let Some(pending) = app.approval.pending.clone() else {
        return;
    };
    let id = &pending.request_id;
    if choice == ApprovalChoice::Cancel && pending.kind == ApprovalRequestKind::CodexQuestion {
        interrupt(client, app);
        return;
    }
    let Some(entry) = app.codex.callbacks.get(id) else {
        return;
    };
    let answer = if choice == ApprovalChoice::Accept {
        match pending.kind {
            ApprovalRequestKind::CodexQuestion => {
                super::codex_questions::question_answer(&entry.draft)
            }
            ApprovalRequestKind::CodexForm => {
                super::codex_questions::form_answer(&entry.draft, &entry.params)
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
            reply_error(app, id, &error);
            return;
        }
    };
    if pending.kind == ApprovalRequestKind::CodexForm
        && answer["action"] == "accept"
        && entry.params["mode"] == "form"
    {
        validate_answer(client, app, id, &answer);
        return;
    }
    send(client, app, id, answer);
}

fn validate_answer(client: &dyn BackendClient, app: &mut AppState, id: &Value, answer: &Value) {
    let entry = app.codex.callbacks.get_mut(id).expect("active callback");
    if entry.checking {
        return;
    }
    entry.revision = entry.revision.saturating_add(1);
    entry.candidate = Some(answer.clone());
    entry.checking = true;
    let result = client.validate_form(
        id.clone(),
        entry.params["requestedSchema"].clone(),
        Some(answer["content"].clone()),
        entry.revision,
    );
    if let Err(error) = result {
        reply_error(app, id, &error.to_string());
    }
}

/// Settle exactly one callback after its native pipe write succeeds.
pub(super) fn send(client: &dyn BackendClient, app: &mut AppState, id: &Value, answer: Value) {
    match client.respond(id, answer) {
        Ok(()) => {
            retire(app, id);
            app.set_status("Answer sent");
        }
        Err(error) => reply_error(app, id, &format!("Could not send answer: {error}")),
    }
}

/// Interrupt the request's own turn; an acknowledgment does not resolve its callback.
pub(super) fn interrupt(client: &dyn BackendClient, app: &mut AppState) {
    let Some(id) = app
        .approval
        .pending
        .as_ref()
        .map(|pending| pending.request_id.clone())
    else {
        return;
    };
    let Some(entry) = app.codex.callbacks.get(&id) else {
        return;
    };
    if entry.interrupting {
        return;
    }
    let params = json!({"threadId":entry.params["threadId"],"turnId":entry.params["turnId"]});
    if !params["threadId"].is_string() || !params["turnId"].is_string() {
        reply_error(
            app,
            &id,
            "This request has no turn to interrupt; decline or cancel it instead",
        );
        return;
    }
    match client.request_native_control("turn/interrupt", params) {
        Ok(()) => {
            super::codex_callbacks::edited(app);
            if let Some(entry) = app.codex.callbacks.get_mut(&id) {
                entry.interrupting = true;
                entry.draft.notice =
                    Some("Interrupt requested; waiting for native resolution".into());
            }
        }
        Err(error) => reply_error(app, &id, &error.to_string()),
    }
}

/// Report an exact-thread interrupt failure without dismissing any live request.
pub(super) fn control_reply(app: &mut AppState, params: &Value) {
    if params["requestMethod"] != "turn/interrupt" || params["response"]["error"].is_null() {
        return;
    }
    let target = &params["requestParams"];
    for entry in app.codex.callbacks.entries.values_mut() {
        if entry.params["threadId"] == target["threadId"]
            && entry.params["turnId"] == target["turnId"]
        {
            entry.interrupting = false;
            entry.draft.notice = None;
            entry.draft.error = Some(format!(
                "Codex could not interrupt: {}. The request is still pending",
                super::claude_question_state::safe_text(
                    params["response"]["error"]["message"]
                        .as_str()
                        .unwrap_or("unknown error")
                )
            ));
        }
    }
}
