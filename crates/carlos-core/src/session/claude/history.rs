//! Retain normalized history records, prompt sources, and ordered fact fragments.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::prompt_transcript::PromptUpdate;
use crate::claude_backend::FactTransfer;
use crate::session::entry_id::EntryId;
use crate::session::entry_key::BackendEntryKey;
use crate::session::models::Role;
use crate::session::state::SessionState;

/// Retain source metadata independently of the client's unknown-prompt cursor.
#[derive(Default)]
pub struct ClaudeHistory {
    records: Vec<Value>,
    unknown: Vec<UnknownPrompt>,
    transfer: FactTransfer,
}

/// Identify a legacy prompt without guessing whether a human wrote it.
#[derive(Debug)]
pub struct UnknownPrompt {
    uuid: String,
    text: String,
    entry_id: EntryId,
}

impl UnknownPrompt {
    /// Borrow the native anchor used to fork before this prompt.
    pub fn uuid(&self) -> &str {
        &self.uuid
    }

    /// Borrow the original prompt text for explicit local editing.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Read the stable transcript entry behind this source record.
    pub fn entry_id(&self) -> EntryId {
        self.entry_id
    }
}

impl ClaudeHistory {
    /// Borrow every original record, including hidden child metadata.
    #[cfg(any(test, feature = "test-support"))]
    pub fn records(&self) -> &[Value] {
        &self.records
    }

    /// Borrow legacy prompt anchors without exposing mutable facts.
    pub fn unknown(&self) -> &[UnknownPrompt] {
        &self.unknown
    }
}

impl SessionState {
    /// Borrow normalized history sources and legacy prompt anchors.
    pub fn claude_history(&self) -> &ClaudeHistory {
        &self.claude.history
    }

    /// Keep the backend's decoded record without translating native messages here.
    pub(crate) fn record_claude_history(&mut self, record: Value) {
        self.claude.history.records.push(record);
    }

    /// Assemble one normalized fact; the caller must route the completed fact again.
    pub(crate) fn receive_claude_history_part(&mut self, params: &Value) -> Result<Option<Value>> {
        self.claude.history.transfer.receive(params)
    }

    /// Validate metadata and choose the next stable history page or native startup.
    pub fn claude_history_control(
        &mut self,
        method: &str,
        result: &Value,
    ) -> Result<(&'static str, Value)> {
        if method == "sessions.info" {
            if result["sessionId"].as_str() != Some(self.thread_id().as_str()) {
                bail!("The requested Claude session was not found");
            }
            return Ok(("sessions.read", json!({"sessionId":self.thread_id()})));
        }
        if let Some(offset) = result["nextOffset"].as_u64() {
            return Ok((
                "sessions.read",
                json!({"sessionId":self.thread_id(),"offset":offset,
                "snapshotId":result["snapshotId"], "chunkOffset":result["chunkOffset"].as_u64().unwrap_or(0)}),
            ));
        }
        self.set_context_usage(None);
        Ok(("start", json!({"sessionId":self.thread_id()})))
    }

    /// Append only the backend's classified source, suppressing an already bound UUID.
    pub fn append_claude_history_prompt(
        &mut self,
        facts: &Value,
        key: &BackendEntryKey,
    ) -> PromptUpdate {
        let source = facts["source"].as_str().unwrap_or("unknown");
        let kind = facts["inputKind"].as_str();
        let uuid = facts["uuid"].as_str().unwrap_or("");
        let text = facts["text"].as_str().unwrap_or("").to_string();
        let mut update = PromptUpdate::default();
        if !uuid.is_empty() && self.has_item_mapping(key) {
            return update;
        }
        let start = self.messages.len();
        match source {
            "human" => {
                let (index, appended) = self.append_message(Role::User, text.clone());
                update.appended = appended;
                update.classified = self.classify_claude_prompt(index, kind);
                update.reset_navigation =
                    self.record_prompt_history(&text, Some(self.messages[index].id));
                self.record_prompt_turn(self.messages[index].id, uuid);
            }
            "unknown" => {
                let (index, appended) =
                    self.append_message(Role::System, format!("[Source unknown] {text}"));
                update.appended = appended;
                self.claude.history.unknown.push(UnknownPrompt {
                    uuid: uuid.to_string(),
                    text,
                    entry_id: self.messages[index].id,
                });
            }
            "automatic" => {
                update.appended = self
                    .append_message(Role::System, format!("Automatic prompt: {text}"))
                    .1;
            }
            _ => {}
        }
        if self.messages.len() > start && !uuid.is_empty() {
            self.put_item_mapping(key, start);
        }
        update
    }
}

#[cfg(any(test, feature = "test-support"))]
impl std::fmt::Debug for ClaudeHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeHistory")
            .field("records", &self.records)
            .field("unknown", &self.unknown)
            .finish_non_exhaustive()
    }
}
