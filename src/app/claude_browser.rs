//! Tracks browser requests before consent and matches native completion notices once.

use serde_json::Value;

use super::claude_question_state::safe_label;
use super::state::{AppState, ApprovalChoice, PendingApprovalRequest};

/// One server request keeps its callback identity apart from consent and completion.
pub(super) struct BrowserRequest {
    callback_id: String,
    consented: bool,
    completed: bool,
}

/// Remember a browser request as soon as Carlos receives its native callback.
pub(super) fn register(app: &mut AppState, id: &str, payload: &Value) {
    let Some(key) = browser_key(payload) else {
        return;
    };
    app.claude.answers.browser.insert(
        key,
        BrowserRequest {
            callback_id: id.to_string(),
            consented: false,
            completed: false,
        },
    );
}

/// Withdraw only browser state belonging to the cancelled native callback.
pub(super) fn withdraw(app: &mut AppState, id: &str) {
    app.claude
        .answers
        .browser
        .retain(|_, request| request.callback_id != id);
}

/// Record the user's answer without treating consent as browser completion.
pub(super) fn finish_answer(
    app: &mut AppState,
    pending: &PendingApprovalRequest,
    choice: ApprovalChoice,
) {
    let Some(payload) = &pending.requested_permissions else {
        return;
    };
    let Some(key) = browser_key(payload) else {
        return;
    };
    if choice != ApprovalChoice::Accept {
        app.claude.answers.browser.remove(&key);
        return;
    }
    let completed = app
        .claude
        .answers
        .browser
        .get_mut(&key)
        .is_some_and(|request| {
            request.consented = true;
            request.completed
        });
    if completed {
        app.claude.answers.browser.remove(&key);
    }
    let state = if completed {
        "The server already reported completion."
    } else {
        "Complete the interaction in your browser; the server has not reported completion."
    };
    app.append_message(
        super::Role::System,
        format!(
            "Browser consent sent to {}. Link: {}. {state}",
            safe_label(&key.0),
            safe_label(payload["url"].as_str().unwrap_or("not supplied"))
        ),
    );
}

/// Keep an early completion visible while requiring the user's explicit consent.
pub(super) fn complete(app: &mut AppState, message: &Value) {
    let (Some(server), Some(id)) = (
        message["mcp_server_name"].as_str(),
        message["elicitation_id"].as_str(),
    ) else {
        return;
    };
    let key = (server.to_string(), id.to_string());
    let Some(request) = app.claude.answers.browser.get_mut(&key) else {
        return;
    };
    if request.completed {
        return;
    }
    request.completed = true;
    if request.consented {
        app.claude.answers.browser.remove(&key);
    } else if let Some(draft) = app.claude.answers.drafts.get_mut(&request.callback_id) {
        draft.notice = Some("The server reported browser completion. Your consent is still required; decline or cancel remains available.".to_string());
    }
    app.append_message(
        super::Role::System,
        format!("Browser interaction completed for {}", safe_label(server)),
    );
}

fn browser_key(payload: &Value) -> Option<(String, String)> {
    if payload["mode"] != "url" {
        return None;
    }
    Some((
        payload["serverName"].as_str()?.to_string(),
        payload["elicitationId"].as_str()?.to_string(),
    ))
}
