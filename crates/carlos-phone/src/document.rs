//! Expose only visible blocks from a whole document parsed by the shared pure Markdown reader.

use std::sync::Arc;

use carlos_client::markdown::{self, Block, BlockKind, Column, Span, Style};

use crate::PhoneError;

/// Native views retain one parsed document and read only the blocks they display.
#[derive(uniffi::Object)]
pub struct PhoneDocument {
    blocks: Vec<Block>,
}

/// A complete semantic block, with independent table cells and original list markers.
pub type PhoneMarkdownBlock = Block;
#[uniffi::remote(Record)]
struct PhoneMarkdownBlock {
    kind: PhoneMarkdownKind,
    indent: u16,
    quote_depth: u16,
    marker: Option<String>,
    spans: Vec<PhoneMarkdownSpan>,
    cells: Vec<Vec<PhoneMarkdownSpan>>,
}

/// Swift lays out the block without interpreting Markdown punctuation.
pub type PhoneMarkdownKind = BlockKind;
#[uniffi::remote(Enum)]
enum PhoneMarkdownKind {
    Paragraph,
    Heading {
        level: u8,
    },
    Code {
        language: Option<String>,
    },
    Rule,
    TableRow {
        header: bool,
        columns: Vec<PhoneMarkdownColumn>,
    },
}

/// Preserve the author's table-column alignment.
pub type PhoneMarkdownColumn = Column;
#[uniffi::remote(Enum)]
enum PhoneMarkdownColumn {
    Unspecified,
    Left,
    Center,
    Right,
}

/// Styled text is distinct from the original Markdown copied by the native clipboard owner.
pub type PhoneMarkdownSpan = Span;
#[uniffi::remote(Record)]
struct PhoneMarkdownSpan {
    text: String,
    style: PhoneMarkdownStyle,
}

/// The pure reader resolves nested styles and late reference definitions.
pub type PhoneMarkdownStyle = Style;
#[uniffi::remote(Record)]
struct PhoneMarkdownStyle {
    strong: bool,
    emphasis: bool,
    code: bool,
    strike: bool,
    link: Option<String>,
}

#[uniffi::export]
impl PhoneDocument {
    /// Parse the entire accepted source off the UI thread; network chunk boundaries are irrelevant.
    #[uniffi::constructor]
    pub fn new(source: String) -> Result<Arc<Self>, PhoneError> {
        Ok(Arc::new(Self {
            blocks: markdown::parse(&source)?,
        }))
    }

    /// Preserve user/reasoning text literally while using the same lazy native block reader.
    #[uniffi::constructor]
    pub fn plain(source: String) -> Result<Arc<Self>, PhoneError> {
        if source.len() > 64 * 1024 * 1024 {
            return Err(PhoneError::Capacity {
                message: "Plain source exceeds 64 MiB; retain original bytes".into(),
            });
        }
        Ok(Arc::new(Self {
            blocks: vec![Block {
                kind: BlockKind::Paragraph,
                indent: 0,
                quote_depth: 0,
                marker: None,
                spans: vec![Span {
                    text: source,
                    style: Style::default(),
                }],
                cells: Vec::new(),
            }],
        }))
    }

    /// Let a lazy native list address immutable blocks without copying all their text.
    pub fn block_count(&self) -> u64 {
        self.blocks.len() as u64
    }

    /// Read one currently visible block, retaining no second growing native document snapshot.
    pub fn block(&self, index: u32) -> Result<PhoneMarkdownBlock, PhoneError> {
        self.blocks
            .get(index as usize)
            .cloned()
            .ok_or_else(|| PhoneError::InvalidInput {
                message: "Markdown block is outside this document".into(),
            })
    }
}
