//! Keep local Claude answer editors beside session-owned callback cards.

use serde_json::Value;

use super::claude_question_state::NativeDraft;
use super::state::AppState;
use crate::session::claude::callbacks::ClaudeCallback;

/// Create an editor only for a newly accepted native callback.
pub(super) fn receive_callback(app: &mut AppState, frame: &Value) {
    match app.session.receive_claude_callback(frame) {
        ClaudeCallback::Ignored => {}
        ClaudeCallback::Unsupported(update) => app.apply_session_update(update),
        ClaudeCallback::Accepted {
            id,
            card,
            description,
            retired,
        } => {
            let draft = NativeDraft::from_description(description);
            let has_fields = !draft.fields.is_empty();
            app.view.claude_answers.drafts.insert(id, draft);
            app.drop_retired_approval_hints(retired);
            app.view.approval_hints.register_callback(card, has_fields);
        }
    }
}

/// Drop exactly the cancelled editor, consent key and retired card hints.
pub(super) fn cancel_callback(app: &mut AppState, id: &Value) {
    if let Some(id) = id.as_str() {
        app.view.claude_answers.drafts.remove(id);
    }
    if app.view.confirm_allow.is_some_and(|entry| {
        app.live_approval_cards()
            .any(|p| p.entry_id() == entry && &p.facts().request_id == id)
    }) {
        app.view.confirm_allow = None;
    }
    let (retired, cleared) = app.session.cancel_claude_callback(id);
    app.drop_retired_approval_hints(retired);
    if let Some(cleared) = cleared {
        app.apply_cleared_approval(cleared);
    }
}
