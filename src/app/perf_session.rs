use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde_json::Value;

use super::models::{Message, MessageKind, Role};
use super::perf::DurationSamples;
use super::render::{
    compute_input_layout, count_rendered_block_for_message, format_read_summary_with_count,
    render_main_view, transcript_content_width,
};
use super::tools::{
    extract_diff_blocks, format_tool_item, raw_function_call_output_to_tool_item,
    raw_function_call_to_tool_item,
};
use super::{AppState, CliOptions, TerminalSize, MSG_TOP};

const PERF_SAMPLE_WINDOW: usize = 4096;
const APPEND_BENCH_STEPS: usize = 16;
const MAX_SCROLL_SAMPLES: usize = 64;
const TYPING_BENCH_TEXT: &str = "the quick brown fox jumps over the lazy dog";
const SYNTHETIC_USER: &str = "perf-user";
const SYNTHETIC_HOST: &str = "perfbox.local";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SyntheticPerfSpec {
    pub(super) seed: u64,
    pub(super) turns: usize,
    pub(super) tool_output_lines: usize,
}

struct PerfReplayStats {
    replay_apply: DurationSamples,
    full_layout_ms: f64,
    full_draw_ms: f64,
    scroll_draw: DurationSamples,
    typing_draw: DurationSamples,
    working_draw: DurationSamples,
    append_total: DurationSamples,
    relevant_items: usize,
    layout_breakdown: Vec<LayoutBreakdownRow>,
}

