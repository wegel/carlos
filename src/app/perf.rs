//! Performance metrics collection for the event loop and rendering pipeline.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crossterm::event::KeyEventKind;

const PERF_SAMPLE_WINDOW: usize = 256;

#[derive(Debug, Default)]
pub(super) struct DurationSamples {
    pub(super) values_us: VecDeque<u32>,
    max_samples: usize,
    count: u64,
    total_us: u128,
    max_us: u32,
}

impl DurationSamples {
    pub(super) fn new(max_samples: usize) -> Self {
        Self {
            values_us: VecDeque::with_capacity(max_samples),
            max_samples,
            count: 0,
            total_us: 0,
            max_us: 0,
        }
    }

    pub(super) fn push(&mut self, duration: Duration) {
        let micros = duration.as_micros().min(u128::from(u32::MAX)) as u32;
        self.count = self.count.saturating_add(1);
        self.total_us = self.total_us.saturating_add(u128::from(micros));
        self.max_us = self.max_us.max(micros);
        self.values_us.push_back(micros);
        if self.values_us.len() > self.max_samples {
            self.values_us.pop_front();
        }
    }

    pub(super) fn percentile_us(&self, p: f64) -> Option<u32> {
        if self.values_us.is_empty() {
            return None;
        }
        let mut sorted: Vec<u32> = self.values_us.iter().copied().collect();
        sorted.sort_unstable();
        let len = sorted.len();
        let rank = ((len as f64 * p).ceil() as usize).saturating_sub(1);
        sorted.get(rank.min(len - 1)).copied()
    }

    fn p50_ms(&self) -> f64 {
        self.percentile_us(0.50).map_or(0.0, |v| v as f64 / 1000.0)
    }

    fn p95_ms(&self) -> f64 {
        self.percentile_us(0.95).map_or(0.0, |v| v as f64 / 1000.0)
    }

    pub(super) fn max_ms(&self) -> f64 {
        self.max_us as f64 / 1000.0
    }

    pub(super) fn avg_ms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            (self.total_us as f64 / self.count as f64) / 1000.0
        }
    }

    pub(super) fn summary(&self) -> String {
        format!(
            "p50 {:.2} p95 {:.2} avg {:.2} max {:.2} ms",
            self.p50_ms(),
            self.p95_ms(),
            self.avg_ms(),
            self.max_ms()
        )
    }
}

#[derive(Debug)]
pub(super) struct PerfMetrics {
    pub(super) show_overlay: bool,
    pub(super) loop_count: u64,
    pub(super) frame_count: u64,
    pub(super) notifications: u64,
    pub(super) key_events: u64,
    pub(super) key_press_events: u64,
    pub(super) key_repeat_events: u64,
    pub(super) key_release_events: u64,
    pub(super) mouse_events: u64,
    pub(super) paste_events: u64,
    pub(super) resize_events: u64,
    pending_key_at: Option<Instant>,
    last_key_at: Option<Instant>,
    last_repeat_at: Option<Instant>,
    pending_press_for_repeat: Option<Instant>,
    last_release_at: Option<Instant>,

    pub(super) poll_wait: DurationSamples,
    pub(super) event_handle: DurationSamples,
    pub(super) draw: DurationSamples,
    pub(super) transcript_render: DurationSamples,
    pub(super) input_layout: DurationSamples,
    pub(super) key_to_draw: DurationSamples,
    pub(super) key_interval: DurationSamples,
    pub(super) repeat_interval: DurationSamples,
    pub(super) press_to_first_repeat: DurationSamples,
    pub(super) release_to_next_key: DurationSamples,
}

impl PerfMetrics {
    pub(super) fn new() -> Self {
        Self {
            show_overlay: true,
            loop_count: 0,
            frame_count: 0,
            notifications: 0,
            key_events: 0,
            key_press_events: 0,
            key_repeat_events: 0,
            key_release_events: 0,
            mouse_events: 0,
            paste_events: 0,
            resize_events: 0,
            pending_key_at: None,
            last_key_at: None,
            last_repeat_at: None,
            pending_press_for_repeat: None,
            last_release_at: None,
            poll_wait: DurationSamples::new(PERF_SAMPLE_WINDOW),
            event_handle: DurationSamples::new(PERF_SAMPLE_WINDOW),
            draw: DurationSamples::new(PERF_SAMPLE_WINDOW),
            transcript_render: DurationSamples::new(PERF_SAMPLE_WINDOW),
            input_layout: DurationSamples::new(PERF_SAMPLE_WINDOW),
            key_to_draw: DurationSamples::new(PERF_SAMPLE_WINDOW),
            key_interval: DurationSamples::new(PERF_SAMPLE_WINDOW),
            repeat_interval: DurationSamples::new(PERF_SAMPLE_WINDOW),
            press_to_first_repeat: DurationSamples::new(PERF_SAMPLE_WINDOW),
            release_to_next_key: DurationSamples::new(PERF_SAMPLE_WINDOW),
        }
    }

