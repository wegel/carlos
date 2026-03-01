use super::AppState;

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

pub(super) fn take_mobile_mouse_buffer(app: &mut AppState) -> Option<String> {
    if app.mobile_mouse_buffer.is_empty() {
        return None;
    }
    Some(std::mem::take(&mut app.mobile_mouse_buffer))
}

pub(super) fn consume_mobile_mouse_char(app: &mut AppState, c: char) -> MobileMouseConsume {
    if app.mobile_mouse_buffer.is_empty() {
        // Activate only on explicit SGR-style prefix to avoid swallowing normal typing.
        if c == '<' {
            app.mobile_mouse_buffer.push(c);
            return MobileMouseConsume::Consumed;
        }
        return MobileMouseConsume::PassThrough;
    }

    let valid = c.is_ascii_digit() || c == ';' || c == 'M' || c == 'm' || c == '<' || c == '[';
    if !valid {
        let mut out = std::mem::take(&mut app.mobile_mouse_buffer);
        out.push(c);
        return MobileMouseConsume::Emit(out);
    }

    app.mobile_mouse_buffer.push(c);

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
