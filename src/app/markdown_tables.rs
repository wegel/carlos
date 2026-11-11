//! Converts only tables that cannot fit their minimum columns into labelled paragraphs.

use std::borrow::Cow;
use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui_core::text::Span;
use unicode_segmentation::UnicodeSegmentation;

struct Table<'a> {
    range: Range<usize>,
    rows: Vec<Vec<&'a str>>,
    minimum: Vec<usize>,
    prefix: &'a str,
    prefix_width: usize,
}

/// Preserve ordinary tables for upstream; linearize only an impossible column budget.
pub(super) fn table_source(text: &str, width: usize) -> Option<Cow<'_, str>> {
    if !text.contains('|') || !text.contains('\n') {
        return None;
    }
    let tables = tables(text);
    if tables.is_empty() {
        return None;
    }
    let mut source = String::new();
    let mut offset = 0;
    for table in tables {
        let minimum = 3 * table.minimum.len() + 1 + table.minimum.iter().sum::<usize>();
        if minimum + table.prefix_width <= width {
            continue;
        }
        source.push_str(&text[offset..table.range.start]);
        source.push_str(&labelled(&table));
        offset = table.range.end;
    }
    if offset == 0 {
        return Some(Cow::Borrowed(text));
    }
    source.push_str(&text[offset..]);
    Some(Cow::Owned(source))
}

fn tables(text: &str) -> Vec<Table<'_>> {
    let mut tables = Vec::new();
    let mut table: Option<Table<'_>> = None;
    let mut column = 0;
    let mut context = Context::default();
    for (event, range) in Parser::new_ext(text, parse_options()).into_offset_iter() {
        context.event(&event);
        if let (Some(table), Some(value)) = (&mut table, visible_value(&event)) {
            if let Some(minimum) = table.minimum.get_mut(column) {
                *minimum = (*minimum).max(
                    value
                        .graphemes(true)
                        .map(|g| Span::raw(g).width())
                        .max()
                        .unwrap_or(1),
                );
            }
        }
        match event {
            Event::Start(Tag::Table(alignments)) => {
                table = Some(Table {
                    prefix: line_prefix(text, range.start),
                    range,
                    rows: Vec::new(),
                    minimum: vec![1; alignments.len()],
                    prefix_width: context.width(),
                });
            }
            Event::End(TagEnd::Table) => {
                if let Some(mut complete) = table.take() {
                    complete.range.end = range.end;
                    tables.push(complete);
                }
            }
            Event::Start(Tag::TableHead | Tag::TableRow) => {
                if let Some(table) = &mut table {
                    if !table.rows.is_empty() {
                        table.prefix = line_prefix(text, range.start);
                    }
                    table.rows.push(Vec::new());
                    column = 0;
                }
            }
            Event::Start(Tag::TableCell) => {
                if let Some(row) = table.as_mut().and_then(|table| table.rows.last_mut()) {
                    row.push(text[range].trim());
                }
            }
            Event::End(TagEnd::TableCell) => column += 1,
            _ => {}
        }
    }
    tables
}

fn visible_value<'a>(event: &'a Event<'_>) -> Option<&'a str> {
    match event {
        Event::Text(value)
        | Event::Code(value)
        | Event::Html(value)
        | Event::InlineHtml(value)
        | Event::InlineMath(value)
        | Event::DisplayMath(value)
        | Event::FootnoteReference(value)
        | Event::Start(
            Tag::Link {
                dest_url: value, ..
            }
            | Tag::Image {
                dest_url: value, ..
            },
        ) => Some(value),
        _ => None,
    }
}

#[derive(Default)]
struct Context {
    quotes: usize,
    lists: Vec<Option<u64>>,
    items: Vec<usize>,
}

impl Context {
    fn event(&mut self, event: &Event<'_>) {
        match event {
            Event::Start(Tag::BlockQuote(_)) => self.quotes += 1,
            Event::End(TagEnd::BlockQuote(_)) => self.quotes = self.quotes.saturating_sub(1),
            Event::Start(Tag::List(index)) => self.lists.push(*index),
            Event::End(TagEnd::List(_)) => {
                self.lists.pop();
            }
            Event::Start(Tag::Item) => {
                let width = self.lists.len().saturating_mul(4).saturating_sub(3);
                let marker = match self.lists.last_mut() {
                    Some(Some(index)) => {
                        let marker = format!("{:width$}. ", *index);
                        *index = index.saturating_add(1);
                        marker.len()
                    }
                    _ => width + 1,
                };
                self.items.push(marker);
            }
            Event::End(TagEnd::Item) => {
                self.items.pop();
            }
            _ => {}
        }
    }

    fn width(&self) -> usize {
        // Upstream reserves one quote marker each, one separator, and the list marker's width.
        self.quotes + usize::from(self.quotes > 0) + self.items.last().copied().unwrap_or(0)
    }
}

fn line_prefix(text: &str, offset: usize) -> &str {
    let start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    &text[start..offset]
}

fn labelled(table: &Table<'_>) -> String {
    let prefix: String = table
        .prefix
        .chars()
        .map(|ch| {
            if ch == '>' || ch.is_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect();
    let header = table.rows.first().map(Vec::as_slice).unwrap_or_default();
    let mut lines = Vec::new();
    if table.rows.len() == 1 {
        for (column, label) in header.iter().enumerate() {
            lines.push(format!("• Column {}: {label}", column + 1));
        }
    }
    for (index, row) in table.rows.iter().skip(1).enumerate() {
        lines.push(format!("**Row {}**", index + 1));
        for column in 0..table.minimum.len() {
            let label = header
                .get(column)
                .filter(|label| !label.is_empty())
                .map_or_else(
                    || format!("Column {}", column + 1),
                    |label| (*label).to_string(),
                );
            let value = row.get(column).copied().unwrap_or_default();
            lines.push(format!("• {label}: {value}"));
        }
    }
    // A table is a block even when adjacent prose has no blank source line.
    format!(
        "\n{prefix}\n{prefix}{}\n{prefix}\n",
        lines.join(&format!("\n{prefix}\n{prefix}"))
    )
}

fn parse_options() -> Options {
    // Match upstream 0.3.13 without enabling unrelated smart punctuation.
    Options::ENABLE_TABLES
        | Options::ENABLE_GFM
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_SUPERSCRIPT
        | Options::ENABLE_SUBSCRIPT
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH
        | Options::ENABLE_DEFINITION_LIST
}
