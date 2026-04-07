//! Offline performance benchmarking: session replay, JSONL parsing, and metric reporting.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde_json::Value;

use super::models::Role;
use super::perf_bench::{
    benchmark_append_draws, benchmark_scroll_draws, benchmark_typing_draws,
    benchmark_working_draws, profile_layout_count_pass, PerfReplayStats,
};
// Re-export synthetic types so tests can access them via the `perf_session` module path.
pub(super) use super::perf_synthetic::{build_synthetic_perf_messages, SyntheticPerfSpec};
use super::render::render_main_view;
use super::tools::{
    extract_diff_blocks, format_tool_item, raw_function_call_output_to_tool_item,
    raw_function_call_to_tool_item,
};
use super::transcript_render::transcript_content_width;
use super::{AppState, CliOptions, TerminalSize};

// --- Session Replay ---

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

// --- JSONL Parsing ---

fn append_perf_session_item(app: &mut AppState, payload: &Value) -> bool {
    let Some(kind) = payload.get("type").and_then(Value::as_str) else {
        return false;
    };
    match kind {
        "message" => append_perf_message(app, payload),
        "reasoning" => append_perf_reasoning(app, payload),
        "function_call" => append_perf_function_call(app, payload),
        "function_call_output" => append_perf_function_call_output(app, payload),
        _ => false,
    }
}

fn append_perf_message(app: &mut AppState, payload: &Value) -> bool {
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

fn append_perf_reasoning(app: &mut AppState, payload: &Value) -> bool {
    let Some(text) = reasoning_summary_text(payload) else {
        return false;
    };
    app.append_message(Role::Reasoning, text);
    true
}

fn append_perf_function_call(app: &mut AppState, payload: &Value) -> bool {
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

fn append_perf_function_call_output(app: &mut AppState, payload: &Value) -> bool {
    let Some((_, tool_item)) = raw_function_call_output_to_tool_item(payload) else {
        return false;
    };
    let diffs = extract_diff_blocks(&tool_item);
    if let Some(first) = diffs.first() {
        app.append_diff_message(Role::ToolOutput, first.file_path.clone(), first.diff.clone());
        for block in diffs.iter().skip(1) {
            app.append_diff_message(Role::ToolOutput, block.file_path.clone(), block.diff.clone());
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

// --- Utilities ---

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
