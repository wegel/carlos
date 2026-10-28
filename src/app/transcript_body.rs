//! Count and draw transcript bodies using one choice of format and width.

use ratatui::style::{Color, Style};

use super::models::{Message, MessageKind, RenderedLine, Role, StyledSegment};
use super::transcript_diff::{append_wrapped_diff_lines, count_wrapped_diff_lines};
use super::transcript_source::{append_source_lines, count_source_lines};
use super::transcript_styles::{
    append_wrapped_ansi_lines, append_wrapped_markdown_lines, append_wrapped_message_lines,
    count_wrapped_ansi_lines_cached, count_wrapped_markdown_lines,
    count_wrapped_message_lines_cached, RenderCountCache,
};

enum BodyKind {
    Empty,
    ToolCall,
    Plain,
    Markdown,
    Output,
    Diff,
}

/// Describe the body without allocating or highlighting its full contents.
pub(super) struct BlockBody {
    kind: BodyKind,
    width: usize,
    marker: Option<(&'static str, Color)>,
}

impl BlockBody {
    /// Choose a body format independently of the transcript theme.
    pub(super) fn new(message: &Message, width: usize) -> Self {
        let kind = match message.kind {
            MessageKind::ToolSummary { .. } => BodyKind::Empty,
            MessageKind::Diff => BodyKind::Diff,
            MessageKind::Plain => match message.role {
                Role::ToolCall => BodyKind::ToolCall,
                Role::Assistant | Role::Reasoning => BodyKind::Markdown,
                Role::ToolOutput => BodyKind::Output,
                _ => BodyKind::Plain,
            },
        };
        Self {
            kind,
            width,
            marker: None,
        }
    }

    /// Reserve space for a symbol beside a single-line body.
    pub(super) fn set_marker(&mut self, marker: &'static str, color: Color) {
        self.width = self.width.saturating_sub(2);
        self.marker = Some((marker, color));
    }

    /// Count the selected format without highlighting large tool outputs.
    pub(super) fn count<'a>(
        &self,
        cache: &mut RenderCountCache<'a>,
        message: &'a Message,
    ) -> usize {
        if self.width < 8 {
            return 0;
        }
        let (role, text, width) = (message.role, message.text.as_str(), self.width);
        match self.kind {
            BodyKind::Empty => 0,
            BodyKind::ToolCall => text.split_once('\n').map_or(0, |(_, body)| {
                count_wrapped_message_lines_cached(cache, role, body, width)
            }),
            BodyKind::Diff => count_wrapped_diff_lines(message.file_path.as_deref(), text, width),
            BodyKind::Markdown => count_wrapped_markdown_lines(role, text, width),
            BodyKind::Output => {
                if message.file_path.is_some() {
                    if let Some(lines) = count_source_lines(text, width) {
                        return lines;
                    }
                }
                count_wrapped_ansi_lines_cached(cache, role, text, width)
            }
            BodyKind::Plain => count_wrapped_message_lines_cached(cache, role, text, width),
        }
    }

    /// Draw the same body and width used by the count pass.
    pub(super) fn append(&self, out: &mut Vec<RenderedLine>, message: &Message) {
        if self.width < 8 {
            return;
        }
        let start = out.len();
        let (role, text, width) = (message.role, message.text.as_str(), self.width);
        match self.kind {
            BodyKind::Empty => {}
            BodyKind::ToolCall => {
                if let Some((_, body)) = text.split_once('\n') {
                    append_wrapped_message_lines(out, role, body, width);
                }
            }
            BodyKind::Diff => {
                append_wrapped_diff_lines(out, role, message.file_path.as_deref(), text, width)
            }
            BodyKind::Markdown => append_wrapped_markdown_lines(out, role, text, width),
            BodyKind::Output => {
                if !message
                    .file_path
                    .as_deref()
                    .is_some_and(|path| append_source_lines(out, path, text, width))
                {
                    append_wrapped_ansi_lines(out, role, text, width);
                }
            }
            BodyKind::Plain => append_wrapped_message_lines(out, role, text, width),
        }
        if let Some((marker, color)) = self.marker {
            if let Some(first) = out.get_mut(start) {
                prepend_symbol(first, marker, color);
            }
        }
    }
}

/// Add a symbol without changing the text or style of the remaining row.
pub(super) fn prepend_symbol(line: &mut RenderedLine, glyph: &str, color: Color) {
    let prefix = format!("{glyph} ");
    line.text.insert_str(0, &prefix);
    line.cells += 2;
    line.styled_segments.insert(
        0,
        StyledSegment {
            text: prefix,
            style: Style::default().fg(color),
        },
    );
}
