//! Read source entries as neutral client facts without terminal formatting.

use carlos_proto::events::EntryFacts;
use carlos_proto::session::{Entry, EntryKind, LabelSource, Tool, ToolKind, ToolStatus};

use super::models::{Message, MessageKind, Role};
use super::state::SessionState;

pub(super) fn is_tool(message: &Message) -> bool {
    matches!(message.role, Role::ToolCall | Role::ToolOutput) || message.kind == MessageKind::Diff
}

impl SessionState {
    pub(super) fn export_entry(&self, message: &Message) -> Entry {
        let facts = self.export_entry_facts(message);
        Entry {
            id: message.id.to_wire(),
            kind: facts.kind,
            text: if is_tool(message) {
                String::new()
            } else {
                message.text.clone()
            },
            done: facts.done,
            tool: facts.tool,
            client_message_id: None,
        }
    }

    pub(super) fn export_entry_facts(&self, message: &Message) -> EntryFacts {
        let kind = match message.role {
            Role::User => EntryKind::User,
            Role::Assistant => EntryKind::Answer,
            Role::Commentary => EntryKind::Commentary,
            Role::Reasoning => EntryKind::Reasoning,
            Role::ToolCall | Role::ToolOutput => EntryKind::Tool,
            Role::System => EntryKind::Notice,
        };
        let done = self.messages.is_complete(message.id);
        let tool = is_tool(message).then(|| export_tool(message, done));
        EntryFacts { kind, done, tool }
    }
}

fn export_tool(message: &Message, done: bool) -> Tool {
    let details = message.tool_output.as_ref();
    let label = details
        .map(|tool| tool.summary.as_str())
        .unwrap_or_else(|| message.text.lines().next().unwrap_or(""));
    let kind = if message.kind == MessageKind::Diff {
        ToolKind::Edit
    } else if message.file_path.is_some() {
        ToolKind::Read
    } else {
        ToolKind::Other
    };
    let status = if !done {
        ToolStatus::Running
    } else if details.is_some_and(|tool| tool.failed) {
        ToolStatus::Failed
    } else {
        ToolStatus::Unknown
    };
    Tool {
        kind,
        label: label.to_owned(),
        label_source: if details.is_some_and(|tool| tool.generated) {
            LabelSource::Ai
        } else {
            LabelSource::Literal
        },
        status,
        summary: None,
        added: None,
        removed: None,
        has_output: message.role == Role::ToolOutput && !message.text.is_empty(),
    }
}
