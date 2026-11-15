//! Measure real daemon input, terminal events and existing renderer phases without a second collector.

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use carlos_client::codec::Incoming;
use crossterm::event::{self, Event, KeyEvent};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::{Frame, Terminal};

use super::transport::Transport;
use super::Device;
use crate::app::notifications::{is_key_press_like, is_perf_toggle_key};
use crate::app::overlay_render::draw_perf_overlay;
use crate::app::perf::PerfMetrics;
use crate::app::render_input::{compute_composer_layout, InputLayout};
use crate::app::view_models::TerminalSize;

impl Device {
    /// Keep the existing diagnostics above help while settings and questions retain their layers.
    pub(super) fn metrics_help(&mut self, frame: &mut Frame<'_>) -> bool {
        if !self.draw_help(frame) {
            return false;
        }
        let size = TerminalSize {
            width: frame.area().width as usize,
            height: frame.area().height as usize,
        };
        self.draw_metrics(frame.buffer_mut(), size);
        true
    }

    /// Honor the existing CARLOS_METRICS opt-in without allocating a collector when disabled.
    pub(super) fn configure_metrics(&mut self) {
        if crate::app::cli::env_flag_enabled("CARLOS_METRICS") {
            self.view.perf = Some(PerfMetrics::new());
        }
    }

    /// Consume the existing local toggle keys before history or editor shortcuts.
    pub(super) fn metrics_key(&mut self, key: KeyEvent) -> bool {
        if !is_perf_toggle_key(key.code, key.modifiers) {
            return false;
        }
        if let Some(perf) = self.view.perf.as_mut() {
            perf.toggle_overlay();
        }
        true
    }

    /// Count one actual device loop iteration.
    pub(super) fn metrics_loop(&mut self) {
        if let Some(perf) = self.view.perf.as_mut() {
            perf.loop_count = perf.loop_count.saturating_add(1);
        }
    }

    /// Measure the completed terminal draw and its pending key latency.
    pub(super) fn metrics_draw(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let started = self.metrics_started();
        terminal.draw(|frame| self.draw(frame))?;
        if let (Some(started), Some(perf)) = (started, self.view.perf.as_mut()) {
            perf.record_draw(started.elapsed());
        }
        Ok(())
    }

    /// Measure the existing eight-millisecond terminal poll without adding a timer.
    pub(super) fn metrics_poll(&mut self) -> Result<bool> {
        let started = self.metrics_started();
        let result = event::poll(Duration::from_millis(8));
        if let (Some(started), Some(perf)) = (started, self.view.perf.as_mut()) {
            perf.poll_wait.push(started.elapsed());
        }
        result.map_err(Into::into)
    }

    /// Count and time one terminal event without retaining its private contents.
    pub(super) fn metrics_input(
        &mut self,
        socket: &Path,
        transport: &mut Option<Transport>,
        input: Event,
    ) -> Result<bool> {
        if let Some(perf) = self.view.perf.as_mut() {
            record_event(perf, &input);
        }
        let started = self.metrics_started();
        let result = self.event_with_reconnect(socket, transport, input);
        self.metrics_handled(started);
        result
    }

    /// Time real client fact/reply handling and count actual daemon notifications.
    pub(super) fn metrics_receive(
        &mut self,
        link: &mut Transport,
        incoming: Incoming,
    ) -> Result<bool> {
        if matches!(&incoming, Incoming::Notification(_)) {
            if let Some(perf) = self.view.perf.as_mut() {
                perf.notifications = perf.notifications.saturating_add(1);
            }
        }
        let started = self.metrics_started();
        let result = self.receive(link, incoming);
        self.metrics_handled(started);
        result
    }

    fn metrics_handled(&mut self, started: Option<Instant>) {
        if let (Some(started), Some(perf)) = (started, self.view.perf.as_mut()) {
            perf.event_handle.push(started.elapsed());
        }
    }

    /// Measure the shared composer layout rather than reporting an unused sample.
    pub(super) fn metrics_layout(&mut self, size: TerminalSize, x: usize) -> InputLayout {
        let started = self.metrics_started();
        let layout = compute_composer_layout(&self.view.input, size, x);
        if let (Some(started), Some(perf)) = (started, self.view.perf.as_mut()) {
            perf.input_layout.push(started.elapsed());
        }
        layout
    }

    /// Measure terminal-side cached row preparation and painting, excluding formatter-worker CPU.
    pub(super) fn metrics_transcript(
        &mut self,
        buffer: &mut Buffer,
        size: TerminalSize,
        layout: &InputLayout,
        x: usize,
    ) {
        let started = self.metrics_started();
        self.draw_transcript(buffer, size, layout, x);
        if let (Some(started), Some(perf)) = (started, self.view.perf.as_mut()) {
            perf.transcript_render.push(started.elapsed());
        }
    }

    /// Reuse the existing performance widget at its existing size threshold.
    pub(super) fn draw_metrics(&self, buffer: &mut Buffer, size: TerminalSize) {
        if let Some(perf) = self.view.perf.as_ref().filter(|perf| perf.show_overlay) {
            draw_perf_overlay(buffer, size, perf);
        }
    }

    /// Capture the existing report before startup restores and releases the terminal.
    pub(super) fn metrics_report(&self) -> Option<String> {
        self.view.perf.as_ref().map(PerfMetrics::final_report)
    }

    fn metrics_started(&self) -> Option<Instant> {
        self.view.perf.as_ref().map(|_| Instant::now())
    }
}

fn record_event(perf: &mut PerfMetrics, event: &Event) {
    match event {
        Event::Key(key) => {
            perf.mark_key_kind(key.kind);
            if is_key_press_like(key.kind) {
                perf.mark_key_event();
            }
        }
        Event::Mouse(_) => perf.mouse_events = perf.mouse_events.saturating_add(1),
        Event::Paste(_) => perf.paste_events = perf.paste_events.saturating_add(1),
        Event::Resize(_, _) => perf.resize_events = perf.resize_events.saturating_add(1),
        _ => {}
    }
}
