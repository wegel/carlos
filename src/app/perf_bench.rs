//! Benchmark harness: scroll, typing, working, and append draw benchmarks, plus layout profiling.

use std::time::Instant;

use anyhow::Result;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::models::{Message, MessageKind, Role};
use super::perf::DurationSamples;
use super::render::{compute_input_layout, render_main_view};
use super::transcript_render::{
    count_rendered_block_for_message_cached, transcript_content_width, RenderCountCache,
};
use super::{AppState, TerminalSize, MSG_TOP};

const PERF_SAMPLE_WINDOW: usize = 4096;
const APPEND_BENCH_STEPS: usize = 16;
const MAX_SCROLL_SAMPLES: usize = 64;
const TYPING_BENCH_TEXT: &str = "the quick brown fox jumps over the lazy dog";

pub(super) struct PerfReplayStats {
    pub(super) replay_apply: DurationSamples,
    pub(super) full_layout_ms: f64,
    pub(super) full_draw_ms: f64,
    pub(super) scroll_draw: DurationSamples,
    pub(super) typing_draw: DurationSamples,
    pub(super) working_draw: DurationSamples,
    pub(super) append_total: DurationSamples,
    pub(super) relevant_items: usize,
    pub(super) layout_breakdown: Vec<LayoutBreakdownRow>,
}

impl PerfReplayStats {
    pub(super) fn new() -> Self {
        Self {
            replay_apply: DurationSamples::new(PERF_SAMPLE_WINDOW),
            full_layout_ms: 0.0,
            full_draw_ms: 0.0,
            scroll_draw: DurationSamples::new(PERF_SAMPLE_WINDOW),
            typing_draw: DurationSamples::new(PERF_SAMPLE_WINDOW),
            working_draw: DurationSamples::new(PERF_SAMPLE_WINDOW),
            append_total: DurationSamples::new(PERF_SAMPLE_WINDOW),
            relevant_items: 0,
            layout_breakdown: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct LayoutBreakdownRow {
    pub(super) label: &'static str,
    pub(super) messages: usize,
    pub(super) lines: usize,
    pub(super) total_ms: f64,
}

pub(super) fn benchmark_scroll_draws(
    app: &mut AppState,
    size: TerminalSize,
    stats: &mut PerfReplayStats,
) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(size.width as u16, size.height as u16))?;
    app.ensure_rendered_lines(transcript_content_width(size), None);
    let input_layout = compute_input_layout(app, size);
    let msg_height = if input_layout.msg_bottom >= MSG_TOP {
        input_layout.msg_bottom - MSG_TOP + 1
    } else {
        0
    };
    let max_scroll = app.rendered_line_count().saturating_sub(msg_height);
    let step = ((max_scroll / MAX_SCROLL_SAMPLES.max(1)).max(msg_height / 2)).max(1);
    let old_scroll = app.viewport.scroll_top;
    let old_follow = app.viewport.auto_follow_bottom;

    let mut pos = 0usize;
    while pos <= max_scroll {
        app.viewport.scroll_top = pos;
        app.viewport.auto_follow_bottom = pos >= max_scroll;
        let draw_started = Instant::now();
        terminal.draw(|frame| render_main_view(frame, app))?;
        stats.scroll_draw.push(draw_started.elapsed());
        if pos == max_scroll {
            break;
        }
        pos = (pos + step).min(max_scroll);
    }

    app.viewport.scroll_top = old_scroll.min(max_scroll);
    app.viewport.auto_follow_bottom = old_follow;
    Ok(())
}

pub(super) fn benchmark_typing_draws(
    app: &mut AppState,
    size: TerminalSize,
    stats: &mut PerfReplayStats,
) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(size.width as u16, size.height as u16))?;
    let original = app.input_text();

    for ch in TYPING_BENCH_TEXT.chars() {
        app.input_insert_text(ch.to_string());
        let draw_started = Instant::now();
        terminal.draw(|frame| render_main_view(frame, app))?;
        stats.typing_draw.push(draw_started.elapsed());
    }

    app.set_input_text(&original);
    Ok(())
}

pub(super) fn benchmark_working_draws(
    app: &mut AppState,
    size: TerminalSize,
    stats: &mut PerfReplayStats,
) -> Result<()> {
    const WORKING_DRAW_SAMPLES: usize = 64;

    let mut terminal = Terminal::new(TestBackend::new(size.width as u16, size.height as u16))?;
    let original_turn_id = app.active_turn_id.clone();
    app.active_turn_id = Some("perf-turn".to_string());

    for _ in 0..WORKING_DRAW_SAMPLES {
        let draw_started = Instant::now();
        terminal.draw(|frame| render_main_view(frame, app))?;
        stats.working_draw.push(draw_started.elapsed());
    }

    app.active_turn_id = original_turn_id;
    Ok(())
}

pub(super) fn benchmark_append_draws(
    app: &mut AppState,
    size: TerminalSize,
    stats: &mut PerfReplayStats,
) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(size.width as u16, size.height as u16))?;
    let idx = app.append_message(Role::Assistant, String::new());

    for _ in 0..APPEND_BENCH_STEPS {
        let started = Instant::now();
        app.messages[idx].text.push_str("\nappend bench line");
        app.mark_transcript_dirty_from(idx);
        app.ensure_rendered_lines(transcript_content_width(size), None);
        terminal.draw(|frame| render_main_view(frame, app))?;
        stats.append_total.push(started.elapsed());
    }

    Ok(())
}

pub(super) fn profile_layout_count_pass(app: &AppState, width: usize) -> Vec<LayoutBreakdownRow> {
    use std::collections::BTreeMap;

    if width == 0 {
        return Vec::new();
    }

    let mut previous_visible_idx = None;
    let mut buckets: BTreeMap<&'static str, (usize, usize, f64)> = BTreeMap::new();
    let mut count_cache = RenderCountCache::new();

    for (idx, msg) in app.messages.iter().enumerate() {
        if msg.text.trim().is_empty() {
            continue;
        }
        let previous_visible = previous_visible_idx.and_then(|prev_idx| app.messages.get(prev_idx));
        let started = Instant::now();
        let lines =
            count_rendered_block_for_message_cached(&mut count_cache, previous_visible, msg, width);
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        let entry = buckets
            .entry(layout_breakdown_label(msg))
            .or_insert((0usize, 0usize, 0.0f64));
        entry.0 += 1;
        entry.1 += lines;
        entry.2 += elapsed_ms;
        previous_visible_idx = Some(idx);
    }

    let mut rows: Vec<_> = buckets
        .into_iter()
        .map(|(label, (messages, lines, total_ms))| LayoutBreakdownRow {
            label,
            messages,
            lines,
            total_ms,
        })
        .collect();
    rows.sort_by(|a, b| b.total_ms.total_cmp(&a.total_ms));
    rows
}

fn layout_breakdown_label(msg: &Message) -> &'static str {
    match (msg.kind, msg.role) {
        (MessageKind::Diff, _) => "diff",
        (MessageKind::Plain, Role::Reasoning) => "reasoning_markdown",
        (MessageKind::Plain, Role::Assistant) => "assistant_markdown",
        (MessageKind::Plain, Role::ToolOutput) => "tool_output_ansi",
        (MessageKind::Plain, Role::Commentary) => "commentary_plain",
        (MessageKind::Plain, Role::ToolCall) => "tool_call_plain",
        (MessageKind::Plain, Role::User) => "user_plain",
        (MessageKind::Plain, Role::System) => "system_plain",
    }
}
