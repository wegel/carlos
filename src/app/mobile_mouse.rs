//! Mobile terminal mouse-event parsing and scroll emulation.

use crossterm::event::{KeyCode, KeyModifiers};

use super::viewport_state::ViewportState;

// --- Mouse Types ---

const MOBILE_PLAIN_SCROLL_STEP: usize = 3;

pub(super) enum MobileMouseConsume {
    PassThrough,
    Consumed,
    Emit(String),
}

// --- Coord Parsing ---

pub(super) fn parse_mobile_mouse_coords(s: &str) -> Option<(usize, usize)> {
    if s.len() > 24
        || !s.contains(';')
        || !s.bytes().all(|byte| {
            byte.is_ascii_digit() || matches!(byte, b';' | b'<' | b'[' | b'M' | b'm' | 0x1b)
        })
    {
        return None;
    }

    let nums: Vec<usize> = s
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<usize>().ok())
        .collect();
    if nums.len() < 2 {
        return None;
    }

    Some((nums[nums.len() - 2], nums[nums.len() - 1]))
}

// --- Scroll Helpers ---

pub(super) fn apply_mobile_mouse_scroll(viewport: &mut ViewportState, y: usize) {
    if let Some(prev) = viewport.mobile_mouse_last_y {
        viewport.auto_follow_bottom = false;
        let step = y.abs_diff(prev).min(8);
        let step = step.max(1);
        let drag_down = y > prev;
        let move_down_history = if viewport.scroll_inverted {
            !drag_down
        } else {
            drag_down
        };
        if move_down_history {
            viewport.scroll_top = viewport.scroll_top.saturating_add(step);
        } else if y != prev {
            viewport.scroll_top = viewport.scroll_top.saturating_sub(step);
        }
    }
    viewport.mobile_mouse_last_y = Some(y);
}

fn apply_mobile_plain_scroll(viewport: &mut ViewportState, y: usize) {
    let direction = if viewport.mobile_plain_new_gesture {
        viewport.mobile_plain_new_gesture = false;
        match viewport.mobile_plain_last_direction {
            0 => match viewport.mobile_mouse_last_y {
                Some(prev) if y > prev => 1,
                Some(prev) if y < prev => -1,
                _ => 0,
            },
            prev_dir => prev_dir,
        }
    } else {
        match viewport.mobile_mouse_last_y {
            Some(prev) if y > prev => 1,
            Some(prev) if y < prev => -1,
            _ => viewport.mobile_plain_last_direction,
        }
    };

    viewport.auto_follow_bottom = false;
    let move_down_history = if viewport.scroll_inverted {
        direction < 0
    } else {
        direction > 0
    };
    if move_down_history {
        viewport.scroll_top = viewport.scroll_top.saturating_add(MOBILE_PLAIN_SCROLL_STEP);
    } else if direction != 0 {
        viewport.scroll_top = viewport.scroll_top.saturating_sub(MOBILE_PLAIN_SCROLL_STEP);
    }

    viewport.mobile_mouse_last_y = Some(y);
    viewport.mobile_plain_last_direction = direction;
}

// --- Public API ---

pub(super) fn take_mobile_mouse_buffer(viewport: &mut ViewportState) -> Option<String> {
    if viewport.mobile_mouse_buffer.is_empty() {
        return None;
    }
    Some(std::mem::take(&mut viewport.mobile_mouse_buffer))
}

pub(super) fn consume_mobile_mouse_char(
    viewport: &mut ViewportState,
    c: char,
) -> MobileMouseConsume {
    if viewport.mobile_mouse_buffer.is_empty() {
        return consume_first_char(viewport, c);
    }
    if !is_valid_continuation(viewport, c) {
        let mut out = std::mem::take(&mut viewport.mobile_mouse_buffer);
        reset_plain_state(viewport);
        out.push(c);
        return MobileMouseConsume::Emit(out);
    }
    viewport.mobile_mouse_buffer.push(c);
    if viewport.mobile_plain_pending_coords || viewport.mobile_plain_suppress_coords {
        return consume_plain_coords_char(viewport);
    }
    consume_csi_sgr_char(viewport, c)
}

// --- State Machine ---

fn consume_first_char(viewport: &mut ViewportState, c: char) -> MobileMouseConsume {
    let in_plain = viewport.mobile_plain_pending_coords || viewport.mobile_plain_suppress_coords;
    if in_plain && (c.is_ascii_digit() || c == ';') {
        viewport.mobile_mouse_buffer.push(c);
        return MobileMouseConsume::Consumed;
    }
    // Activate only on explicit CSI/SGR-style prefix to avoid swallowing normal typing.
    if c == '<' || c == '[' {
        viewport.mobile_mouse_buffer.push(c);
        return MobileMouseConsume::Consumed;
    }
    MobileMouseConsume::PassThrough
}

fn is_valid_continuation(viewport: &ViewportState, c: char) -> bool {
    if viewport.mobile_plain_pending_coords || viewport.mobile_plain_suppress_coords {
        c.is_ascii_digit() || c == ';'
    } else {
        matches!(c, '0'..='9' | ';' | 'M' | 'm' | '<' | '[')
    }
}