impl PerfReplayStats {
    fn new() -> Self {
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
struct LayoutBreakdownRow {
    label: &'static str,
    messages: usize,
    lines: usize,
    total_ms: f64,
}

pub(super) fn run_perf_session(opts: &CliOptions) -> Result<()> {
    let terminal_size = TerminalSize {
        width: opts.perf_width,
        height: opts.perf_height,
    };
    let source_label = if opts.perf_synthetic {
        format!(
            "synthetic seed={} turns={} tool_lines={}",
            opts.perf_seed, opts.perf_turns, opts.perf_tool_lines
        )
    } else {
        opts.perf_session_path
            .clone()
            .context("missing session path")?
    };

    let rss_before_kib = current_rss_kib();
    let start = Instant::now();
    let (mut app, mut stats) = if opts.perf_synthetic {
        replay_synthetic_perf_session(
            SyntheticPerfSpec {
                seed: opts.perf_seed,
                turns: opts.perf_turns,
                tool_output_lines: opts.perf_tool_lines,
            },
            terminal_size,
        )?
    } else {
        replay_perf_session(
            opts.perf_session_path
                .as_deref()
                .context("missing session path")?,
            terminal_size,
        )?
    };
    let replay_elapsed = start.elapsed();
    let rss_after_replay_kib = current_rss_kib();

    benchmark_scroll_draws(&mut app, terminal_size, &mut stats)?;
    benchmark_typing_draws(&mut app, terminal_size, &mut stats)?;
    benchmark_working_draws(&mut app, terminal_size, &mut stats)?;
    benchmark_append_draws(&mut app, terminal_size, &mut stats)?;
    let rss_after_bench_kib = current_rss_kib();

    println!("carlos perf-session");
    println!("source: {source_label}");
    println!("viewport: {}x{}", terminal_size.width, terminal_size.height);
    println!(
        "transcript: messages={} rendered_lines={} relevant_items={} replay_elapsed_ms={:.2}",
        app.messages.len(),
        app.rendered_line_count(),
        stats.relevant_items,
        replay_elapsed.as_secs_f64() * 1000.0
    );
    println!(
        "memory_kib: before={} after_replay={} after_bench={}",
        rss_before_kib.unwrap_or(0),
        rss_after_replay_kib.unwrap_or(0),
        rss_after_bench_kib.unwrap_or(0)
    );
    println!("replay_apply:  {}", stats.replay_apply.summary());
    println!("full_layout:   {:.2} ms", stats.full_layout_ms);
    println!("full_draw:     {:.2} ms", stats.full_draw_ms);
    println!("scroll_draw:   {}", stats.scroll_draw.summary());
    println!("typing_draw:   {}", stats.typing_draw.summary());
    println!("working_draw:  {}", stats.working_draw.summary());
    println!("append_total:  {}", stats.append_total.summary());
    if !stats.layout_breakdown.is_empty() {
        println!("layout_breakdown:");
        for row in &stats.layout_breakdown {
            println!(
                "  {} msgs={} lines={} total_ms={:.2}",
                row.label, row.messages, row.lines, row.total_ms
            );
        }
    }

    Ok(())
}

fn replay_perf_session(path: &str, size: TerminalSize) -> Result<(AppState, PerfReplayStats)> {
    let file = File::open(path).with_context(|| format!("open {path}"))?;
    let reader = BufReader::new(file);
    let thread_id = thread_id_from_session_path(path);
    let mut app = AppState::new(thread_id);
    let mut stats = PerfReplayStats::new();

    for line in reader.lines() {
        let line = line?;
        let value: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if value.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };

        let apply_started = Instant::now();
        if !append_perf_session_item(&mut app, payload) {
            continue;
        }
        stats.replay_apply.push(apply_started.elapsed());
        stats.relevant_items = stats.relevant_items.saturating_add(1);
    }

    let mut terminal = Terminal::new(TestBackend::new(size.width as u16, size.height as u16))?;
    stats.layout_breakdown = profile_layout_count_pass(&app, transcript_content_width(size));
    let render_started = Instant::now();
    app.ensure_rendered_lines(transcript_content_width(size), None);
    stats.full_layout_ms = render_started.elapsed().as_secs_f64() * 1000.0;

    let draw_started = Instant::now();
    terminal.draw(|frame| render_main_view(frame, &mut app))?;
    stats.full_draw_ms = draw_started.elapsed().as_secs_f64() * 1000.0;

    Ok((app, stats))
}

fn replay_synthetic_perf_session(
    spec: SyntheticPerfSpec,
    size: TerminalSize,
) -> Result<(AppState, PerfReplayStats)> {
    let mut app = AppState::new(format!("synthetic-{:016x}", spec.seed));
    let mut stats = PerfReplayStats::new();
    let apply_started = Instant::now();
    app.messages = build_synthetic_perf_messages(spec);
    app.mark_transcript_dirty();
    stats.replay_apply.push(apply_started.elapsed());
    stats.relevant_items = app.messages.len();

    let mut terminal = Terminal::new(TestBackend::new(size.width as u16, size.height as u16))?;
    stats.layout_breakdown = profile_layout_count_pass(&app, transcript_content_width(size));
    let render_started = Instant::now();
    app.ensure_rendered_lines(transcript_content_width(size), None);
    stats.full_layout_ms = render_started.elapsed().as_secs_f64() * 1000.0;

    let draw_started = Instant::now();
    terminal.draw(|frame| render_main_view(frame, &mut app))?;
    stats.full_draw_ms = draw_started.elapsed().as_secs_f64() * 1000.0;

    Ok((app, stats))
}

pub(super) fn build_synthetic_perf_messages(spec: SyntheticPerfSpec) -> Vec<Message> {
    let mut rng = SyntheticRng::new(spec.seed);
    let mut messages = Vec::with_capacity(spec.turns.saturating_mul(6));

    for turn in 0..spec.turns {
        messages.push(Message {
            role: Role::User,
            text: synthetic_user_message(turn, &mut rng),
            kind: MessageKind::Plain,
            file_path: None,
        });
        messages.push(Message {
            role: Role::Commentary,
            text: synthetic_commentary_message(turn, &mut rng),
            kind: MessageKind::Plain,
            file_path: None,
        });
        messages.push(Message {
            role: Role::Reasoning,
            text: synthetic_reasoning_message(turn, &mut rng),
            kind: MessageKind::Plain,
            file_path: None,
        });
        messages.push(Message {
            role: Role::ToolCall,
            text: synthetic_tool_call(turn, &mut rng),
            kind: MessageKind::Plain,
            file_path: None,
        });
        messages.push(synthetic_tool_output(turn, spec, &mut rng));
        messages.push(Message {
            role: Role::Assistant,
            text: synthetic_assistant_message(turn, &mut rng),
            kind: MessageKind::Plain,
            file_path: None,
        });
    }

    messages
}

fn append_perf_session_item(app: &mut AppState, payload: &Value) -> bool {
    let Some(kind) = payload.get("type").and_then(Value::as_str) else {
        return false;
    };

    match kind {
        "message" => {
            let role = match payload.get("role").and_then(Value::as_str) {
                Some("user") => Role::User,
                Some("assistant") => {
                    if payload.get("phase").and_then(Value::as_str) == Some("commentary") {
                        Role::Commentary
                    } else {
                        Role::Assistant
                    }
                }
                _ => return false,
            };
            let text = payload
                .get("text")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| session_message_text(payload));
            if let Some(text) = text.filter(|text| !text.trim().is_empty()) {
                app.append_message(role, text);
                return true;
            }
            false
        }
        "reasoning" => {
            let Some(text) = reasoning_summary_text(payload) else {
                return false;
            };
            app.append_message(Role::Reasoning, text);
            true
        }
        "function_call" => {
            let Some((_, tool_item)) = raw_function_call_to_tool_item(payload) else {
                return false;
            };
            let Some(text) = format_tool_item(&tool_item, Role::ToolCall) else {
                return false;
            };
            if text.trim().is_empty() {
                return false;
            }
            app.append_message(Role::ToolCall, text);
            true
        }
        "function_call_output" => {
            let Some((_, tool_item)) = raw_function_call_output_to_tool_item(payload) else {
                return false;
            };
            let diffs = extract_diff_blocks(&tool_item);
            if let Some(first) = diffs.first() {
                app.append_diff_message(
                    Role::ToolOutput,
                    first.file_path.clone(),
                    first.diff.clone(),
                );
                for block in diffs.iter().skip(1) {
                    app.append_diff_message(
                        Role::ToolOutput,
                        block.file_path.clone(),
                        block.diff.clone(),
                    );
                }
                return true;
            }
            let Some(text) = format_tool_item(&tool_item, Role::ToolOutput) else {
                return false;
            };
            if text.trim().is_empty() {
                return false;
            }
            app.append_message(Role::ToolOutput, text);
            true
        }
        _ => false,
    }
}

