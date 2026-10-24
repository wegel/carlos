//! Track old and new source separately while coloring a diff hunk.

use ratatui::style::Style;
use ratatui_interact::components::{DiffLine, DiffLineType};

use super::models::StyledSegment;
use super::source_highlight::SourceHighlighter;

/// Give each side its own parser so a removed comment cannot color added code.
pub(super) struct DiffHighlighters {
    old: Option<SourceHighlighter>,
    new: Option<SourceHighlighter>,
}

impl DiffHighlighters {
    /// Start fresh at a hunk because omitted lines may change the parser state.
    pub(super) fn new(old_path: Option<&str>, new_path: Option<&str>) -> Self {
        Self {
            old: old_path.and_then(SourceHighlighter::for_path),
            new: new_path.and_then(SourceHighlighter::for_path),
        }
    }

    /// Color source tokens, keeping the caller's diff background.
    pub(super) fn highlight(&mut self, line: &DiffLine, style: Style) -> Vec<StyledSegment> {
        let highlighter = match line.line_type {
            DiffLineType::Deletion => self.old.as_mut(),
            DiffLineType::Addition => self.new.as_mut(),
            DiffLineType::Context => {
                if let Some(old) = self.old.as_mut() {
                    old.highlight(&line.content, style);
                }
                self.new.as_mut()
            }
            DiffLineType::HunkHeader => None,
        };
        match highlighter {
            Some(highlighter) => highlighter.highlight(&line.content, style),
            None => vec![StyledSegment {
                text: line.content.clone(),
                style,
            }],
        }
    }
}

/// Split a git diff before parsing so each file keeps its own name and language.
pub(super) fn split_file_diffs(diff: &str) -> Vec<&str> {
    let mut starts = vec![0];
    let mut offset = 0;
    for line in diff.split_inclusive('\n') {
        if offset > 0 && line.starts_with("diff --git ") {
            starts.push(offset);
        }
        offset += line.len();
    }
    starts.push(diff.len());
    starts
        .windows(2)
        .map(|pair| &diff[pair[0]..pair[1]])
        .collect()
}
