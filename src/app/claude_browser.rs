//! Tracks browser requests before consent and matches native completion notices once.

use serde_json::Value;

use super::claude_question_state::safe_label;
use super::state::{AppState, ApprovalChoice, ApprovalRequestDraft};

/// Record the user's answer without treating consent as browser completion.
pub(super) fn finish_answer(
    app: &mut AppState,
    pending: &ApprovalRequestDraft,
    choice: ApprovalChoice,
) {
    let Some(payload) = &pending.requested_permissions else {
        return;
    };
    let Some((server, completed)) = app
        .session
        .answer_claude_browser(payload, choice == ApprovalChoice::Accept)
    else {
        return;
    };
    let state = if completed {
        "The server already reported completion."
    } else {
        "Complete the interaction in your browser; the server has not reported completion."
    };
    app.append_message(
        super::Role::System,
        format!(
            "Browser consent sent to {}. Link: {}. {state}",
            safe_label(&server),
            safe_label(payload["url"].as_str().unwrap_or("not supplied"))
        ),
    );
}

/// Keep an early completion visible while requiring the user's explicit consent.
pub(super) fn complete(app: &mut AppState, message: &Value) {
    let Some(completion) = app.session.complete_claude_browser(message) else {
        return;
    };
    if let Some(draft) = completion
        .callback_id
        .as_deref()
        .and_then(|id| app.claude.answers.drafts.get_mut(id))
    {
        draft.notice = Some("The server reported browser completion. Your consent is still required; decline or cancel remains available.".to_string());
    }
    app.append_message(
        super::Role::System,
        format!(
            "Browser interaction completed for {}",
            safe_label(&completion.server)
        ),
    );
}
