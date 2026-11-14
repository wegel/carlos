//! Borrow complete source text for the existing renderer without constructing a core message.

use carlos_core::session::models::{Message, MessageKind, Role};
#[cfg(test)]
use carlos_proto::session::{Entry, EntryFormat, EntryKind, LabelSource, ToolStatus};
#[cfg(test)]
use carlos_proto::source::Source;

/// A body renderer needs source facts and a borrowed string, not a mutable session.
#[derive(Clone, Copy, Debug)]
pub(super) struct RenderInput<'a> {
    pub role: Role,
    pub kind: MessageKind,
    pub text: &'a str,
    pub file_path: Option<&'a str>,
    pub label: Option<&'a str>,
    pub label_generated: bool,
    pub failed: bool,
}

impl<'a> From<&'a Message> for RenderInput<'a> {
    fn from(message: &'a Message) -> Self {
        Self {
            role: message.role,
            kind: message.kind,
            text: &message.text,
            file_path: message.file_path.as_deref(),
            label: message
                .tool_output
                .as_ref()
                .map(|tool| tool.summary.as_str()),
            label_generated: message
                .tool_output
                .as_ref()
                .is_some_and(|tool| tool.generated),
            failed: message.tool_output.as_ref().is_some_and(|tool| tool.failed),
        }
    }
}

impl<'a> RenderInput<'a> {
    /// Refuse a suffix or retired body instead of parsing it as a complete Markdown/ANSI document.
    #[cfg(test)]
    pub fn from_client(entry: &'a Entry, source: Option<(&Source, &'a str)>) -> Option<Self> {
        let text = match &entry.source {
            Some(expected) => {
                let (actual, text) = source?;
                if actual != expected || text.len() as u64 != expected.bytes.get() {
                    return None;
                }
                text
            }
            None if entry.tool.is_some()
                || matches!(entry.kind, EntryKind::ToolCall | EntryKind::Tool) =>
            {
                let (actual, text) = source?;
                if actual.revision != entry.tool.as_ref()?.output_revision
                    || text.len() as u64 != actual.bytes.get()
                {
                    return None;
                }
                text
            }
            None => &entry.text,
        };
        Some(Self {
            role: match entry.kind {
                EntryKind::User => Role::User,
                EntryKind::Reasoning => Role::Reasoning,
                EntryKind::Commentary => Role::Commentary,
                EntryKind::Answer => Role::Assistant,
                EntryKind::ToolCall => Role::ToolCall,
                EntryKind::Tool => Role::ToolOutput,
                EntryKind::Notice => Role::System,
            },
            kind: match entry.format {
                EntryFormat::Plain => MessageKind::Plain,
                EntryFormat::UserSteer => MessageKind::UserSteer,
                EntryFormat::Diff => MessageKind::Diff,
            },
            text,
            file_path: entry.file_path.as_deref(),
            label: entry
                .tool
                .as_ref()
                .filter(|tool| tool.label_source != LabelSource::Fallback)
                .map(|tool| tool.label.as_str()),
            label_generated: entry
                .tool
                .as_ref()
                .is_some_and(|tool| tool.label_source == LabelSource::Ai),
            failed: entry
                .tool
                .as_ref()
                .is_some_and(|tool| tool.status == ToolStatus::Failed),
        })
    }
}
