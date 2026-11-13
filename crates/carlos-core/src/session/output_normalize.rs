//! Strip split terminal controls and classify split diff lines with bounded state.

use carlos_proto::output::OutputSegment;
use carlos_proto::session::OutputKind;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct ParseState {
    pub(super) control: u8,
    pub(super) line: u8,
    pub(super) prefix: u8,
}

impl ParseState {
    pub(super) fn valid(self) -> bool {
        self.control <= 6
            && self.line <= 5
            && self.prefix <= 5
            && (self.line == 0 || self.prefix == 0)
    }

    pub(super) fn feed(&mut self, ch: char, diff: bool, out: &mut Vec<OutputSegment>) {
        if let Some(ch) = self.plain_char(ch) {
            self.line_char(ch, diff, out);
        }
    }

    pub(super) fn finish(&mut self, out: &mut Vec<OutputSegment>) {
        if self.prefix != 0 {
            let text = prefix_text(self.prefix);
            push(out, prefix_kind(self.prefix), text);
            self.line = kind_tag(prefix_kind(self.prefix));
            self.prefix = 0;
        }
    }

    fn plain_char(&mut self, ch: char) -> Option<char> {
        match self.control {
            0 => match ch {
                '\u{1b}' => self.control = 1,
                '\u{9b}' => self.control = 2,
                '\n' | '\r' | '\t' => return Some(ch),
                ch if !ch.is_control() => return Some(ch),
                _ => {}
            },
            1 => {
                self.control = match ch {
                    '[' => 2,
                    ']' => 3,
                    'P' | 'X' | '^' | '_' => 5,
                    _ => 0,
                };
                if !ch.is_ascii() {
                    return self.plain_char(ch);
                }
            }
            2 => {
                if ('@'..='~').contains(&ch) {
                    self.control = 0;
                }
            }
            3 => match ch {
                '\u{7}' => self.control = 0,
                '\u{1b}' => self.control = 4,
                _ => {}
            },
            4 => {
                self.control = match ch {
                    '\\' | '\u{7}' => 0,
                    '\u{1b}' => 4,
                    _ => 3,
                }
            }
            5 => {
                if ch == '\u{1b}' {
                    self.control = 6;
                }
            }
            6 => {
                self.control = match ch {
                    '\\' => 0,
                    '\u{1b}' => 6,
                    _ => 5,
                }
            }
            _ => unreachable!("cursor state validated"),
        }
        None
    }

    fn line_char(&mut self, ch: char, diff: bool, out: &mut Vec<OutputSegment>) {
        if self.line != 0 {
            push_char(out, tag_kind(self.line), ch);
        } else if !diff {
            self.line = 1;
            push_char(out, OutputKind::Plain, ch);
        } else {
            self.diff_char(ch, out);
        }
        if ch == '\n' {
            self.line = 0;
            self.prefix = 0;
        }
    }

    fn diff_char(&mut self, ch: char, out: &mut Vec<OutputSegment>) {
        let prefix = self.prefix;
        let next = match (prefix, ch) {
            (0, '+') => Some(1),
            (1, '+') => Some(2),
            (0, '-') => Some(3),
            (3, '-') => Some(4),
            (0, '@') => Some(5),
            _ => None,
        };
        if let Some(next) = next {
            self.prefix = next;
            return;
        }
        let kind = match (prefix, ch) {
            (2, '+') | (4, '-') => OutputKind::Context,
            (5, '@') => OutputKind::Hunk,
            (0 | 5, _) => OutputKind::Context,
            _ => prefix_kind(prefix),
        };
        self.prefix = 0;
        self.line = kind_tag(kind);
        push(out, kind, prefix_text(prefix));
        push_char(out, kind, ch);
    }
}

fn prefix_text(prefix: u8) -> &'static str {
    match prefix {
        1 => "+",
        2 => "++",
        3 => "-",
        4 => "--",
        5 => "@",
        _ => "",
    }
}

fn prefix_kind(prefix: u8) -> OutputKind {
    match prefix {
        1 | 2 => OutputKind::Addition,
        3 | 4 => OutputKind::Deletion,
        _ => OutputKind::Context,
    }
}

fn kind_tag(kind: OutputKind) -> u8 {
    match kind {
        OutputKind::Plain => 1,
        OutputKind::Addition => 2,
        OutputKind::Deletion => 3,
        OutputKind::Hunk => 4,
        _ => 5,
    }
}

fn tag_kind(tag: u8) -> OutputKind {
    match tag {
        1 => OutputKind::Plain,
        2 => OutputKind::Addition,
        3 => OutputKind::Deletion,
        4 => OutputKind::Hunk,
        _ => OutputKind::Context,
    }
}

fn push(out: &mut Vec<OutputSegment>, kind: OutputKind, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut().filter(|last| last.kind == kind) {
        last.text.push_str(text);
    } else {
        out.push(OutputSegment {
            kind,
            text: text.to_owned(),
        });
    }
}

fn push_char(out: &mut Vec<OutputSegment>, kind: OutputKind, ch: char) {
    if let Some(last) = out.last_mut().filter(|last| last.kind == kind) {
        last.text.push(ch);
    } else {
        out.push(OutputSegment {
            kind,
            text: ch.to_string(),
        });
    }
}
