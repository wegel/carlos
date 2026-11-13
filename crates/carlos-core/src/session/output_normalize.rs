//! Strip split terminal controls and classify split diff lines with bounded state.

use carlos_proto::output::OutputSegment;
use carlos_proto::session::OutputKind;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum Control {
    #[default]
    Ground,
    Escape,
    Csi,
    Osc,
    OscEscape,
    String,
    StringEscape,
    Intermediate,
}

impl Control {
    pub(super) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Ground),
            1 => Some(Self::Escape),
            2 => Some(Self::Csi),
            3 => Some(Self::Osc),
            4 => Some(Self::OscEscape),
            5 => Some(Self::String),
            6 => Some(Self::StringEscape),
            7 => Some(Self::Intermediate),
            _ => None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct ParseState {
    pub(super) control: Control,
    pub(super) line: u8,
    pub(super) prefix: u8,
}

impl ParseState {
    pub(super) fn valid(self) -> bool {
        self.line <= 5 && self.prefix <= 5 && (self.line == 0 || self.prefix == 0)
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
            Control::Ground => match ch {
                '\u{1b}' => self.control = Control::Escape,
                '\u{9b}' => self.control = Control::Csi,
                '\u{9d}' => self.control = Control::Osc,
                '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}' => self.control = Control::String,
                '\n' | '\r' | '\t' => return Some(ch),
                ch if !ch.is_control() => return Some(ch),
                _ => {}
            },
            Control::Escape => return self.escape_char(ch),
            Control::Csi => {
                if ch == '\u{1b}' {
                    self.control = Control::Escape;
                } else if ('@'..='~').contains(&ch) || matches!(ch, '\u{18}' | '\u{1a}') {
                    self.control = Control::Ground;
                }
            }
            Control::Intermediate => {
                if ch == '\u{1b}' {
                    self.control = Control::Escape;
                } else if ('0'..='~').contains(&ch) {
                    self.control = Control::Ground;
                }
            }
            _ => self.string_char(ch),
        }
        None
    }

    fn escape_char(&mut self, ch: char) -> Option<char> {
        self.control = match ch {
            '[' => Control::Csi,
            ']' => Control::Osc,
            'P' | 'X' | '^' | '_' => Control::String,
            ' '..='/' => Control::Intermediate,
            _ => Control::Ground,
        };
        if !ch.is_ascii() {
            self.plain_char(ch)
        } else {
            None
        }
    }

    fn string_char(&mut self, ch: char) {
        let osc = matches!(self.control, Control::Osc | Control::OscEscape);
        let escaped = matches!(self.control, Control::OscEscape | Control::StringEscape);
        self.control = if ch == '\u{9c}' || osc && ch == '\u{7}' || escaped && ch == '\\' {
            Control::Ground
        } else if ch == '\u{1b}' {
            if osc {
                Control::OscEscape
            } else {
                Control::StringEscape
            }
        } else if osc {
            Control::Osc
        } else {
            Control::String
        };
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
