//! Share complete block headings and body choices with the source-owning formatter thread.

use std::sync::Arc;

use carlos_core::session::models::{MessageKind, Role};

use super::super::ansi_pages::PageError;
use super::super::{prepend_symbol, BlockBody};
use super::body::BodyPages;
use crate::app::flow_transcript::EntryContext;
use crate::app::render_entry_cache::{PaintedRows, PreparedRow, RowFormat};
use crate::app::tool_output_state::compact_tool_message;
use crate::app::transcript_block::BlockPlan;
use crate::app::transcript_input::RenderInput;
use crate::app::view_models::{DisplayKind, DisplayMessage, RenderedLine};

/// Copy only one selected body's metadata across the formatter boundary.
#[derive(Clone, PartialEq, Eq)]
pub(in crate::app) struct Facts {
    role: Role,
    kind: MessageKind,
    path: Option<String>,
    label: Option<String>,
    generated: bool,
    failed: bool,
}

/// Match one viewport request without including another full source string.
#[derive(Clone, PartialEq, Eq)]
pub(in crate::app) struct View {
    pub facts: Facts,
    pub target: super::selected::Target,
    pub format: RowFormat,
    pub previous: Option<EntryContext>,
    pub first: usize,
    pub length: usize,
}

/// Keep only source checkpoints/logical lines plus small shared block headings.
pub(in crate::app) struct FormattedBlock {
    pages: BodyPages,
    prefix: Arc<Vec<RenderedLine>>,
    body: Option<BlockBody>,
    context: EntryContext,
    visible: bool,
}

impl Facts {
    /// Retain metadata while the accepted source travels separately in bounded pieces.
    pub fn from_input(input: RenderInput<'_>) -> Self {
        Self {
            role: input.role,
            kind: input.kind,
            path: input.file_path.map(Into::into),
            label: input.label.map(Into::into),
            generated: input.label_generated,
            failed: input.failed,
        }
    }

    fn input<'a>(&'a self, text: &'a str) -> RenderInput<'a> {
        RenderInput {
            role: self.role,
            kind: self.kind,
            text,
            file_path: self.path.as_deref(),
            label: self.label.as_deref(),
            label_generated: self.generated,
            failed: self.failed,
        }
    }
}

impl FormattedBlock {
    /// Resolve the existing complete block plan using an exact body row count.
    pub fn new(original: Arc<String>, view: &View) -> Result<Self, PageError> {
        let (context, prefix, body) = {
            let input = view.facts.input(&original);
            let context = EntryContext::new(input, view.format.width, view.format.theme);
            let plan = BlockPlan::from_context(
                view.previous,
                context,
                DisplayMessage {
                    source: input,
                    kind: DisplayKind::Full,
                },
                view.format.width,
                view.format.theme,
            );
            let (prefix, body) = plan.into_parts();
            (context, prefix, body)
        };
        let pages = match &body {
            Some(body) => BodyPages::new(original, body, view.facts.input(""))?,
            None => BodyPages::Empty(original),
        };
        let mut block = Self {
            pages,
            prefix: Arc::new(prefix),
            body,
            context,
            visible: !view.format.hidden,
        };
        block.fold(view);
        Ok(block)
    }

    /// Keep checkpoint indexes for ordinary live appends instead of rebuilding the complete body.
    pub fn append(&mut self, text: &str, view: &View) -> Result<bool, PageError> {
        if !self.pages.append(text)? {
            return Ok(false);
        }
        self.fold(view);
        Ok(true)
    }

    /// Borrow every original byte for copy or a checked source append.
    pub fn original(&self) -> &str {
        self.pages.original()
    }

    /// Move the original back to the receiving state without cloning it.
    pub fn into_original(self) -> Arc<String> {
        self.pages.into_original()
    }

    /// Return headings plus the requested body rows at their full block positions.
    pub fn window(&self, first: usize, length: usize) -> Result<PreparedRow, PageError> {
        let count = if self.visible {
            self.prefix.len() + self.body.as_ref().map_or(0, |_| self.pages.rows())
        } else {
            0
        };
        let body_first = first.saturating_sub(self.prefix.len());
        let body_length = count
            .min(first.saturating_add(length))
            .saturating_sub(first.max(self.prefix.len()));
        let mut rows = if self.body.is_some() && self.visible {
            self.pages.window(body_first, body_length)?
        } else {
            Vec::new()
        };
        if body_first == 0 {
            if let Some((marker, color)) = self.body.as_ref().and_then(|body| body.marker) {
                if let Some(first) = rows.first_mut() {
                    prepend_symbol(first, marker, color);
                }
            }
        }
        let prefix = self.visible.then(|| Arc::clone(&self.prefix));
        Ok(PreparedRow {
            source: None,
            count,
            lines: Some(PaintedRows::window(
                prefix,
                self.prefix.len() + body_first,
                Arc::new(rows),
            )),
            context: self.context,
            loading: false,
        })
    }

    fn fold(&mut self, view: &View) {
        let input = view.facts.input(self.pages.original());
        if !view.format.compact
            || !(matches!(input.role, Role::ToolCall | Role::ToolOutput)
                || input.kind == MessageKind::Diff)
        {
            return;
        }
        let Some(display) = compact_tool_message(input, self.pages.rows()) else {
            return;
        };
        let plan = BlockPlan::from_context(
            view.previous,
            self.context,
            display,
            view.format.width,
            view.format.theme,
        );
        let (prefix, _) = plan.into_parts();
        self.prefix = Arc::new(prefix);
        self.body = None;
    }
}
