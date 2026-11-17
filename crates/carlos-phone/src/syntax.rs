//! Classify source with real Sublime grammars; Swift chooses colors and retains original copy bytes.

use std::path::Path;
use std::sync::OnceLock;

use syntect::highlighting::ScopeSelectors;
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

const MAX_BYTES: usize = 256 * 1024;
const MAX_LINE_BYTES: usize = 8192;
const MAX_SPANS: usize = 8192;

/// Syntax roles contain no terminal escapes, fonts or platform colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum PhoneSyntaxRole {
    Plain,
    Comment,
    String,
    Number,
    Keyword,
    Type,
    Function,
    Variable,
    Operator,
    Invalid,
}

/// Adjacent source fragments preserve all original Unicode, whitespace and line endings.
#[derive(Debug, uniffi::Record)]
pub struct PhoneSyntaxSpan {
    pub text: String,
    pub role: PhoneSyntaxRole,
}

/// Color a bounded code snippet, falling back to complete plain text for unsupported or costly input.
#[uniffi::export]
pub fn highlight_code(
    source: String,
    language: Option<String>,
    file_path: Option<String>,
) -> Vec<PhoneSyntaxSpan> {
    if source.len() <= MAX_BYTES && source.lines().all(|line| line.len() <= MAX_LINE_BYTES) {
        let grammars = GRAMMARS.get_or_init(SyntaxSet::load_defaults_newlines);
        if let Some(syntax) = select(grammars, language.as_deref(), file_path.as_deref()) {
            if let Some(spans) = parse(&source, syntax, grammars) {
                return spans;
            }
        }
    }
    vec![PhoneSyntaxSpan {
        text: source,
        role: PhoneSyntaxRole::Plain,
    }]
}

static GRAMMARS: OnceLock<SyntaxSet> = OnceLock::new();
static SELECTORS: OnceLock<Vec<(ScopeSelectors, PhoneSyntaxRole)>> = OnceLock::new();

fn select<'a>(
    grammars: &'a SyntaxSet,
    language: Option<&str>,
    path: Option<&str>,
) -> Option<&'a SyntaxReference> {
    if let Some(language) = language {
        return grammars.find_syntax_by_token(language.split_ascii_whitespace().next()?);
    }
    // The path belongs to the remote host. Never open it on this device.
    let path = Path::new(path?);
    path.extension()
        .and_then(|extension| grammars.find_syntax_by_extension(extension.to_str()?))
        .or_else(|| grammars.find_syntax_by_token(path.file_name()?.to_str()?))
}

fn parse(
    source: &str,
    syntax: &SyntaxReference,
    grammars: &SyntaxSet,
) -> Option<Vec<PhoneSyntaxSpan>> {
    let mut parser = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut spans = Vec::new();
    for line in source.split_inclusive('\n') {
        let mut start = 0;
        for (end, change) in parser.parse_line(line, grammars).ok()? {
            append(&mut spans, line.get(start..end)?, role(&stack));
            stack.apply(&change).ok()?;
            start = end;
            if spans.len() > MAX_SPANS {
                return None;
            }
        }
        append(&mut spans, line.get(start..)?, role(&stack));
    }
    Some(spans)
}

fn append(spans: &mut Vec<PhoneSyntaxSpan>, text: &str, role: PhoneSyntaxRole) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut().filter(|last| last.role == role) {
        last.text.push_str(text);
    } else {
        spans.push(PhoneSyntaxSpan {
            text: text.to_owned(),
            role,
        });
    }
}

fn role(stack: &ScopeStack) -> PhoneSyntaxRole {
    selectors()
        .iter()
        .filter_map(|(selector, role)| {
            selector
                .does_match(stack.as_slice())
                .map(|power| (power, *role))
        })
        .max_by_key(|(power, _)| *power)
        .map_or(PhoneSyntaxRole::Plain, |(_, role)| role)
}

fn selectors() -> &'static [(ScopeSelectors, PhoneSyntaxRole)] {
    SELECTORS.get_or_init(|| {
        [
            ("comment", PhoneSyntaxRole::Comment),
            ("string", PhoneSyntaxRole::String),
            (
                "constant.numeric, constant.language, constant.character",
                PhoneSyntaxRole::Number,
            ),
            ("keyword, storage", PhoneSyntaxRole::Keyword),
            (
                "entity.name.type, support.type, support.class",
                PhoneSyntaxRole::Type,
            ),
            (
                "entity.name.function, support.function",
                PhoneSyntaxRole::Function,
            ),
            (
                "variable, entity.name.tag, entity.other.attribute-name",
                PhoneSyntaxRole::Variable,
            ),
            ("keyword.operator", PhoneSyntaxRole::Operator),
            ("invalid", PhoneSyntaxRole::Invalid),
        ]
        .into_iter()
        .map(|(selector, role)| (selector.parse().expect("fixed grammar selector"), role))
        .collect()
    })
}
