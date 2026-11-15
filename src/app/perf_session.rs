//! Offline performance benchmarking: session replay, JSONL parsing, and metric reporting.

use std::fs::File;
use std::io::BufReader;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use carlos_core::perf_fixture::{replay_perf_rows, synthetic_perf_rows};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::cli::CliOptions;
use super::perf_bench::{
    benchmark_append_draws, benchmark_scroll_draws, benchmark_typing_draws,
    benchmark_working_draws, profile_layout_count_pass, PerfReplayStats,
};
use super::perf_view::PerfView;
use super::TerminalSize;

// Re-export synthetic types so tests can access them via the `perf_session` module path.
pub(super) use carlos_core::perf_fixture::SyntheticPerfSpec;

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

    print_perf_report(
        &app,
        &stats,
        &source_label,
        terminal_size,
        replay_elapsed,
        ReplayMemory {
            before: rss_before_kib,
            after_replay: rss_after_replay_kib,
            after_bench: rss_after_bench_kib,
        },
    );
    Ok(())
}

struct ReplayMemory {
    before: Option<u64>,
    after_replay: Option<u64>,
    after_bench: Option<u64>,
}

fn print_perf_report(
    app: &PerfView,
    stats: &PerfReplayStats,
    source_label: &str,
    size: TerminalSize,
    replay_elapsed: Duration,
    memory: ReplayMemory,
) {
    println!("carlos perf-session");
    println!("source: {source_label}");
    println!("viewport: {}x{}", size.width, size.height);
    println!(
        "transcript: messages={} rendered_lines={} relevant_items={} replay_elapsed_ms={:.2}",
        app.rows.len(),
        app.rendered_line_count(),
        stats.relevant_items,
        replay_elapsed.as_secs_f64() * 1000.0
    );
    println!(
        "memory_kib: before={} after_replay={} after_bench={}",
        memory.before.unwrap_or(0),
        memory.after_replay.unwrap_or(0),
        memory.after_bench.unwrap_or(0)
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
}

fn replay_perf_session(path: &str, size: TerminalSize) -> Result<(PerfView, PerfReplayStats)> {
    let file = File::open(path).with_context(|| format!("open {path}"))?;
    let mut stats = PerfReplayStats::new();
    let data = replay_perf_rows(BufReader::new(file), |duration| {
        stats.replay_apply.push(duration)
    })?;
    stats.relevant_items = data.relevant_items;
    prepare_view(PerfView::new(data), stats, size)
}

fn replay_synthetic_perf_session(
    spec: SyntheticPerfSpec,
    size: TerminalSize,
) -> Result<(PerfView, PerfReplayStats)> {
    let mut stats = PerfReplayStats::new();
    let data = synthetic_perf_rows(spec, |duration| stats.replay_apply.push(duration));
    stats.relevant_items = data.relevant_items;
    prepare_view(PerfView::new(data), stats, size)
}

fn prepare_view(
    mut app: PerfView,
    mut stats: PerfReplayStats,
    size: TerminalSize,
) -> Result<(PerfView, PerfReplayStats)> {
    let mut terminal = Terminal::new(TestBackend::new(size.width as u16, size.height as u16))?;
    app.view.tool_output.update_size(size);
    app.view.layout.update_size(size);
    stats.layout_breakdown = profile_layout_count_pass(&app, app.transcript_width(size));
    let started = Instant::now();
    app.ensure_rendered_lines(app.transcript_width(size));
    stats.full_layout_ms = started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    terminal.draw(|frame| app.draw(frame))?;
    stats.full_draw_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok((app, stats))
}

// --- Process memory ---

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