    pub(super) fn record_draw(&mut self, duration: Duration) {
        self.frame_count = self.frame_count.saturating_add(1);
        self.draw.push(duration);
        if let Some(started) = self.pending_key_at.take() {
            self.key_to_draw.push(started.elapsed());
        }
    }

    pub(super) fn mark_key_event(&mut self) {
        self.key_events = self.key_events.saturating_add(1);
        let now = Instant::now();
        if let Some(last) = self.last_key_at.replace(now) {
            self.key_interval.push(last.elapsed());
        }
        if self.pending_key_at.is_none() {
            self.pending_key_at = Some(now);
        }
    }

    pub(super) fn mark_key_kind(&mut self, kind: KeyEventKind) {
        let now = Instant::now();
        match kind {
            KeyEventKind::Press => {
                self.key_press_events = self.key_press_events.saturating_add(1);
                if let Some(released_at) = self.last_release_at.take() {
                    self.release_to_next_key
                        .push(now.duration_since(released_at));
                }
                self.pending_press_for_repeat = Some(now);
                self.last_repeat_at = None;
            }
            KeyEventKind::Repeat => {
                self.key_repeat_events = self.key_repeat_events.saturating_add(1);
                if let Some(last_repeat) = self.last_repeat_at.replace(now) {
                    self.repeat_interval.push(now.duration_since(last_repeat));
                }
                if let Some(pressed_at) = self.pending_press_for_repeat.take() {
                    self.press_to_first_repeat
                        .push(now.duration_since(pressed_at));
                }
            }
            KeyEventKind::Release => {
                self.key_release_events = self.key_release_events.saturating_add(1);
                self.last_release_at = Some(now);
                self.pending_press_for_repeat = None;
                self.last_repeat_at = None;
            }
        }
    }

    pub(super) fn toggle_overlay(&mut self) {
        self.show_overlay = !self.show_overlay;
    }

    pub(super) fn overlay_lines(&self) -> Vec<String> {
        vec![
            format!("loop {}  frame {}", self.loop_count, self.frame_count),
            format!(
                "evt key {} (p{} r{} u{}) mouse {} paste {} resize {} notif {}",
                self.key_events,
                self.key_press_events,
                self.key_repeat_events,
                self.key_release_events,
                self.mouse_events,
                self.paste_events,
                self.resize_events,
                self.notifications
            ),
            format!("draw          {}", self.draw.summary()),
            format!("poll wait     {}", self.poll_wait.summary()),
            format!("event handle  {}", self.event_handle.summary()),
            format!("text render   {}", self.transcript_render.summary()),
            format!("input layout  {}", self.input_layout.summary()),
            format!("key -> draw   {}", self.key_to_draw.summary()),
            format!("key interval  {}", self.key_interval.summary()),
            format!("repeat intvl  {}", self.repeat_interval.summary()),
            format!("press->repeat {}", self.press_to_first_repeat.summary()),
            format!("release->key  {}", self.release_to_next_key.summary()),
        ]
    }

    pub(super) fn final_report(&self) -> String {
        let mut out = String::new();
        out.push_str("carlos perf metrics\n");
        out.push_str(&format!(
            "counts: loop={} frame={} key={} key_press={} key_repeat={} key_release={} mouse={} paste={} resize={} notifications={}\n",
            self.loop_count,
            self.frame_count,
            self.key_events,
            self.key_press_events,
            self.key_repeat_events,
            self.key_release_events,
            self.mouse_events,
            self.paste_events,
            self.resize_events,
            self.notifications
        ));
        out.push_str(&format!("draw:         {}\n", self.draw.summary()));
        out.push_str(&format!("poll_wait:    {}\n", self.poll_wait.summary()));
        out.push_str(&format!("event_handle: {}\n", self.event_handle.summary()));
        out.push_str(&format!(
            "text_render:  {}\n",
            self.transcript_render.summary()
        ));
        out.push_str(&format!("input_layout: {}\n", self.input_layout.summary()));
        out.push_str(&format!("key_to_draw:  {}\n", self.key_to_draw.summary()));
        out.push_str(&format!("key_interval: {}\n", self.key_interval.summary()));
        out.push_str(&format!(
            "repeat_interval: {}\n",
            self.repeat_interval.summary()
        ));
        out.push_str(&format!(
            "press_to_first_repeat: {}\n",
            self.press_to_first_repeat.summary()
        ));
        out.push_str(&format!(
            "release_to_next_key: {}",
            self.release_to_next_key.summary()
        ));
        out
    }
}
