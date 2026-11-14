//! Read source entries as neutral client facts without terminal formatting.

use carlos_proto::events::EntryFacts;
use carlos_proto::output::OutputRevision;
use carlos_proto::session::{
    Entry, EntryFormat, EntryKind, LabelSource, Tool, ToolKind, ToolStatus,
};

use super::models::{Message, MessageKind, Role};
use super::state::SessionState;

pub(super) fn is_tool(message: &Message) -> bool {
    matches!(message.role, Role::ToolCall | Role::ToolOutput) || message.kind == MessageKind::Diff
}

/// Budget escaped text conservatively before cloning a newest/older wire page.
pub(super) fn page_start(rows: &[Message], limit: usize) -> usize {
    let mut start = rows.len();
    let mut bytes = 0usize;
    for message in rows.iter().rev().take(limit.clamp(1, 1000)) {
        let size = wire_row_bytes(message);
        if start < rows.len() && size > (4 * 1024 * 1024usize).saturating_sub(bytes) {
            break;
        }
        bytes = bytes.saturating_add(size);
        start -= 1;
    }
    start
}

pub(super) fn wire_row_bytes(row: &Message) -> usize {
    let source = if !is_tool(row) && row.text.len() <= carlos_proto::source::INLINE_SOURCE_BYTES {
        row.text.len()
    } else {
        0
    };
    let label = row
        .tool_output
        .as_ref()
        .map(|tool| tool.summary.len())
        .unwrap_or_else(|| {
            if row.role == Role::ToolCall {
                row.text.lines().next().map_or(0, str::len)
            } else {
                0
            }
        });
    source
        .saturating_add(label)
        .saturating_add(row.file_path.as_ref().map_or(0, String::len))
        .saturating_mul(6)
        .saturating_add(1024)
}

impl SessionState {
    pub(super) fn export_entry(&self, message: &Message) -> Entry {
        let facts = self.export_entry_facts(message);
        Entry {
            id: message.id.to_wire(),
            kind: facts.kind,
            format: facts.format,
            file_path: facts.file_path,
            text: if is_tool(message)
                || message.text.len() > carlos_proto::source::INLINE_SOURCE_BYTES
            {
                String::new()
            } else {
                message.text.clone()
            },
            source: (!is_tool(message)
                && message.text.len() > carlos_proto::source::INLINE_SOURCE_BYTES)
                .then(|| self.export_source(message)),
            done: facts.done,
            tool: facts.tool,
            operation: self.entry_operation(message.id),
        }
    }

    pub(super) fn export_entry_facts(&self, message: &Message) -> EntryFacts {
        let kind = match message.role {
            Role::User => EntryKind::User,
            Role::Assistant => EntryKind::Answer,
            Role::Commentary => EntryKind::Commentary,
            Role::Reasoning => EntryKind::Reasoning,
            Role::ToolCall => EntryKind::ToolCall,
            Role::ToolOutput => EntryKind::Tool,
            Role::System => EntryKind::Notice,
        };
        let format = match message.kind {
            MessageKind::Plain => EntryFormat::Plain,
            MessageKind::UserSteer => EntryFormat::UserSteer,
            MessageKind::Diff => EntryFormat::Diff,
        };
        let done = self.messages.is_complete(message.id);
        let tool = is_tool(message)
            .then(|| export_tool(message, done, self.messages.output_revision(message.id)));
        EntryFacts {
            kind,
            format,
            file_path: message.file_path.clone(),
            done,
            tool,
        }
    }
}

fn export_tool(message: &Message, done: bool, revision: u64) -> Tool {
    let details = message.tool_output.as_ref();
    let label = details
        .map(|tool| tool.summary.as_str())
        .unwrap_or_else(|| {
            if message.role == Role::ToolOutput || message.kind == MessageKind::Diff {
                "Tool output"
            } else {
                message.text.lines().next().unwrap_or("")
            }
        });
    let kind = if message.kind == MessageKind::Diff {
        ToolKind::Edit
    } else if message.file_path.is_some() {
        ToolKind::Read
    } else {
        ToolKind::Other
    };
    let status = if details.is_some_and(|tool| tool.failed) {
        ToolStatus::Failed
    } else if !done {
        ToolStatus::Running
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
        output_revision: OutputRevision::new(revision),
        has_output: (message.role == Role::ToolOutput || message.kind == MessageKind::Diff)
            && !message.text.is_empty(),
    }
}
