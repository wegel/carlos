// ---------------------------------------------------------------------------
// Perf overlay
// ---------------------------------------------------------------------------

pub(super) fn draw_perf_overlay(buf: &mut Buffer, size: TerminalSize, perf: &PerfMetrics) {
    let lines = perf.overlay_lines();
    if lines.is_empty() || size.width < 44 || size.height < lines.len() + 4 {
        return;
    }

    let inner_w = lines
        .iter()
        .map(|line| visual_width(line))
        .max()
        .unwrap_or(0)
        .min(size.width.saturating_sub(6));
    if inner_w == 0 {
        return;
    }

    let box_w = inner_w + 4;
    let box_h = lines.len() + 2;
    let start_x = size.width.saturating_sub(box_w + 2);
    let start_y = 1usize;
    if start_y + box_h > size.height {
        return;
    }

    fill_rect(
        buf,
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP1),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;
    draw_box_border(
        buf,
        left,
        top,
        right,
        bottom,
        Style::default().fg(COLOR_STEP7),
        ("┌", "┐", "└", "┘"),
        "─",
        "│",
    );

    for (i, line) in lines.iter().enumerate() {
        draw_str(
            buf,
            start_x + 2,
            start_y + 1 + i,
            line,
            Style::default().fg(COLOR_DIM),
            inner_w,
        );
    }
}
