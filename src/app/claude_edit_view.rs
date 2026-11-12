//! Keep unsent edit drafts and format local progress and failure labels.

use std::collections::VecDeque;

use super::state::AppState;
use crate::session::claude::EditProgress;

/// Own only unsent recovery text and the client's failure badge.
#[derive(Default)]
pub(super) struct ClaudeEditView {
    pub(super) failed: bool,
    pub(super) unsent_drafts: VecDeque<String>,
}

/// Give the progress or failure a real status-bar label, including narrow terminals.
pub(super) fn label(app: &AppState, width: usize) -> Option<&'static str> {
    let failed = app.view.claude_edit.failed;
    if app.session.claude_edit_progress().is_none() && !failed {
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
        match app.session.claude_edit_progress() {
            Some(EditProgress::Forking) => "Preparing edited branch…",
            Some(EditProgress::Starting) => "Starting edited branch…",
            _ => "Sending edited prompt…",
        }
    })
}