fn consume_plain_coords_char(viewport: &mut ViewportState) -> MobileMouseConsume {
    if let Some((_, y)) = parse_plain_mobile_pair(&viewport.mobile_mouse_buffer) {
        if viewport.mobile_plain_pending_coords {
            apply_mobile_plain_scroll(viewport, y);
        }
        viewport.mobile_mouse_buffer.clear();
        viewport.mobile_plain_pending_coords = false;
        viewport.mobile_plain_suppress_coords = false;
        return MobileMouseConsume::Consumed;
    }
    if viewport.mobile_mouse_buffer.len() > 8 {
        let emit = viewport.mobile_plain_pending_coords;
        reset_plain_state(viewport);
        if emit {
            return MobileMouseConsume::Emit(std::mem::take(&mut viewport.mobile_mouse_buffer));
        }
        viewport.mobile_mouse_buffer.clear();
        return MobileMouseConsume::Consumed;
    }
    MobileMouseConsume::Consumed
}

/// Handle CSI/SGR-style mouse sequences — apply on explicit terminator.
fn consume_csi_sgr_char(viewport: &mut ViewportState, c: char) -> MobileMouseConsume {
    if c == 'M' || c == 'm' {
        if let Some((_, y)) = parse_mobile_mouse_coords(&viewport.mobile_mouse_buffer) {
            apply_mobile_mouse_scroll(viewport, y);
            viewport.mobile_mouse_buffer.clear();
            return MobileMouseConsume::Consumed;
        }
        return MobileMouseConsume::Emit(std::mem::take(&mut viewport.mobile_mouse_buffer));
    }
    if viewport.mobile_mouse_buffer.len() > 24 {
        return MobileMouseConsume::Emit(std::mem::take(&mut viewport.mobile_mouse_buffer));
    }
    MobileMouseConsume::Consumed
}

fn reset_plain_state(viewport: &mut ViewportState) {
    viewport.mobile_plain_pending_coords = false;
    viewport.mobile_plain_suppress_coords = false;
    viewport.mobile_plain_last_direction = 0;
    viewport.mobile_plain_new_gesture = false;
}

// --- Plain Parsing ---

fn parse_plain_mobile_pair(s: &str) -> Option<(usize, usize)> {
    let mut parts = s.split(';');
    let x = parts.next()?;
    let y = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if x.len() < 2 || x.len() > 3 || y.len() < 2 || y.len() > 3 {
        return None;
    }
    let x = x.parse::<usize>().ok()?;
    let y = y.parse::<usize>().ok()?;
    Some((x, y))
}

pub(super) fn parse_repeated_plain_mobile_pair(s: &str) -> Option<(usize, usize)> {
    if s.len() < 10 || s.contains(char::is_whitespace) {
        return None;
    }

    for unit_len in 5..=7 {
        if s.len() < unit_len * 2 {
            continue;
        }
        let Some(unit) = s.get(..unit_len) else {
            continue;
        };
        let Some((x, y)) = parse_plain_mobile_pair(unit) else {
            continue;
        };
        let mut idx = 0usize;
        let mut repeats = 0usize;
        while idx + unit_len <= s.len() && s.get(idx..idx + unit_len) == Some(unit) {
            idx += unit_len;
            repeats += 1;
        }
        if repeats >= 2 && idx == s.len() {
            return Some((x, y));
        }
    }

    None
}

/// Capture the next broken mobile coordinate pair after its modified middle click.
pub(super) fn begin_plain(viewport: &mut ViewportState) {
    viewport.mobile_plain_pending_coords = true;
    viewport.mobile_plain_suppress_coords = false;
    viewport.mobile_plain_new_gesture = true;
    viewport.mobile_mouse_buffer.clear();
}

/// Recognize mouse fragments without swallowing ordinary unmodified digits.
pub(super) fn is_mobile_mouse_key_candidate(
    viewport: &ViewportState,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    if viewport.mobile_plain_pending_coords || viewport.mobile_plain_suppress_coords {
        return matches!(code, KeyCode::Char(ch) if ch.is_ascii_digit() || ch == ';');
    }
    if !viewport.mobile_mouse_buffer.is_empty() {
        return matches!(code, KeyCode::Char(_));
    }
    let KeyCode::Char(ch) = code else {
        return false;
    };
    ch == '<'
        || (modifiers.contains(KeyModifiers::ALT)
            && matches!(ch, '[' | '<' | ';' | 'M' | 'm' | '0'..='9'))
}

/// Preserve pasted mobile gestures while leaving ordinary nonempty drafts alone.
pub(super) fn consume_paste(viewport: &mut ViewportState, empty: bool, pasted: &str) -> bool {
    let coords = parse_repeated_plain_mobile_pair(pasted)
        .or_else(|| empty.then(|| parse_mobile_mouse_coords(pasted)).flatten());
    if let Some((_, y)) = coords {
        apply_mobile_mouse_scroll(viewport, y);
        return true;
    }
    false
}

#[cfg(test)]
#[path = "mobile_mouse_tests.rs"]
mod tests;
