//! Show asynchronous Claude edits and keep failed text separate from a newer composer draft.

use serde_json::Value;

use super::claude_state::ClaudePhase;
use super::models::Role;
use super::state::AppState;

/// Keep the edit's wire submission separate from helper readiness and user typing.
#[derive(Clone)]
pub(super) enum EditState {
    Forking,
    Starting,
    Sending {
        text: String,
        queued: bool,
        uuid: Option<String>,
    },
    Failed,
}

/// Keep the selected edit until the fork and replacement startup are both ready.
pub(super) fn begin(app: &mut AppState, text: String) {
    app.claude.phase = ClaudePhase::Starting;
    app.claude.history.edited_prompt = Some(text);
    app.claude.history.edit = Some(EditState::Forking);
    retire_queue(app);
    app.viewport.auto_follow_bottom = true;
}

/// Keep a newer composer unchanged until the saved edit has actually been submitted.
pub(super) fn waiting(app: &AppState) -> bool {
    matches!(
        app.claude.history.edit,
        Some(EditState::Forking | EditState::Starting | EditState::Sending { .. })
    )
}

/// Give the progress or failure a real status-bar label, including narrow terminals.
pub(super) fn label(app: &AppState, width: usize) -> Option<&'static str> {
    let failed = matches!(app.claude.history.edit, Some(EditState::Failed));
    if app.claude.history.edit.is_none() {
        return super::claude_session_history::has_unsent(app).then_some(if width < 30 {
            "F5 drafts"
        } else {
            "F5: saved drafts"
        });
    }
    Some(if width < 30 {
        if failed {
            "Edit failed"
        } else {
            "Editing…"
        }
    } else if failed {
        "Edit failed · draft kept"
    } else {
        match app.claude.history.edit {
            Some(EditState::Forking) => "Preparing edited branch…",
            Some(EditState::Starting) => "Starting edited branch…",
            _ => "Sending edited prompt…",
        }
    })
}

/// Restore a failed edit without reopening a closed helper or replacing a newer draft.
pub(super) fn failed(app: &mut AppState, message: &str) {
    super::claude_session_history::restore_edit(app);
    if app.claude.phase != ClaudePhase::Closed {
        app.claude.phase = ClaudePhase::Ready;
    }
    unavailable(app, message);
}

/// Explain an unavailable rewind without changing whether the helper is ready or closed.
pub(super) fn unavailable(app: &mut AppState, message: &str) {
    app.claude.history.edit = Some(EditState::Failed);
    let message = super::claude_question_state::safe_text(message);
    app.append_message(
        Role::System,
        format!("Could not edit Claude message: {message}. Draft kept."),
    );
    app.viewport.auto_follow_bottom = true;
}

/// Remember which queued text is owned by this edit until its actual SDK acknowledgement.
pub(super) fn queued(app: &mut AppState, text: &str) {
    app.claude.history.edit = Some(EditState::Sending {
        text: text.to_string(),
        queued: true,
        uuid: None,
    });
}

/// Move recovery ownership from the local queue to the edit before dispatching it.
pub(super) fn dispatching(app: &mut AppState, text: &str, origin: crate::backend::PromptOrigin) {
    if let Some(EditState::Sending {
        text: expected,
        queued,
        ..
    }) = app.claude.history.edit.as_mut()
    {
        if origin == crate::backend::PromptOrigin::Human && expected == text {
            *queued = false;
        }
    }
}

/// Bind success to the UUID returned by this exact human submission, never to echoed prose.
pub(super) fn bind_submission(
    app: &mut AppState,
    text: &str,
    origin: crate::backend::PromptOrigin,
    response: &Value,
) {
    if let Some(EditState::Sending {
        text: expected,
        uuid,
        queued: false,
    }) = app.claude.history.edit.as_mut()
    {
        if origin == crate::backend::PromptOrigin::Human && expected == text {
            *uuid = response["result"]["turn"]["id"]
                .as_str()
                .map(str::to_string)
                .or_else(|| response["turn"]["id"].as_str().map(str::to_string));
        }
    }
}

/// Recover an unacknowledged dispatched edit; an undispatched edit is still owned by the queue.
pub(super) fn recover_sending(app: &mut AppState) {
    if let Some(EditState::Sending {
        text,
        queued: false,
        ..
    }) = &app.claude.history.edit
    {
        app.claude.history.edited_prompt = Some(text.clone());
    }
}

/// Recover the dequeued edit after a send failure while preserving later user typing.
pub(super) fn send_failed(app: &mut AppState, text: &str, message: &str) -> bool {
    if !matches!(&app.claude.history.edit, Some(EditState::Sending { text: expected, .. }) if expected == text)
    {
        return false;
    }
    app.claude.history.edited_prompt = Some(text.to_string());
    failed(app, message);
    true
}

/// Clear progress only for the edit's acknowledged SDK UUID, or dismiss an old failure on a later send.
pub(super) fn accepted(app: &mut AppState, id: &str) {
    if matches!(app.claude.history.edit, Some(EditState::Failed)) {
        app.claude.history.edit = None;
    } else if matches!(&app.claude.history.edit, Some(EditState::Sending { uuid: Some(uuid), .. }) if uuid == id)
    {
        app.claude.history.edit = None;
        app.append_message(Role::System, "Claude started the edited branch".to_string());
    }
}

/// Keep old local work off the new branch while retaining every human draft for explicit recovery.
pub(super) fn retire_queue(app: &mut AppState) {
    app.ralph_runtime.stop_after_failure();
    super::claude_session_history::retain_queued(app);
}
