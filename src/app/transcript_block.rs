//! Plan headings, labels, body width, and spacing once for both count and draw passes.

use super::flow_transcript::{inline_reasoning, section_header};
use super::layout_state::TranscriptTheme;
use super::models::{Message, MessageKind, RenderedLine, Role};
use super::tool_output_state::compact_tool_message;
use super::tool_summary::{summary_lines, tool_failed, tool_gutter};
use super::transcript_body::{prepend_symbol, BlockBody};
use super::transcript_spacing::should_insert_separator_between;
use super::transcript_styles::RenderCountCache;
use crate::theme::role_gutter_fg;

/// Keep only small heading/label rows eager; large bodies remain lazy.
pub(super) struct BlockPlan {
    prefix: Vec<RenderedLine>,
    body: Option<BlockBody>,
}

impl BlockPlan {
    /// Resolve all theme-specific row choices before counting or highlighting text.
    pub(super) fn new(
        previous: Option<&Message>,
        original: &Message,
        display: &Message,
        width: usize,
        theme: TranscriptTheme,
    ) -> Self {
        let mut plan = Self {
            prefix: Vec::new(),
            body: None,
        };
        if width < 8 || !has_visible_content(original) {
            return plan;
        }
        let flow = theme.is_flow();
        let spacing_width = width.saturating_sub(if flow { 2 } else { 0 });
        if previous.is_some_and(|previous| {
            should_insert_separator_between(previous, original, spacing_width)
        }) {
            plan.prefix.push(blank_line());
        }
        if flow {
            if let Some(header) = section_header(previous, original, width) {
                plan.prefix.push(header);
            }
        }
        if needs_tool_label(display, flow) {
            plan.prefix
                .extend(tool_labels(original, display, width, flow));
        }
        let mut body = BlockBody::new(display, width);
        if flow && inline_reasoning(original, width) {
            body.set_marker("◇", role_gutter_fg(Role::Reasoning));
        }
        plan.body = Some(body);
        plan
    }

    /// Count the exact stored headings and body choices.
    pub(super) fn count<'a>(
        &self,
        cache: &mut RenderCountCache<'a>,
        message: &'a Message,
    ) -> usize {
        self.prefix.len()
            + self
                .body
                .as_ref()
                .map_or(0, |body| body.count(cache, message))
    }

    /// Materialize only this block using the choices saved by the count pass.
    pub(super) fn render(&self, message: &Message) -> Vec<RenderedLine> {
        let mut out = self.prefix.clone();
        if let Some(body) = &self.body {
            body.append(&mut out, message);
        }
        out
    }
}

/// Recognize text or an explicit label when formatting an individual block.
pub(super) fn has_visible_content(message: &Message) -> bool {
    (message.role == Role::ToolCall && message.tool_output.is_some())
        || !message.text.trim().is_empty()
}

/// Choose the folded message and the plan that both passes will use.
pub(super) fn plan_display_block<'a>(
    cache: &mut RenderCountCache<'a>,
    previous: Option<&Message>,
    message: &'a Message,
    width: usize,
    compact: bool,
    theme: TranscriptTheme,
) -> (Option<Message>, BlockPlan) {
    let folded = if compact
        && (matches!(message.role, Role::ToolCall | Role::ToolOutput)
            || message.kind == MessageKind::Diff)
    {
        compact_tool_message(
            message,
            BlockBody::new(message, width).count(cache, message),
        )
    } else {
        None
    };
    let display = folded.as_ref().unwrap_or(message);
    let plan = BlockPlan::new(previous, message, display, width, theme);
    (folded, plan)
}

fn needs_tool_label(message: &Message, flow: bool) -> bool {
    matches!(message.kind, MessageKind::ToolSummary { .. })
        || message.role == Role::ToolCall
        || (message.role == Role::ToolOutput
            && (flow
                || tool_failed(message)
                || message
                    .tool_output
                    .as_ref()
                    .is_some_and(|tool| tool.generated)))
}

fn tool_labels(
    original: &Message,
    display: &Message,
    width: usize,
    flow: bool,
) -> Vec<RenderedLine> {
    let rows = match display.kind {
        MessageKind::ToolSummary { rows } => rows,
        _ => 0,
    };
    let inline = flow && matches!(original.role, Role::ToolCall | Role::ToolOutput);
    let summary = inline
        .then(|| compact_tool_message(display, rows))
        .flatten();
    let mut lines = summary_lines(
        summary.as_ref().unwrap_or(display),
        width.saturating_sub(if inline { 2 } else { 0 }),
        rows,
    );
    if inline {
        let (glyph, color) = tool_gutter(original);
        for (index, line) in lines.iter_mut().enumerate() {
            prepend_symbol(line, if index == 0 { glyph } else { " " }, color);
        }
    }
    lines
}

fn blank_line() -> RenderedLine {
    RenderedLine {
        text: String::new(),
        styled_segments: Vec::new(),
        role: Role::System,
        separator: true,
        cells: 0,
        soft_wrap_to_next: false,
    }
}
