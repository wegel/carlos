use super::AppState;

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
        if y > prev {
            app.scroll_top = app.scroll_top.saturating_add(step.max(1));
        } else if y < prev {
            app.scroll_top = app.scroll_top.saturating_sub(step.max(1));
        }
    }
    app.mobile_mouse_last_y = Some(y);
}

pub(super) fn consume_mobile_mouse_char(app: &mut AppState, c: char) -> bool {
    if !app.input_is_empty() {
        app.mobile_mouse_buffer.clear();
        return false;
    }

    if app.mobile_mouse_buffer.is_empty() {
        if c != '<' {
            return false;
        }
        app.mobile_mouse_buffer.push(c);
        return true;
    }

    if c.is_ascii_digit() || c == ';' || c == 'M' || c == 'm' {
        app.mobile_mouse_buffer.push(c);
        if let Some((_, y)) = parse_mobile_mouse_coords(&app.mobile_mouse_buffer) {
            apply_mobile_mouse_scroll(app, y);
            app.mobile_mouse_buffer.clear();
        } else if app.mobile_mouse_buffer.len() > 24 {
            app.mobile_mouse_buffer.clear();
        }
        return true;
    }

    app.mobile_mouse_buffer.clear();
    false
}
