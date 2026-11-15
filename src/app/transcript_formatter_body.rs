//! Keep one original body with the existing formatter's logical lines or source checkpoints.

use std::sync::Arc;

use super::super::ansi_pages::plain::PlainPages;
use super::super::ansi_pages::{AnsiPages, PageError};
use super::super::document::DocumentFrame;
use super::super::{BlockBody, BodyKind};
use crate::app::transcript_diff::{
    diff_line_style, diff_path_rendered_line, diff_transcript_logical_lines,
};
use crate::app::transcript_input::RenderInput;
use crate::app::transcript_source::source_logical_lines;
use crate::app::view_models::{RenderedLine, StyledSegment};

/// Each variant owns the same single original allocation through one Arc.
pub(super) enum BodyPages {
    Empty(Arc<String>),
    Plain(PlainPages),
    UnparsedOutput(PlainPages),
    Ansi(AnsiPages),
    Styled {
        original: Arc<String>,
        document: DocumentFrame,
        prefix: Vec<RenderedLine>,
    },
}

impl BodyPages {
    /// Use the existing body choice, role, path and width without copying a core message.
    pub fn new(
        original: Arc<String>,
        body: &BlockBody,
        facts: RenderInput<'_>,
    ) -> Result<Self, PageError> {
        if body.width < 8 {
            return Ok(Self::Empty(original));
        }
        match body.kind {
            BodyKind::Empty => Ok(Self::Empty(original)),
            BodyKind::ToolCall => match original.find('\n') {
                Some(line) => PlainPages::from_offset(original, facts.role, body.width, line + 1)
                    .map(Self::Plain),
                None => Ok(Self::Empty(original)),
            },
            BodyKind::Plain => PlainPages::new(original, facts.role, body.width).map(Self::Plain),
            BodyKind::Markdown => {
                let document = DocumentFrame::parse_role(facts.role, &original, body.width);
                Ok(Self::styled(original, document, Vec::new()))
            }
            BodyKind::Output => Self::output(original, body.width, facts),
            BodyKind::Diff => Ok(Self::diff(original, body.width, facts)),
        }
    }

    /// Reindex only the unfinished tail for plain or already-ANSI bodies.
    pub fn append(&mut self, text: &str) -> Result<bool, PageError> {
        match self {
            Self::Plain(pages) if !text.contains(['\x1b', '\u{009b}']) => {
                pages.append(text);
                Ok(true)
            }
            Self::Ansi(pages) => pages.append(text).map(|()| true),
            _ => Ok(false),
        }
    }

    /// Borrow every original byte for exact copy, including skipped tool-call headings.
    pub fn original(&self) -> &str {
        match self {
            Self::Empty(original) | Self::Styled { original, .. } => original,
            Self::Plain(pages) | Self::UnparsedOutput(pages) => pages.original_text(),
            Self::Ansi(pages) => pages.original_text(),
        }
    }

    /// Move the one original allocation when a body changes format or needs a whole parse.
    pub fn into_original(self) -> Arc<String> {
        match self {
            Self::Empty(original) | Self::Styled { original, .. } => original,
            Self::Plain(pages) | Self::UnparsedOutput(pages) => pages.into_original(),
            Self::Ansi(pages) => pages.into_original(),
        }
    }

    /// Count body rows without allocating painted strings for hidden rows.
    pub fn rows(&self) -> usize {
        match self {
            Self::Empty(_) => 0,
            Self::Plain(pages) | Self::UnparsedOutput(pages) => pages.rows(),
            Self::Ansi(pages) => pages.rows(),
            Self::Styled {
                document, prefix, ..
            } => prefix.len() + document.rows(),
        }
    }

    /// Paint a requested body interval using its saved wrap and style choices.
    pub fn window(&self, first: usize, length: usize) -> Result<Vec<RenderedLine>, PageError> {
        match self {
            Self::Empty(_) => Ok(Vec::new()),
            Self::Plain(pages) | Self::UnparsedOutput(pages) => Ok(pages.window(first, length)),
            Self::Ansi(pages) => pages.window(first, length),
            Self::Styled {
                document, prefix, ..
            } => {
                let end = first.saturating_add(length).min(self.rows());
                let mut rows: Vec<_> = prefix.iter().skip(first).take(length).cloned().collect();
                rows.extend(document.window(
                    first.saturating_sub(prefix.len()),
                    end.saturating_sub(first.max(prefix.len())),
                ));
                Ok(rows)
            }
        }
    }

    fn output(
        original: Arc<String>,
        width: usize,
        facts: RenderInput<'_>,
    ) -> Result<Self, PageError> {
        if let Some(lines) = facts
            .file_path
            .and_then(|path| source_logical_lines(path, &original))
        {
            let document = DocumentFrame::from_lines(lines, facts.role, width);
            return Ok(Self::styled(original, document, Vec::new()));
        }
        // Trying the ANSI parser briefly shares the Arc, never the String's bytes.
        match AnsiPages::new(Arc::clone(&original), facts.role, width) {
            Ok(pages) => Ok(Self::Ansi(pages)),
            Err(_) => {
                let incomplete = original.contains(['\x1b', '\u{009b}']);
                PlainPages::new(original, facts.role, width).map(|pages| {
                    if incomplete {
                        Self::UnparsedOutput(pages)
                    } else {
                        Self::Plain(pages)
                    }
                })
            }
        }
    }

    fn diff(original: Arc<String>, width: usize, facts: RenderInput<'_>) -> Self {
        let (document, prefix) =
            match diff_transcript_logical_lines(facts.file_path, &original, true) {
                Some(lines) => (
                    DocumentFrame::from_lines(lines, facts.role, width),
                    Vec::new(),
                ),
                None => {
                    let lines = original
                        .split('\n')
                        .map(|line| {
                            vec![StyledSegment {
                                text: line.into(),
                                style: diff_line_style(line),
                            }]
                        })
                        .collect();
                    let prefix = facts
                        .file_path
                        .filter(|path| !path.is_empty())
                        .map(|path| vec![diff_path_rendered_line(path, facts.role)])
                        .unwrap_or_default();
                    (
                        DocumentFrame::uniform_lines(lines, facts.role, width),
                        prefix,
                    )
                }
            };
        Self::styled(original, document, prefix)
    }

    fn styled(
        original: Arc<String>,
        mut document: DocumentFrame,
        prefix: Vec<RenderedLine>,
    ) -> Self {
        document.index();
        Self::Styled {
            original,
            document,
            prefix,
        }
    }
}
