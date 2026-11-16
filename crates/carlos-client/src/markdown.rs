//! Parse complete Markdown in pure Rust while each platform owns fonts, layout and original copy bytes.

mod parser;

use crate::ClientError;

/// A native renderer receives semantic blocks, including list and quote offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub indent: u16,
    pub quote_depth: u16,
    pub marker: Option<String>,
    pub spans: Vec<Span>,
    pub cells: Vec<Vec<Span>>,
}

/// A code fence retains its language; a table row retains each column's alignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading { level: u8 },
    Code { language: Option<String> },
    Rule,
    TableRow { header: bool, columns: Vec<Column> },
}

/// The alignment specified by a Markdown table, without a terminal width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Unspecified,
    Left,
    Center,
    Right,
}

/// Adjacent text with the same inline style shares one span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// Nested inline markup is resolved once before a platform chooses its attributed text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub strong: bool,
    pub emphasis: bool,
    pub code: bool,
    pub strike: bool,
    pub link: Option<String>,
}

/// Resolve references against the whole source; never parse independently split network chunks.
pub fn parse(source: &str) -> Result<Vec<Block>, ClientError> {
    if source.len() > 64 * 1024 * 1024 {
        return Err(ClientError::Capacity(
            "Markdown source exceeds 64 MiB; retain original bytes",
        ));
    }
    parser::parse(source)
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
