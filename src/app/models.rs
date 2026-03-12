use ratatui::style::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    User,
    Assistant,
    Reasoning,
    ToolCall,
    ToolOutput,
    System,
}

#[derive(Debug, Clone)]
pub(super) struct Message {
    pub(super) role: Role,
    pub(super) text: String,
    pub(super) kind: MessageKind,
    pub(super) file_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MessageKind {
    Plain,
    Diff,
}

#[derive(Debug, Clone)]
pub(super) struct DiffBlock {
    pub(super) file_path: Option<String>,
    pub(super) diff: String,
}

#[derive(Debug, Clone)]
pub(super) struct ThreadSummary {
    pub(super) id: String,
    pub(super) name: Option<String>,
    pub(super) preview: String,
    pub(super) cwd: String,
    pub(super) created_at: i64,
    pub(super) updated_at: i64,
}

#[derive(Debug, Clone)]
pub(super) struct StyledSegment {
    pub(super) text: String,
    pub(super) style: Style,
}

#[derive(Debug, Clone)]
pub(super) struct RenderedLine {
    pub(super) text: String,
    pub(super) styled_segments: Vec<StyledSegment>,
    pub(super) role: Role,
    pub(super) separator: bool,
    pub(super) cells: usize,
    pub(super) soft_wrap_to_next: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TerminalSize {
    pub(super) width: usize,
    pub(super) height: usize,
}