fn benchmark_scroll_draws(
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
    let old_scroll = app.scroll_top;
    let old_follow = app.auto_follow_bottom;

    let mut pos = 0usize;
    while pos <= max_scroll {
        app.scroll_top = pos;
        app.auto_follow_bottom = pos >= max_scroll;
        let draw_started = Instant::now();
        terminal.draw(|frame| render_main_view(frame, app))?;
        stats.scroll_draw.push(draw_started.elapsed());
        if pos == max_scroll {
            break;
        }
        pos = (pos + step).min(max_scroll);
    }

    app.scroll_top = old_scroll.min(max_scroll);
    app.auto_follow_bottom = old_follow;
    Ok(())
}

fn benchmark_typing_draws(
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

fn benchmark_working_draws(
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

fn benchmark_append_draws(
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

fn profile_layout_count_pass(app: &AppState, width: usize) -> Vec<LayoutBreakdownRow> {
    use std::collections::BTreeMap;

    if width == 0 {
        return Vec::new();
    }

    let mut previous_visible_idx = None;
    let mut buckets: BTreeMap<&'static str, (usize, usize, f64)> = BTreeMap::new();

    for (idx, msg) in app.messages.iter().enumerate() {
        if msg.text.trim().is_empty() {
            continue;
        }
        let previous_visible = previous_visible_idx.and_then(|prev_idx| app.messages.get(prev_idx));
        let started = Instant::now();
        let lines = count_rendered_block_for_message(previous_visible, msg, width);
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

fn reasoning_summary_text(item: &Value) -> Option<String> {
    let summary = item.get("summary")?.as_array()?;
    let mut parts = Vec::new();
    for entry in summary {
        if let Some(text) = entry.as_str() {
            if !text.trim().is_empty() {
                parts.push(text.to_string());
            }
            continue;
        }

        let text = entry.get("text").and_then(Value::as_str).or_else(|| {
            entry
                .get("content")
                .and_then(Value::as_array)
                .and_then(|parts| parts.first())
                .and_then(|part| part.get("text"))
                .and_then(Value::as_str)
        });
        if let Some(text) = text.filter(|t| !t.trim().is_empty()) {
            parts.push(text.to_string());
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn session_message_text(item: &Value) -> Option<String> {
    let content = item.get("content").and_then(Value::as_array)?;
    let mut parts = Vec::new();
    for part in content {
        let Some(kind) = part.get("type").and_then(Value::as_str) else {
            continue;
        };
        if !matches!(kind, "text" | "output_text" | "input_text") {
            continue;
        }
        if let Some(text) = part.get("text").and_then(Value::as_str) {
            if !text.is_empty() {
                parts.push(text.to_string());
            }
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn current_rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        let rest = line.strip_prefix("VmRSS:")?;
        let value = rest.split_whitespace().next()?;
        if let Ok(parsed) = value.parse::<u64>() {
            return Some(parsed);
        }
    }
    None
}

fn thread_id_from_session_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.rsplit('-').next())
        .unwrap_or("perf-session")
        .to_string()
}

#[derive(Debug, Clone, Copy)]
struct SyntheticRng(u64);

impl SyntheticRng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 1 } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn range(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            0
        } else {
            (self.next_u64() as usize) % upper
        }
    }

    fn choose<'a>(&mut self, items: &'a [&'static str]) -> &'a str {
        items[self.range(items.len())]
    }
}

fn synthetic_user_message(turn: usize, rng: &mut SyntheticRng) -> String {
    let intro = synthetic_paragraph(rng, 2, 9, 14);
    if turn % 7 == 0 {
        format!(
            "{intro}\n\n```rust\n{}\n```",
            synthetic_code_block(turn, rng)
        )
    } else {
        intro
    }
}

fn synthetic_commentary_message(turn: usize, rng: &mut SyntheticRng) -> String {
    format!(
        "{}\n{}",
        synthetic_heading(rng),
        synthetic_paragraph(rng, 1 + (turn % 2), 10, 16)
    )
}

fn synthetic_reasoning_message(turn: usize, rng: &mut SyntheticRng) -> String {
    let count = 2 + (turn % 4);
    let mut lines = Vec::with_capacity(count);
    for _ in 0..count {
        lines.push(format!("**{}**", synthetic_heading(rng)));
    }
    lines.join("\n")
}

fn synthetic_tool_call(turn: usize, rng: &mut SyntheticRng) -> String {
    match turn % 4 {
        0 | 1 => {
            let path = format!(
                "src/module_{:02}/file_{:03}.rs",
                (turn / 6) % 48,
                (turn / 2) % 192
            );
            format_read_summary_with_count(&path, 1)
        }
        2 => format!(
            "remote exec on {}@{}\n$ {}",
            SYNTHETIC_USER,
            SYNTHETIC_HOST,
            synthetic_shell_pipeline(turn, rng)
        ),
        _ => format!(
            "run `cargo test -p crate_{:02} {} -- --nocapture`",
            turn % 19,
            synthetic_identifier(rng)
        ),
    }
}

fn synthetic_tool_output(turn: usize, spec: SyntheticPerfSpec, rng: &mut SyntheticRng) -> Message {
    if turn % 9 == 0 {
        return Message {
            role: Role::ToolOutput,
            kind: MessageKind::Diff,
            file_path: Some(format!(
                "src/module_{:02}/perf_case_{:03}.rs",
                turn % 32,
                turn
            )),
            text: synthetic_diff(turn, rng),
        };
    }

    let mut lines = Vec::with_capacity(spec.tool_output_lines.max(1));
    for idx in 0..spec.tool_output_lines.max(1) {
        let level = match idx % 3 {
            0 => "\u{1b}[32mINFO\u{1b}[0m",
            1 => "\u{1b}[33mWARN\u{1b}[0m",
            _ => "\u{1b}[36mDEBUG\u{1b}[0m",
        };
        lines.push(format!(
            "{level} turn={turn:05} line={idx:03} {} {} {}",
            synthetic_identifier(rng),
            synthetic_heading(rng).to_ascii_lowercase(),
            synthetic_paragraph(rng, 1, 8, 12)
        ));
    }

    Message {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        file_path: None,
        text: lines.join("\n"),
    }
}

fn synthetic_assistant_message(turn: usize, rng: &mut SyntheticRng) -> String {
    let mut sections = vec![synthetic_paragraph(rng, 2, 10, 16)];
    if turn % 5 == 0 {
        sections.push(format!(
            "- {}\n- {}\n- {}",
            synthetic_sentence(rng, 8, 12),
            synthetic_sentence(rng, 8, 12),
            synthetic_sentence(rng, 8, 12)
        ));
    }
    sections.join("\n\n")
}

fn synthetic_code_block(turn: usize, rng: &mut SyntheticRng) -> String {
    let value = 10 + rng.range(90);
    format!(
        "fn case_{turn:04}() {{\n    let {} = {value};\n    println!(\"{{}}\", {});\n}}",
        synthetic_identifier(rng),
        synthetic_identifier(rng)
    )
}

fn synthetic_diff(turn: usize, rng: &mut SyntheticRng) -> String {
    format!(
        "@@ -{old_line},3 +{new_line},5 @@\n-{}\n-{}\n+{}\n+{}\n+{}",
        synthetic_sentence(rng, 6, 9),
        synthetic_sentence(rng, 6, 9),
        synthetic_sentence(rng, 7, 11),
        synthetic_sentence(rng, 7, 11),
        synthetic_sentence(rng, 7, 11),
        old_line = 10 + (turn % 70),
        new_line = 10 + (turn % 70)
    )
}

fn synthetic_shell_pipeline(turn: usize, rng: &mut SyntheticRng) -> String {
    format!(
        "set -euo pipefail; jq -r '.items[] | .name' {} | sed -n '1,40p' | awk 'NF'",
        synthetic_remote_path(turn, rng)
    )
}

fn synthetic_remote_path(turn: usize, rng: &mut SyntheticRng) -> String {
    format!(
        "/srv/{}/jobs/{:04}/{}.json",
        synthetic_identifier(rng),
        turn % 10_000,
        synthetic_identifier(rng)
    )
}

fn synthetic_heading(rng: &mut SyntheticRng) -> String {
    let mut words = vec![
        rng.choose(&HEADING_VERBS).to_string(),
        rng.choose(&HEADING_OBJECTS).to_string(),
        rng.choose(&HEADING_OBJECTS).to_string(),
    ];
    for word in &mut words {
        if let Some(first) = word.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        break;
    }
    words.join(" ")
}

fn synthetic_paragraph(
    rng: &mut SyntheticRng,
    sentences: usize,
    min_words: usize,
    max_words: usize,
) -> String {
    (0..sentences)
        .map(|_| synthetic_sentence(rng, min_words, max_words))
        .collect::<Vec<_>>()
        .join(" ")
}

fn synthetic_sentence(rng: &mut SyntheticRng, min_words: usize, max_words: usize) -> String {
    let span = max_words.saturating_sub(min_words);
    let words = min_words + rng.range(span.max(1) + 1);
    let mut out = String::new();
    for idx in 0..words {
        let word = if idx == 0 {
            rng.choose(&SYNTHETIC_WORDS).to_ascii_uppercase()
        } else {
            rng.choose(&SYNTHETIC_WORDS).to_string()
        };
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&word);
        if idx + 1 == words / 2 && words > 10 && rng.range(3) == 0 {
            out.push(',');
        }
    }
    out.push('.');
    out
}

fn synthetic_identifier(rng: &mut SyntheticRng) -> String {
    format!("{}_{}", rng.choose(&SYNTHETIC_WORDS), 10 + rng.range(990))
}

const SYNTHETIC_WORDS: [&str; 40] = [
    "atlas", "buffer", "cache", "delta", "engine", "frame", "graph", "handle", "index", "journal",
    "kernel", "layout", "marker", "native", "offset", "packet", "queue", "render", "sample",
    "token", "update", "vector", "window", "yield", "bridge", "cursor", "driver", "event",
    "format", "guard", "history", "input", "ledger", "metric", "notice", "output", "parser",
    "reader", "signal", "worker",
];

const HEADING_VERBS: [&str; 12] = [
    "analyzing",
    "checking",
    "profiling",
    "reviewing",
    "comparing",
    "tracing",
    "stabilizing",
    "measuring",
    "inspecting",
    "sampling",
    "tracking",
    "validating",
];

const HEADING_OBJECTS: [&str; 16] = [
    "viewport",
    "render",
    "history",
    "selection",
    "input",
    "layout",
    "scroll",
    "diff",
    "session",
    "cursor",
    "repaint",
    "metrics",
    "tooling",
    "state",
    "wrapping",
    "output",
];
