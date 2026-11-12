//! Keep submitted branch text and its exact acknowledgement UUID with the session.

use serde_json::Value;

use super::state::ClaudePhase;
use crate::backend::PromptOrigin;
use crate::session::state::SessionState;

/// Describe an accepted fork or a prompt waiting for its SDK acknowledgement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditProgress {
    Forking,
    Starting,
    Sending,
}

#[derive(Default, Debug)]
pub(in crate::session) struct ClaudeEdit {
    prompt: Option<String>,
    request: Option<EditRequest>,
}

#[derive(Debug)]
enum EditRequest {
    Forking,
    Starting,
    Sending {
        text: String,
        queued: bool,
        uuid: Option<String>,
    },
}

impl SessionState {
    /// Read progress without exposing submitted text to a mutable client.
    pub(crate) fn claude_edit_progress(&self) -> Option<EditProgress> {
        self.claude
            .edit
            .request
            .as_ref()
            .map(|request| match request {
                EditRequest::Forking => EditProgress::Forking,
                EditRequest::Starting => EditProgress::Starting,
                EditRequest::Sending { .. } => EditProgress::Sending,
            })
    }

    /// Retain the selected edit after the backend accepts the fork request.
    pub(crate) fn begin_claude_edit(&mut self, text: String) {
        self.claude.phase = ClaudePhase::Starting;
        self.claude.edit.prompt = Some(text);
        self.claude.edit.request = Some(EditRequest::Forking);
    }

    /// Move branch text across the existing helper reset without retaining send progress.
    pub(crate) fn resume_claude_edit(&mut self, text: Option<String>) {
        self.claude.edit.request = text.as_ref().map(|_| EditRequest::Starting);
        self.claude.edit.prompt = text;
    }

    /// Return saved branch text once to its queue or to the client's recovery drafts.
    pub(crate) fn take_claude_edit_prompt(&mut self) -> Option<String> {
        self.claude.edit.prompt.take()
    }

    /// Track the queued edit until its exact human submission is dispatched.
    pub(crate) fn queue_claude_edit(&mut self, text: &str) {
        self.claude.edit.request = Some(EditRequest::Sending {
            text: text.to_string(),
            queued: true,
            uuid: None,
        });
    }

    /// Move recovery responsibility from the queue only for matching human text.
    pub(crate) fn dispatch_claude_edit(&mut self, text: &str, origin: PromptOrigin) {
        if let Some(EditRequest::Sending {
            text: expected,
            queued,
            ..
        }) = self.claude.edit.request.as_mut()
        {
            if origin == PromptOrigin::Human && expected == text {
                *queued = false;
            }
        }
    }

    /// Bind only dispatched matching human text to the returned native UUID.
    pub(crate) fn bind_claude_edit(&mut self, text: &str, origin: PromptOrigin, response: &Value) {
        if let Some(EditRequest::Sending {
            text: expected,
            uuid,
            queued: false,
        }) = self.claude.edit.request.as_mut()
        {
            if origin == PromptOrigin::Human && expected == text {
                *uuid = response["result"]["turn"]["id"]
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| response["turn"]["id"].as_str().map(str::to_string));
            }
        }
    }

    /// Recover dispatched text; a still-queued edit remains the queue's responsibility.
    pub(crate) fn recover_sending_claude_edit(&mut self) {
        if let Some(EditRequest::Sending {
            text,
            queued: false,
            ..
        }) = &self.claude.edit.request
        {
            self.claude.edit.prompt = Some(text.clone());
        }
    }

    /// Save failed text only when it matches the current outgoing edit.
    pub(crate) fn reject_claude_edit_send(&mut self, text: &str) -> bool {
        if !matches!(&self.claude.edit.request,
            Some(EditRequest::Sending { text: expected, .. }) if expected == text)
        {
            return false;
        }
        self.claude.edit.prompt = Some(text.to_string());
        true
    }

    /// Finish only the edited prompt's acknowledged UUID and report its notice.
    pub(crate) fn accept_claude_edit(&mut self, id: &str) -> bool {
        if !matches!(&self.claude.edit.request,
            Some(EditRequest::Sending { uuid: Some(uuid), .. }) if uuid == id)
        {
            return false;
        }
        self.claude.edit.request = None;
        true
    }

    /// End progress while retaining any saved text until the client takes it.
    pub(crate) fn end_claude_edit(&mut self) {
        self.claude.edit.request = None;
    }

    /// Return queued human drafts in order after removing matching unbound history tails.
    pub(crate) fn retain_claude_queued_drafts(&mut self) -> (Vec<String>, bool) {
        let drafts = self.ralph_take_queued_human_drafts();
        let mut reset_navigation = false;
        for text in drafts.iter().rev() {
            reset_navigation |= self.discard_pending_prompt(text);
        }
        (drafts, reset_navigation)
    }

    /// Drop branch text and progress when a native reset drops history.
    pub(crate) fn clear_claude_edit(&mut self) {
        self.claude.edit = ClaudeEdit::default();
    }
}

#[cfg(test)]
impl SessionState {
    /// Inspect submitted fork text without taking recovery ownership.
    pub(crate) fn claude_edit_prompt_for_test(&self) -> Option<&str> {
        self.claude.edit.prompt.as_deref()
    }
}
