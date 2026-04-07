//! Input layout computation and textarea input helpers.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui_textarea::{Input as TextInput, Key as TextKey};

use super::text::{char_to_byte_idx, visual_width, wrap_input_line};
use super::transcript_render::transcript_content_width;
use super::{AppState, Message, Role, TerminalSize, MSG_CONTENT_X};

// --- Types ---

#[derive(Debug, Clone)]
pub(super) struct InputLayout {
    pub(super) msg_bottom: usize, // 1-based; 0 means no transcript row is available
    pub(super) input_top: usize,  // 1-based
    pub(super) input_height: usize, // rows
    pub(super) text_width: usize, // cells available for input text
    pub(super) visible_lines: Vec<String>,
    pub(super) cursor_x: usize, // 0-based terminal column
    pub(super) cursor_y: usize, // 0-based terminal row
}

// --- Input helpers ---

pub(super) fn input_cursor_visual_position(
    line: &str,
    cursor_col_chars: usize,
    width: usize,
) -> (usize, usize) {
    if width == 0 {
        return (0, 0);
    }

    let cursor_byte = char_to_byte_idx(line, cursor_col_chars).min(line.len());
    let prefix = &line[..cursor_byte];
    let wrapped_prefix = wrap_input_line(prefix, width);
    let row = wrapped_prefix.len().saturating_sub(1);
    let col = wrapped_prefix
        .last()
        .map(|part| visual_width(part))
        .unwrap_or(0);
    (row, col)
}

pub(super) fn textarea_input_from_code(code: KeyCode, modifiers: KeyModifiers) -> TextInput {
    let key = match code {
        KeyCode::Char(c) => TextKey::Char(c),
        KeyCode::Backspace => TextKey::Backspace,
        KeyCode::Enter => TextKey::Enter,
        KeyCode::Left => TextKey::Left,
        KeyCode::Right => TextKey::Right,
        KeyCode::Up => TextKey::Up,
        KeyCode::Down => TextKey::Down,
        KeyCode::Tab => TextKey::Tab,
        KeyCode::Delete => TextKey::Delete,
        KeyCode::Home => TextKey::Home,
        KeyCode::End => TextKey::End,
        KeyCode::PageUp => TextKey::PageUp,
        KeyCode::PageDown => TextKey::PageDown,
        KeyCode::Esc => TextKey::Esc,
        KeyCode::F(n) => TextKey::F(n),
        _ => TextKey::Null,
    };

    TextInput {
        key,
        ctrl: modifiers.contains(KeyModifiers::CONTROL),
        alt: modifiers.contains(KeyModifiers::ALT),
        shift: modifiers.contains(KeyModifiers::SHIFT),
    }
}

pub(super) fn textarea_input_from_key(k: crossterm::event::KeyEvent) -> TextInput {
    textarea_input_from_code(k.code, k.modifiers)
}

pub(super) fn normalize_pasted_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(super) fn is_newline_enter(mods: KeyModifiers) -> bool {
    mods.contains(KeyModifiers::SHIFT) || mods.contains(KeyModifiers::ALT)
}

// --- Input layout computation ---

/// Wrap all input lines and track the cursor position within the wrapped output.
fn wrap_input_with_cursor(app: &AppState, text_width: usize) -> (Vec<String>, usize, usize, bool) {
    let mut wrapped = Vec::new();
    let (cursor_row, cursor_col_chars) = app.input.cursor();
    let mut cursor_wrapped_row = 0usize;
    let mut cursor_wrapped_col = 0usize;
    let mut cursor_set = false;

    for (row, line) in app.input.lines().iter().enumerate() {
        let wrapped_line = wrap_input_line(line, text_width);
        if row < cursor_row {
            cursor_wrapped_row += wrapped_line.len();
        } else if row == cursor_row {
            let (r, c) = input_cursor_visual_position(line, cursor_col_chars, text_width);
            cursor_wrapped_row += r;
            cursor_wrapped_col = c;
            cursor_set = true;
        }
        wrapped.extend(wrapped_line);
    }
    if wrapped.is_empty() {
        wrapped.push(String::new());
    }
    (wrapped, cursor_wrapped_row, cursor_wrapped_col, cursor_set)
}

pub(super) fn compute_input_layout(app: &AppState, size: TerminalSize) -> InputLayout {
    let text_width = transcript_content_width(size);
    let (wrapped, mut cursor_wrapped_row, mut cursor_wrapped_col, cursor_set) =
        wrap_input_with_cursor(app, text_width);

    let mut max_input_rows = 8usize.min(size.height.max(1));
    if size.height > 1 {
        max_input_rows = max_input_rows.min(size.height - 1);
    }
    let input_height = wrapped.len().clamp(1, max_input_rows.max(1));

    if !cursor_set {
        cursor_wrapped_row = wrapped.len().saturating_sub(1);
        cursor_wrapped_col = wrapped.last().map(|l| visual_width(l)).unwrap_or(0);
    }

    let mut visible_start = wrapped.len().saturating_sub(input_height);
    if cursor_wrapped_row < visible_start {
        visible_start = cursor_wrapped_row;
    }
    if cursor_wrapped_row >= visible_start + input_height {
        visible_start = cursor_wrapped_row + 1 - input_height;
    }

    let visible_end = (visible_start + input_height).min(wrapped.len());
    let mut visible_lines = wrapped[visible_start..visible_end].to_vec();
    while visible_lines.len() < input_height {
        visible_lines.insert(0, String::new());
    }

    let input_top = size.height + 1 - input_height;
    let msg_bottom = input_top.saturating_sub(2);
    let cursor_visual_row = cursor_wrapped_row.saturating_sub(visible_start);
    let cursor_x = MSG_CONTENT_X + cursor_wrapped_col;
    let cursor_y = input_top.saturating_sub(1) + cursor_visual_row.min(input_height - 1);

    InputLayout {
        msg_bottom,
        input_top,
        input_height,
        text_width,
        visible_lines,
        cursor_x,
        cursor_y,
    }
}

// --- Transcript helpers ---

pub(super) fn last_assistant_message(messages: &[Message]) -> Option<&str> {
    messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, Role::Assistant) && !m.text.is_empty())
        .map(|m| m.text.as_str())
}
