//! Apply native browser completion notices to the exact local answer draft.

use serde_json::Value;

use super::state::AppState;

/// Exercise the browser notice command without constructing a native reply in tests.
#[cfg(test)]
pub(super) fn finish_answer(
    app: &mut AppState,
    pending: &super::state::ApprovalRequestDraft,
    choice: super::state::ApprovalChoice,
) {
    let update = app.session.finish_claude_browser_answer(pending, choice);
    app.apply_session_update(update);
}

/// Keep early completion visible while requiring the user's explicit consent.
pub(super) fn complete(app: &mut AppState, message: &Value) {
    let Some((callback_id, update)) = app.session.receive_claude_browser_completion(message) else {
        return;
    };
    if let Some(draft) = callback_id
        .as_deref()
        .and_then(|id| app.view.claude_answers.drafts.get_mut(id))
    {
        draft.notice = Some("The server reported browser completion. Your consent is still required; decline or cancel remains available.".to_string());
    }
    app.apply_session_update(update);
}
