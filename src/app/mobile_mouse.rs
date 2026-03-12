use super::AppState;

const MOBILE_PLAIN_SCROLL_STEP: usize = 3;

pub(super) enum MobileMouseConsume {
    PassThrough,
    Consumed,
    Emit(String),
}

pub(super) fn parse_mobile_mouse_coords(s: &str) -> Option<(usize, usize)> {
    if !s.contains(';') {
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

pub(super) fn apply_mobile_mouse_scroll(app: &mut AppState, y: usize) {
    if let Some(prev) = app.mobile_mouse_last_y {
        app.auto_follow_bottom = false;
        let step = y.abs_diff(prev).min(8);
        let step = step.max(1);
        let drag_down = y > prev;
        let move_down_history = if app.scroll_inverted {
            !drag_down
        } else {
            drag_down
        };
        if move_down_history {
            app.scroll_top = app.scroll_top.saturating_add(step);
        } else if y != prev {
            app.scroll_top = app.scroll_top.saturating_sub(step);
        }
    }
    app.mobile_mouse_last_y = Some(y);
}

fn apply_mobile_plain_scroll(app: &mut AppState, y: usize) {
    let direction = if app.mobile_plain_new_gesture {
        app.mobile_plain_new_gesture = false;
        match app.mobile_plain_last_direction {
            0 => match app.mobile_mouse_last_y {
                Some(prev) if y > prev => 1,
                Some(prev) if y < prev => -1,
                _ => 0,
            },
            prev_dir => prev_dir,
        }
    } else {
        match app.mobile_mouse_last_y {
            Some(prev) if y > prev => 1,
            Some(prev) if y < prev => -1,
            _ => app.mobile_plain_last_direction,
        }
    };

    app.auto_follow_bottom = false;
    let move_down_history = if app.scroll_inverted {
        direction < 0
    } else {
        direction > 0
    };
    if move_down_history {
        app.scroll_top = app.scroll_top.saturating_add(MOBILE_PLAIN_SCROLL_STEP);
    } else if direction != 0 {
        app.scroll_top = app.scroll_top.saturating_sub(MOBILE_PLAIN_SCROLL_STEP);
    }

    app.mobile_mouse_last_y = Some(y);
    app.mobile_plain_last_direction = direction;
}

pub(super) fn take_mobile_mouse_buffer(app: &mut AppState) -> Option<String> {
    if app.mobile_mouse_buffer.is_empty() {
        return None;
    }
    Some(std::mem::take(&mut app.mobile_mouse_buffer))
}

pub(super) fn consume_mobile_mouse_char(app: &mut AppState, c: char) -> MobileMouseConsume {
    if app.mobile_mouse_buffer.is_empty() {
        if (app.mobile_plain_pending_coords || app.mobile_plain_suppress_coords)
            && (c.is_ascii_digit() || c == ';')
        {
            app.mobile_mouse_buffer.push(c);
            return MobileMouseConsume::Consumed;
        }
        // Activate only on explicit CSI/SGR-style prefix to avoid swallowing normal typing.
        if c == '<' || c == '[' {
            app.mobile_mouse_buffer.push(c);
            return MobileMouseConsume::Consumed;
        }
        return MobileMouseConsume::PassThrough;
    }

    let valid = if app.mobile_plain_pending_coords || app.mobile_plain_suppress_coords {
        c.is_ascii_digit() || c == ';'
    } else {
        c.is_ascii_digit() || c == ';' || c == 'M' || c == 'm' || c == '<' || c == '['
    };
    if !valid {
        let mut out = std::mem::take(&mut app.mobile_mouse_buffer);
        app.mobile_plain_pending_coords = false;
        app.mobile_plain_suppress_coords = false;
        app.mobile_plain_last_direction = 0;
        app.mobile_plain_new_gesture = false;
        out.push(c);
        return MobileMouseConsume::Emit(out);
    }

    app.mobile_mouse_buffer.push(c);

    if app.mobile_plain_pending_coords || app.mobile_plain_suppress_coords {
        if let Some((_, y)) = parse_plain_mobile_pair(&app.mobile_mouse_buffer) {
            if app.mobile_plain_pending_coords {
                apply_mobile_plain_scroll(app, y);
            }
            app.mobile_mouse_buffer.clear();
            app.mobile_plain_pending_coords = false;
            app.mobile_plain_suppress_coords = false;
            return MobileMouseConsume::Consumed;
        }
        if app.mobile_mouse_buffer.len() > 8 {
            let emit = app.mobile_plain_pending_coords;
            app.mobile_plain_pending_coords = false;
            app.mobile_plain_suppress_coords = false;
            app.mobile_plain_last_direction = 0;
            app.mobile_plain_new_gesture = false;
            if emit {
                return MobileMouseConsume::Emit(std::mem::take(&mut app.mobile_mouse_buffer));
            }
            let _ = std::mem::take(&mut app.mobile_mouse_buffer);
            return MobileMouseConsume::Consumed;
        }
        return MobileMouseConsume::Consumed;
    }

    // Apply only on explicit terminator to reduce false positives while typing.
    if c == 'M' || c == 'm' {
        if let Some((_, y)) = parse_mobile_mouse_coords(&app.mobile_mouse_buffer) {
            apply_mobile_mouse_scroll(app, y);
            let _ = std::mem::take(&mut app.mobile_mouse_buffer);
            return MobileMouseConsume::Consumed;
        }
        return MobileMouseConsume::Emit(std::mem::take(&mut app.mobile_mouse_buffer));
    }

    if app.mobile_mouse_buffer.len() > 24 {
        return MobileMouseConsume::Emit(std::mem::take(&mut app.mobile_mouse_buffer));
    }

    MobileMouseConsume::Consumed
}

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
        let unit = &s[..unit_len];
        let Some((x, y)) = parse_plain_mobile_pair(unit) else {
            continue;
        };
        let mut idx = 0usize;
        let mut repeats = 0usize;
        while idx + unit_len <= s.len() && &s[idx..idx + unit_len] == unit {
            idx += unit_len;
            repeats += 1;
        }
        if repeats >= 2 && idx == s.len() {
            return Some((x, y));
        }
    }

    None
}
