//! Preserve metric windows, key-repeat buckets and the local status sweep without a session.

use std::time::Duration;

use crossterm::event::KeyEventKind;

use super::notifications::kitt_head_index;
use super::perf::{DurationSamples, PerfMetrics};

#[test]
fn kitt_head_index_bounces_across_separator() {
    let seq: Vec<usize> = (0..9).map(|tick| kitt_head_index(5, tick)).collect();
    assert_eq!(seq, vec![0, 1, 2, 3, 4, 3, 2, 1, 0]);
    assert_eq!(kitt_head_index(1, 42), 0);
}

#[test]

fn duration_samples_tracks_percentiles_and_window() {
    let mut samples = DurationSamples::new(3);
    samples.push(Duration::from_micros(1_000));
    samples.push(Duration::from_micros(2_000));
    samples.push(Duration::from_micros(3_000));
    samples.push(Duration::from_micros(4_000));

    // Window size is 3, so first sample is dropped.
    assert_eq!(samples.values_us.len(), 3);
    assert_eq!(samples.percentile_us(0.50), Some(3_000));
    assert_eq!(samples.percentile_us(0.95), Some(4_000));
    assert_eq!(samples.avg_ms(), 2.50);
    assert_eq!(samples.max_ms(), 4.0);
}

#[test]
fn perf_metrics_overlay_lines_include_latency_rows() {
    let mut perf = PerfMetrics::new();
    perf.poll_wait.push(Duration::from_micros(500));
    perf.event_handle.push(Duration::from_micros(700));
    perf.draw.push(Duration::from_micros(900));
    perf.key_interval.push(Duration::from_micros(1_100));
    perf.repeat_interval.push(Duration::from_micros(1_300));
    perf.press_to_first_repeat
        .push(Duration::from_micros(1_500));
    perf.release_to_next_key.push(Duration::from_micros(1_700));

    let lines = perf.overlay_lines();
    assert!(lines.iter().any(|line| line.contains("poll wait")));
    assert!(lines.iter().any(|line| line.contains("event handle")));
    assert!(lines.iter().any(|line| line.contains("draw")));
    assert!(lines.iter().any(|line| line.contains("key interval")));
    assert!(lines.iter().any(|line| line.contains("repeat intvl")));
    assert!(lines.iter().any(|line| line.contains("press->repeat")));
    assert!(lines.iter().any(|line| line.contains("release->key")));
}

#[test]
fn perf_metrics_tracks_repeat_transition_buckets() {
    let mut perf = PerfMetrics::new();
    perf.mark_key_kind(KeyEventKind::Press);
    perf.mark_key_kind(KeyEventKind::Repeat);
    perf.mark_key_kind(KeyEventKind::Repeat);
    perf.mark_key_kind(KeyEventKind::Release);
    perf.mark_key_kind(KeyEventKind::Press);

    assert_eq!(perf.key_press_events, 2);
    assert_eq!(perf.key_repeat_events, 2);
    assert_eq!(perf.key_release_events, 1);
    assert_eq!(perf.press_to_first_repeat.values_us.len(), 1);
    assert_eq!(perf.repeat_interval.values_us.len(), 1);
    assert_eq!(perf.release_to_next_key.values_us.len(), 1);
}
