//! Plan headings, labels, body width, and spacing once for both count and draw passes.

use carlos_core::session::models::Message;
use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;

use super::flow_transcript::{inline_reasoning, section_header_from_context, EntryContext};
use super::layout_state::TranscriptTheme;
use super::tool_output_state::compact_tool_message;
use super::tool_summary::{summary_lines, tool_failed, tool_gutter};
use super::transcript_body::{prepend_symbol, BlockBody};
use super::transcript_input::RenderInput;
use super::transcript_styles::RenderCountCache;
use super::view_models::{DisplayKind, DisplayMessage, RenderedLine};
use crate::theme::role_gutter_fg;

/// Keep only small heading/label rows eager; large bodies remain lazy.
pub(super) struct BlockPlan {
    prefix: Vec<RenderedLine>,
    body: Option<BlockBody>,
}

impl BlockPlan {
    /// Resolve all theme-specific row choices before counting or highlighting text.
    #[cfg(test)]
    pub(super) fn new(
        previous: Option<&Message>,
        display: DisplayMessage<'_>,
        width: usize,
        theme: TranscriptTheme,
    ) -> Self {
        Self::from_input(previous.map(Into::into), display, width, theme)
    }

    /// Plan headings and labels directly from one borrowed client or core entry.
    #[cfg(test)]
    pub(super) fn from_input(
        previous: Option<RenderInput<'_>>,
        display: DisplayMessage<'_>,
        width: usize,
        theme: TranscriptTheme,
    ) -> Self {
        Self::from_context(
            previous.map(|entry| EntryContext::new(entry, width, theme)),
            EntryContext::new(display.source, width, theme),
            display,
            width,
            theme,
        )
    }

    /// Build a row using the saved heading facts of its previous visible entry.
    pub(super) fn from_context(
        previous: Option<EntryContext>,
        context: EntryContext,
        display: DisplayMessage<'_>,
        width: usize,
        theme: TranscriptTheme,
    ) -> Self {
        let original = display.source;
        let mut plan = Self {
            prefix: Vec::new(),
            body: None,
        };
        if width < 8 || !visible_input(original) {
            return plan;
        }
        let flow = theme.is_flow();
        if previous.is_some_and(|previous| previous.separates(context)) {
            plan.prefix.push(blank_line());
        }
        if flow || original.kind == MessageKind::UserSteer {
            if let Some(header) = section_header_from_context(previous, context, original, width) {
                plan.prefix.push(header);
            }
        }
        if needs_tool_label(display, flow) {
            plan.prefix.extend(tool_labels(display, width, flow));
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
        self.count_input(cache, message.into())
    }

    /// Count a complete block without making a source Message.
    pub(super) fn count_input<'a>(
        &self,
        cache: &mut RenderCountCache<'a>,
        message: RenderInput<'a>,
    ) -> usize {
        self.prefix.len()
            + self
                .body
                .as_ref()
                .map_or(0, |body| body.count_input(cache, message))
    }

    /// Materialize only this block using the choices saved by the count pass.
    #[cfg(test)]
    pub(super) fn render(&self, message: &Message) -> Vec<RenderedLine> {
        self.render_input(message.into())
    }

    /// Materialize a borrowed client block with the same headings and source rules.
    pub(super) fn render_input(&self, message: RenderInput<'_>) -> Vec<RenderedLine> {
        let mut out = self.prefix.clone();
        if let Some(body) = &self.body {
            body.append_input(&mut out, message);
        }
        out
    }
}

/// Recognize text or an explicit label when formatting an individual block.
#[cfg(test)]
pub(super) fn has_visible_content(message: &Message) -> bool {
    visible_input(message.into())
}

pub(super) fn visible_input(message: RenderInput<'_>) -> bool {
    (message.role == Role::ToolCall && message.label.is_some()) || !message.text.trim().is_empty()
}

/// Plan a full or folded view while borrowing the stored source message.
pub(super) fn plan_display_block<'a>(
    cache: &mut RenderCountCache<'a>,
    previous: Option<&Message>,
    message: &'a Message,
    width: usize,
    compact: bool,
    theme: TranscriptTheme,
) -> BlockPlan {
    plan_input_block(
        cache,
        previous.map(Into::into),
        message.into(),
        width,
        compact,
        theme,
    )
}

/// Share the complete block planner with the client-backed transcript.
pub(super) fn plan_input_block<'a>(
    cache: &mut RenderCountCache<'a>,
    previous: Option<RenderInput<'_>>,
    message: RenderInput<'a>,
    width: usize,
    compact: bool,
    theme: TranscriptTheme,
) -> BlockPlan {
    plan_context_block(
        cache,
        previous.map(|entry| EntryContext::new(entry, width, theme)),
        EntryContext::new(message, width, theme),
        message,
        width,
        compact,
        theme,
    )
}

/// Plan one source without requiring its neighbor's source to stay resident.
pub(super) fn plan_context_block<'a>(
    cache: &mut RenderCountCache<'a>,
    previous: Option<EntryContext>,
    context: EntryContext,
    message: RenderInput<'a>,
    width: usize,
    compact: bool,
    theme: TranscriptTheme,
) -> BlockPlan {
    let full = DisplayMessage {
        source: message,
        kind: DisplayKind::Full,
    };
    let display = if compact
        && (matches!(message.role, Role::ToolCall | Role::ToolOutput)
            || message.kind == MessageKind::Diff)
    {
        let rows = BlockBody::new(full, width).count_input(cache, message);
        compact_tool_message(message, rows).unwrap_or(full)
    } else {
        full
    };
    BlockPlan::from_context(previous, context, display, width, theme)
}

fn needs_tool_label(display: DisplayMessage<'_>, flow: bool) -> bool {
    let message = display.source;
    matches!(display.kind, DisplayKind::ToolSummary { .. })
        || message.role == Role::ToolCall
        || (message.role == Role::ToolOutput
            && (flow || tool_failed(message) || message.label_generated))
}

fn tool_labels(display: DisplayMessage<'_>, width: usize, flow: bool) -> Vec<RenderedLine> {
    let original = display.source;
    let inline = flow && matches!(original.role, Role::ToolCall | Role::ToolOutput);
    let summary = if inline {
        compact_tool_message(original, display.hidden_rows()).unwrap_or(display)
    } else {
        display
    };
    let mut lines = summary_lines(summary, width.saturating_sub(if inline { 2 } else { 0 }));
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

#[cfg(test)]
#[path = "transcript_block_tests.rs"]
mod tests;
