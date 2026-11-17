//! Expose compact visible rows without copying body caches or native routing data.

use carlos_proto::session::{Entry, EntryFormat, EntryKind, LabelSource, ToolKind, ToolStatus};

/// A row's semantic role; Swift chooses its glyph, color and layout.
pub type PhoneEntryKind = EntryKind;
#[uniffi::remote(Enum)]
enum PhoneEntryKind {
    User,
    Reasoning,
    Commentary,
    Answer,
    ToolCall,
    Tool,
    Notice,
}

/// Diff and steering text retain their source meaning without terminal controls.
pub type PhoneEntryFormat = EntryFormat;
#[uniffi::remote(Enum)]
enum PhoneEntryFormat {
    Plain,
    UserSteer,
    Diff,
}

/// The actual tool family, including an honest unknown family.
pub type PhoneToolKind = ToolKind;
#[uniffi::remote(Enum)]
enum PhoneToolKind {
    Command,
    Read,
    Search,
    Edit,
    Other,
}

/// Literal and fallback labels truncate; an actual AI label may wrap.
pub type PhoneLabelSource = LabelSource;
#[uniffi::remote(Enum)]
enum PhoneLabelSource {
    Fallback,
    Literal,
    Ai,
}

/// Missing native outcomes never become successful tools.
pub type PhoneToolStatus = ToolStatus;
#[uniffi::remote(Enum)]
enum PhoneToolStatus {
    Running,
    Ok,
    Failed,
    Interrupted,
    Unknown,
}

/// A missing source size means this row contains its complete inline text.
#[derive(uniffi::Record)]
pub struct PhoneText {
    pub preview: String,
    pub source_bytes: Option<u64>,
    pub source_revision: Option<u64>,
}

/// Preserve missing diff counts instead of inventing zero changes.
#[derive(uniffi::Record)]
pub struct PhoneDiff {
    pub added: Option<u64>,
    pub removed: Option<u64>,
}

/// Tool headers are small; expanded output uses a separately captured body lifetime.
#[derive(uniffi::Record)]
pub struct PhoneTool {
    pub has_result: bool,
    pub kind: PhoneToolKind,
    pub label: String,
    pub label_source: PhoneLabelSource,
    pub status: PhoneToolStatus,
    pub summary: Option<String>,
    pub diff: Option<PhoneDiff>,
    pub has_output: bool,
}

/// One visible or changed row, addressed by its stable daemon id.
#[derive(uniffi::Record)]
pub struct PhoneEntry {
    pub id: String,
    pub kind: PhoneEntryKind,
    pub format: PhoneEntryFormat,
    pub file_path: Option<String>,
    pub text: PhoneText,
    pub done: bool,
    pub tool: Option<PhoneTool>,
}

impl From<&Entry> for PhoneEntry {
    fn from(entry: &Entry) -> Self {
        Self {
            id: entry.id.to_string(),
            kind: entry.kind,
            format: entry.format,
            file_path: entry.file_path.clone(),
            text: PhoneText {
                preview: entry.text.clone(),
                source_bytes: entry.source.as_ref().map(|source| source.bytes.get()),
                source_revision: entry.source.as_ref().map(|source| source.revision.get()),
            },
            done: entry.done,
            tool: entry.tool.as_ref().map(|tool| PhoneTool {
                has_result: false,
                kind: tool.kind,
                label: tool.label.clone(),
                label_source: tool.label_source,
                status: tool.status,
                summary: tool.summary.clone(),
                has_output: tool.has_output,
                diff: (tool.added.is_some() || tool.removed.is_some()).then_some(PhoneDiff {
                    added: tool.added,
                    removed: tool.removed,
                }),
            }),
        }
    }
}
