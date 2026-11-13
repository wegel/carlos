//! Record row edits at the only mutable source-text entry points.

use super::Transcript;
use crate::session::entry_id::EntryId;
use crate::session::models::{MessageKind, Role, ToolOutputDetails};
use crate::session::transcript_journal::RowChange;

impl Transcript {
    /// Change entry facts while retaining the allocated id.
    pub fn update(
        &mut self,
        index: usize,
        role: Role,
        text: Option<String>,
        kind: MessageKind,
        path: Option<String>,
    ) {
        if let Some(message) = self
            .entries
            .get(index)
            .filter(|_| index < self.journal.published)
        {
            let same_shape = message.role == role && message.kind == kind;
            let appended = text
                .as_ref()
                .filter(|text| same_shape && text.starts_with(&message.text));
            let delta = appended
                .filter(|text| text.len() > message.text.len())
                .map(|_| message.text.len());
            let replace = text.as_ref().is_some_and(|text| text != &message.text)
                && appended.is_none()
                || !same_shape;
            let ended = !self.active.contains(&index) && delta.is_some();
            if replace || ended {
                self.replace_output(index);
            }
            self.record_change(index, replace, delta, true);
        }
        if let Some(message) = self.entries.get_mut(index) {
            message.role = role;
            if let Some(text) = text {
                message.text = text;
            }
            message.kind = kind;
            message.file_path = path;
        }
    }

    /// Append a delta in constant time apart from growing the entry text.
    pub(crate) fn append_text(&mut self, index: usize, delta: &str, role: Option<Role>) {
        if let Some(message) = self.entries.get(index) {
            let replace =
                message.kind != MessageKind::Plain || role.is_some_and(|role| role != message.role);
            let facts = replace || role.is_some_and(|role| role != message.role);
            let from = message.text.len();
            if replace || !self.active.contains(&index) && !delta.is_empty() {
                self.replace_output(index);
            }
            self.record_change(index, replace, Some(from), facts);
        }
        if let Some(message) = self.entries.get_mut(index) {
            if let Some(role) = role {
                message.role = role;
            }
            if message.kind != MessageKind::Plain {
                message.kind = MessageKind::Plain;
                message.file_path = None;
                message.text.clear();
            }
            message.text.push_str(delta);
        }
    }

    /// Attach source-path facts without changing identity.
    pub(crate) fn set_path(&mut self, index: usize, path: Option<String>) {
        self.record_facts(index);
        self.entries[index].file_path = path;
    }

    /// Update a prompt's native steer classification.
    pub(crate) fn set_kind(&mut self, index: usize, kind: MessageKind) {
        if (self.entries[index].kind == MessageKind::Diff) != (kind == MessageKind::Diff) {
            self.replace_output(index);
        }
        self.record_facts(index);
        self.entries[index].kind = kind;
    }

    /// Attach a tool label and failure state to its original entry.
    pub(crate) fn set_tool(&mut self, index: usize, details: ToolOutputDetails) {
        self.record_facts(index);
        self.entries[index].tool_output = Some(details);
    }

    /// Change only the label of an existing tool output.
    pub(crate) fn set_label(
        &mut self,
        id: EntryId,
        label: String,
        generated: bool,
    ) -> Option<usize> {
        let index = self.position(id)?;
        self.record_facts(index);
        let tool = self.entries[index].tool_output.as_mut()?;
        tool.summary = label;
        tool.generated = generated;
        Some(index)
    }
}

impl Transcript {
    pub(super) fn record_facts(&mut self, index: usize) {
        self.record_change(index, false, None, true);
    }

    fn record_change(&mut self, index: usize, replace: bool, delta: Option<usize>, facts: bool) {
        if let Some(message) = self.entries.get(index) {
            self.journal.record(RowChange {
                id: message.id,
                index,
                replace,
                delta,
                facts,
            });
        }
    }
}
