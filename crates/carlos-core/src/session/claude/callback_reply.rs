//! Send a converted approval answer before retiring its session-owned card.

use serde_json::Value;

use crate::backend::BackendClient;
use crate::session::approval_reply::ApprovalCleared;
use crate::session::approval_state::ApprovalChoice;
use crate::session::entry_id::EntryId;
use crate::session::state::SessionState;
use crate::session::text::safe_text;

/// Return only the transcript and editor changes after a successful native answer.
pub struct ApprovalAnswered {
    pub cleared: ApprovalCleared,
}

impl SessionState {
    /// Receive an answer after the client has converted its draft and confirmed consent.
    pub fn answer_session_approval(
        &mut self,
        client: &dyn BackendClient,
        card: EntryId,
        answer: Value,
        choice: ApprovalChoice,
    ) -> Result<Option<ApprovalAnswered>, String> {
        let Some(pending) = self.approval().pending().filter(|p| p.entry_id() == card) else {
            return Ok(None);
        };
        if let Err(error) = client.respond(&pending.facts().request_id, answer) {
            let error = safe_text(&format!("approval reply failed: {error}"));
            self.set_status(error.clone());
            return Err(error);
        }
        self.finish_pending_claude_browser_answer(choice);
        let cleared = self.clear_pending_session_approval();
        self.set_status(match choice {
            ApprovalChoice::Accept => "approval accepted",
            ApprovalChoice::AcceptForSession => "approval accepted for session",
            ApprovalChoice::Decline => "approval declined",
            ApprovalChoice::Cancel => "approval canceled",
        });
        Ok(Some(ApprovalAnswered { cleared }))
    }
}
