//! Color source tokens without changing their text or the transcript background.

use std::path::Path;
use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};
use two_face::re_exports::syntect::easy::HighlightLines;
use two_face::re_exports::syntect::highlighting::{FontStyle, Theme};
use two_face::re_exports::syntect::parsing::SyntaxSet;
use two_face::theme::EmbeddedThemeName;

use super::models::StyledSegment;

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_no_newlines);
static THEME: LazyLock<Theme> =
    LazyLock::new(|| two_face::theme::extra()[EmbeddedThemeName::CatppuccinMocha].clone());

/// Keep multiline strings and comments in one version of a source file.
pub(super) struct SourceHighlighter(HighlightLines<'static>);

impl SourceHighlighter {
    /// Choose a language from a filename without opening the file.
    pub(super) fn for_path(path: &str) -> Option<Self> {
        let path = Path::new(path);
        let name = path.file_name()?.to_str()?;
        let syntax = SYNTAXES
            .find_syntax_by_extension(name)
            .or_else(|| SYNTAXES.find_syntax_by_extension(path.extension()?.to_str()?))?;
        Some(Self(HighlightLines::new(syntax, &THEME)))
    }

    /// Color one source line while preserving the caller's background.
    pub(super) fn highlight(&mut self, text: &str, base: Style) -> Vec<StyledSegment> {
        let Ok(tokens) = self.0.highlight_line(text, &SYNTAXES) else {
            return vec![StyledSegment {
                text: text.to_string(),
                style: base,
            }];
        };
        tokens
            .into_iter()
            .filter(|(_, text)| !text.is_empty())
            .map(|(style, text)| {
                let mut modifiers = Modifier::empty();
                for (font, modifier) in [
                    (FontStyle::BOLD, Modifier::BOLD),
                    (FontStyle::ITALIC, Modifier::ITALIC),
                    (FontStyle::UNDERLINE, Modifier::UNDERLINED),
                ] {
                    if style.font_style.contains(font) {
                        modifiers |= modifier;
                    }
                }
                let color = style.foreground;
                StyledSegment {
                    text: text.to_string(),
                    style: base
                        .fg(Color::Rgb(color.r, color.g, color.b))
                        .add_modifier(modifiers),
                }
            })
            .collect()
    }
}
