//! Turn CommonMark events into bounded native blocks without HTML execution or platform markup parsing.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use super::{Block, BlockKind, Column, Span, Style};
use crate::ClientError;

#[derive(Default)]
struct State {
    blocks: Vec<Block>,
    current: Option<Block>,
    styles: Vec<Style>,
    lists: Vec<Option<u64>>,
    marker: Option<String>,
    quotes: u16,
    table: Option<Vec<Column>>,
    header: bool,
}

pub(super) fn parse(source: &str) -> Result<Vec<Block>, ClientError> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut state = State::default();
    for (count, event) in Parser::new_ext(source, options).enumerate() {
        if count >= 262144 || state.styles.len() > 64 || state.lists.len() > 64 || state.quotes > 64
        {
            return Err(ClientError::Capacity(
                "Markdown structure exceeds the native display allowance; retain original bytes",
            ));
        }
        state.event(event);
    }
    state.flush();
    Ok(state.blocks)
}

impl State {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                self.text(&text, false)
            }
            Event::Code(text) => self.text(&text, true),
            Event::SoftBreak => self.text(" ", false),
            Event::HardBreak => self.text("\n", false),
            Event::Rule => {
                self.begin(BlockKind::Rule);
                self.flush();
            }
            Event::TaskListMarker(done) => {
                self.block().marker = Some(if done { "☑" } else { "☐" }.into())
            }
            Event::FootnoteReference(label) => self.text(&format!("[{label}]"), false),
            Event::InlineMath(text) | Event::DisplayMath(text) => self.text(&text, false),
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.begin(BlockKind::Paragraph),
            Tag::Heading { level, .. } => self.begin(BlockKind::Heading { level: level as u8 }),
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(value) => {
                        value.split_whitespace().next().map(str::to_owned)
                    }
                    CodeBlockKind::Indented => None,
                };
                self.begin(BlockKind::Code { language });
            }
            Tag::List(start) => {
                self.flush();
                self.lists.push(start);
            }
            Tag::Item => self.item(),
            Tag::BlockQuote(_) => {
                self.flush();
                self.quotes += 1;
            }
            Tag::Table(columns) => self.table(columns),
            Tag::TableHead => {
                self.header = true;
                self.table_row();
            }
            Tag::TableRow => {
                self.header = false;
                self.table_row();
            }
            Tag::TableCell => {
                self.block().cells.push(Vec::new());
            }
            Tag::Emphasis
            | Tag::Strong
            | Tag::Strikethrough
            | Tag::Link { .. }
            | Tag::Image { .. } => self.inline(tag),
            Tag::FootnoteDefinition(label) => {
                self.flush();
                self.marker = Some(format!("[{label}]"));
            }
            Tag::HtmlBlock => self.begin(BlockKind::Paragraph),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::Item
            | TagEnd::FootnoteDefinition
            | TagEnd::TableHead
            | TagEnd::TableRow => self.flush(),
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quotes = self.quotes.saturating_sub(1);
            }
            TagEnd::Table => {
                self.flush();
                self.table = None;
            }
            TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Link
            | TagEnd::Image => {
                self.styles.pop();
            }
            _ => {}
        }
    }

    fn inline(&mut self, tag: Tag<'_>) {
        let mut style = self.styles.last().cloned().unwrap_or_default();
        match tag {
            Tag::Emphasis => style.emphasis = true,
            Tag::Strong => style.strong = true,
            Tag::Strikethrough => style.strike = true,
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                style.link = Some(dest_url.into_string())
            }
            _ => unreachable!("only inline style tags"),
        }
        self.styles.push(style);
    }

    fn item(&mut self) {
        self.flush();
        self.marker = Some(match self.lists.last_mut() {
            Some(Some(next)) => {
                let marker = format!("{next}.");
                *next = next.saturating_add(1);
                marker
            }
            _ => "•".into(),
        });
    }

    fn table(&mut self, columns: Vec<Alignment>) {
        self.flush();
        self.table = Some(
            columns
                .into_iter()
                .map(|column| match column {
                    Alignment::None => Column::Unspecified,
                    Alignment::Left => Column::Left,
                    Alignment::Center => Column::Center,
                    Alignment::Right => Column::Right,
                })
                .collect(),
        );
    }

    fn table_row(&mut self) {
        self.begin(BlockKind::TableRow {
            header: self.header,
            columns: self.table.clone().unwrap_or_default(),
        });
    }

    fn text(&mut self, text: &str, code: bool) {
        let mut style = self.styles.last().cloned().unwrap_or_default();
        style.code |= code;
        let block = self.block();
        let target = block.cells.last_mut().unwrap_or(&mut block.spans);
        if let Some(last) = target.last_mut().filter(|last| last.style == style) {
            last.text.push_str(text);
        } else {
            target.push(Span {
                text: text.into(),
                style,
            });
        }
    }

    fn begin(&mut self, kind: BlockKind) {
        self.flush();
        let mut block = self.new_block();
        block.kind = kind;
        self.current = Some(block);
    }

    fn block(&mut self) -> &mut Block {
        if self.current.is_none() {
            self.current = Some(self.new_block());
        }
        self.current.as_mut().expect("current block")
    }

    fn new_block(&mut self) -> Block {
        Block {
            kind: BlockKind::Paragraph,
            indent: self.lists.len() as u16,
            quote_depth: self.quotes,
            marker: self.marker.take(),
            spans: Vec::new(),
            cells: Vec::new(),
        }
    }

    fn flush(&mut self) {
        if let Some(block) = self.current.take() {
            self.blocks.push(block);
        }
    }
}
