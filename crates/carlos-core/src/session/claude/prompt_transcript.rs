//! Apply accepted prompt UUIDs to transcript and rewind facts without a terminal.

use serde_json::Value;

use crate::session::entry_key::BackendEntryKey;
use crate::session::models::{MessageKind, Role};
use crate::session::state::SessionState;

/// Return caller-local history navigation changes after a consumed input.
#[derive(Default)]
pub struct PromptUpdate {
    pub reset_navigation: bool,
}

impl SessionState {
    /// Attach a local echo to its native UUID before the next SDK frame.
    pub fn attach_claude_prompt_echo(
        &mut self,
        id: &str,
        key: &BackendEntryKey,
        kind: Option<&str>,
    ) {
        let Some(index) = self.messages.len().checked_sub(1) else {
            return;
        };
        self.put_item_mapping(key, index);
        self.record_prompt_turn(self.messages[index].id, id);
        self.classify_claude_prompt(index, kind)
    }

    /// Classify only exact human steering inputs without changing their text or identity.
    pub(crate) fn classify_claude_prompt(&mut self, index: usize, kind: Option<&str>) {
        if self.messages[index].role == Role::User && kind == Some("steer") {
            self.transcript_set_kind(index, MessageKind::UserSteer);
        }
    }

    /// Recreate one surviving accepted prompt only when native Claude consumes it.
    pub fn consume_claude_prompt(&mut self, id: &str, key: &BackendEntryKey) -> PromptUpdate {
        let mut update = PromptUpdate::default();
        let Some(prompt) = self.claude.prompts.entries.get_mut(id) else {
            return update;
        };
        prompt.consumed = true;
        let (text, human, steer, clear) = (
            prompt.text.clone(),
            prompt.human,
            prompt.steer,
            prompt.clear,
        );
        if clear {
            return update;
        }
        if !self.has_item_mapping(key) {
            let index = self.append_message(
                if human { Role::User } else { Role::System },
                if human {
                    text.clone()
                } else {
                    format!("Automatic prompt: {text}")
                },
            );

            self.put_item_mapping(key, index);
            if steer {
                self.classify_claude_prompt(index, Some("steer"));
            }
            if human {
                update.reset_navigation =
                    self.record_prompt_history(&text, Some(self.messages[index].id));
                self.record_prompt_turn(self.messages[index].id, id);
            }
        }
        self.record_turn_started(id);
        update
    }

    /// Adopt a submitted UUID before echo binding, preserving the active-turn guard.
    pub(crate) fn record_submitted_claude_prompt(
        &mut self,
        response: &Value,
        key: &BackendEntryKey,
    ) {
        let Some(id) = response["result"]["turn"]["id"].as_str() else {
            return;
        };
        if self.active_turn_id().as_deref() == Some("claude-turn-pending")
            || self.active_turn_id().is_none()
        {
            self.record_turn_started(id);
        }
        self.attach_claude_prompt_echo(id, key, response["result"]["inputKind"].as_str());
        if let Some(index) = self.messages.len().checked_sub(1) {
            self.record_prompt_turn(self.messages[index].id, id);
        }
    }
}
