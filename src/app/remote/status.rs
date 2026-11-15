//! Paint the shared footer from accepted facts and advance only this device's display clocks.

use std::time::{Duration, Instant};

use carlos_core::session::context_usage::ContextUsage;
use carlos_core::session::text::safe_label;
use carlos_core::session::usage::{unix_now, RateWindow, WEEK_MINUTES};
use carlos_proto::session::{Availability, Backend, SessionInfo};
use carlos_proto::task_overview::TaskMainState;
use ratatui::buffer::Buffer;
use ratatui::style::Style;

use super::ralph_keys::ralph_badges;
use super::Device;
use crate::app::notifications::animation_tick;
use crate::app::render::{draw_composer, draw_status_separator, draw_str};
use crate::app::render_input::InputLayout;
use crate::app::status_line::{
    draw_labels, fit_model, place, place_context, place_weekly, sweep_status_line, Label,
    PlacedLabel,
};
use crate::app::text::visual_width;
use crate::app::view_models::TerminalSize;
use crate::app::weekly_usage::WeeklyIndicator;
use crate::theme::{
    COLOR_GUTTER_AGENT_COMMENTARY, COLOR_GUTTER_AGENT_THINKING, COLOR_GUTTER_USER, COLOR_STEP8,
    COLOR_TEXT,
};

#[derive(Default)]
pub(super) struct StatusClock {
    minute: Option<Instant>,
    animation: Option<u128>,
}

impl StatusClock {
    fn tick(&mut self, now: Instant, animation: u128, running: bool) -> bool {
        let animated = running && self.animation != Some(animation);
        self.animation = Some(animation);
        let minute = self.minute.is_none_or(|until| until <= now);
        if minute {
            self.minute = Some(now + Duration::from_secs(60));
        }
        animated || minute
    }
}

impl Device {
    pub(super) fn draw_footer(
        &self,
        buffer: &mut Buffer,
        size: TerminalSize,
        layout: &InputLayout,
        content_x: usize,
    ) {
        let info = self
            .client
            .session(&self.session)
            .map(|replica| replica.info());
        let y = layout.input_top.saturating_sub(2);
        if let Some(hint) = self.footer_hint(info, size.width) {
            let hint = self.dictation_status(self.task_status(self.queue_status(hint)));
            let mut edge = size.width.saturating_sub(1);
            let mut labels = Vec::new();
            if let Some(text) = self.callback_hint(size.width, info.map(|info| info.backend)) {
                place(
                    &mut labels,
                    &mut edge,
                    Label::plain(text, COLOR_GUTTER_USER),
                );
            }
            draw_status_separator(buffer, y, edge, false);
            draw_labels(buffer, y, &labels);
            draw_str(
                buffer,
                content_x,
                y,
                &hint,
                Style::default().fg(COLOR_TEXT),
                edge.saturating_sub(content_x),
            );
        } else if let Some(info) = info {
            self.draw_status(buffer, size, y, info);
        }
        let name = self
            .view
            .input
            .is_empty()
            .then(|| info.and_then(|info| info.name.as_deref()))
            .flatten();
        draw_composer(
            buffer,
            size,
            layout,
            content_x,
            self.history.rewind_draft.is_some(),
            name,
        );
        self.draw_approval(buffer, size);
    }

    fn footer_hint(&self, info: Option<&SessionInfo>, width: usize) -> Option<String> {
        if let Some(hint) = self.disconnect_hint() {
            return Some(hint);
        }
        if let Some(hint) = &self.view.status_hint {
            return Some(hint.clone());
        }
        if let Some(info) = info.filter(|info| {
            info.backend != Backend::Codex && info.availability == Availability::Closed
        }) {
            let remedy = if info.backend == Backend::Claude {
                "reopen this saved session"
            } else {
                "start a new session"
            };
            return Some(format!(
                "{:?} is closed; {remedy}; draft kept",
                info.backend
            ));
        }
        if let Some(hint) = self.history_hint(width) {
            return Some(hint.into());
        }
        if self.waiting_ready {
            return Some("Waiting for the session to become ready; your draft is kept. Press Enter again when ready".into());
        }
        if self.reading.loading {
            return Some("Loading visible source… · F6 retries failed reads".into());
        }
        let info = match info {
            Some(info) => info,
            None => return Some("Connecting…".into()),
        };
        if info.backend == Backend::Codex && self.client.current_session_has_snapshot(&self.session)
        {
            match info.availability {
                Availability::Closed if self.reconnect.is_some() => {
                    return Some("Requesting Codex reconnect…".into())
                }
                Availability::Closed => return Some("Codex offline · F5 reconnect".into()),
                Availability::Restoring => return Some("Restoring Codex…".into()),
                _ => {}
            }
        }
        if info.availability == Availability::Ready {
            (!self.client.current_session_is_ready(&self.session))
                .then(|| "Restoring session…".into())
        } else {
            Some(format!("{:?}", info.availability))
        }
    }

    fn draw_status(&self, buffer: &mut Buffer, size: TerminalSize, y: usize, info: &SessionInfo) {
        let mut edge = size.width.saturating_sub(1);
        let mut labels = Vec::new();
        self.place_callback(&mut labels, &mut edge, size.width, info);
        if let Some(quota) = info
            .quota
            .as_ref()
            .filter(|quota| quota.remaining_pct.is_finite())
        {
            let used = (100.0 - quota.remaining_pct).round().clamp(0.0, 100.0) as u64;
            if let Some(weekly) = RateWindow::new(used, Some(WEEK_MINUTES), quota.resets_at)
                .indicator(unix_now())
                .map(WeeklyIndicator::from)
            {
                place_weekly(&mut labels, &mut edge, weekly);
            }
        }
        let mode = self.status_mode(info);
        let notice = self.status_queue_notice(size.width, info);
        let reserve = mode.map_or(0, |text| visual_width(text) + 1)
            + notice.as_ref().map_or(0, |text| visual_width(text) + 1);
        place_context(
            &mut labels,
            &mut edge,
            info.context.as_ref().map(|usage| ContextUsage {
                used: usage.used,
                max: usage.max,
            }),
            info.status
                .as_ref()
                .is_some_and(|status| !status.context_fresh),
            reserve,
        );
        for label in notice.into_iter().chain(mode.map(str::to_owned)) {
            place(
                &mut labels,
                &mut edge,
                Label::plain(label, COLOR_GUTTER_USER),
            );
        }
        self.place_status_tasks(&mut labels, &mut edge, info);
        self.place_status_badges(&mut labels, &mut edge, info);
        draw_status_separator(buffer, y, edge, false);
        draw_labels(buffer, y, &labels);
        if self.status_running() {
            let width = if self.view.layout.uses_flow(size) || size.is_small() || edge < 8 {
                size.width.saturating_sub(1)
            } else {
                edge
            };
            sweep_status_line(buffer, y, width, info.ralph, animation_tick());
        }
    }

    fn place_status_badges(
        &self,
        labels: &mut Vec<PlacedLabel>,
        edge: &mut usize,
        info: &SessionInfo,
    ) {
        let mut badges = Vec::new();
        let activity = info
            .status
            .as_ref()
            .and_then(|status| status.activity.as_ref())
            .and_then(activity_label);
        let show_phase = activity.is_none();
        if let Some(text) = activity {
            badges.push(Label::plain(text, COLOR_GUTTER_AGENT_THINKING));
        }
        if let Some(text) = self.view.dictation_status_label() {
            badges.push(Label {
                parts: vec![(text, COLOR_GUTTER_AGENT_COMMENTARY)],
                bold: true,
            });
        }
        badges.extend(ralph_badges(info));
        let model = self
            .client
            .session(&self.session)
            .and_then(|replica| replica.settings())
            .map(|settings| settings.label.as_str())
            .or(info.model.as_deref())
            .unwrap_or("model?");
        let reserved: usize = badges.iter().map(|badge| badge.width() + 1).sum();
        let phase = format!("{:?}", info.phase);
        if show_phase && visual_width(model) + reserved + phase.len() + 4 < *edge {
            badges.push(Label::plain(phase, COLOR_STEP8));
        }
        let reserved: usize = badges.iter().map(|badge| badge.width() + 1).sum();
        place(
            labels,
            edge,
            Label::plain(
                fit_model(model, edge.saturating_sub(reserved + 2)),
                COLOR_STEP8,
            ),
        );
        for badge in badges {
            place(labels, edge, badge);
        }
    }

    fn place_status_tasks(
        &self,
        labels: &mut Vec<PlacedLabel>,
        edge: &mut usize,
        info: &SessionInfo,
    ) {
        let active = self.task_facts().map_or(0, |facts| facts.counts.active);
        let uncertain = info
            .status
            .as_ref()
            .is_some_and(|status| status.tasks_uncertain);
        let text = if active == 0 && uncertain {
            Some("Tasks?".into())
        } else if active > 0 {
            Some(format!(
                "{active}{} {}",
                if uncertain { "+" } else { "" },
                if active == 1 { "task" } else { "tasks" }
            ))
        } else {
            None
        };
        if let Some(text) = text {
            place(labels, edge, Label::plain(text, COLOR_GUTTER_USER));
        }
    }

    fn place_callback(
        &self,
        labels: &mut Vec<PlacedLabel>,
        edge: &mut usize,
        width: usize,
        info: &SessionInfo,
    ) {
        if let Some(text) = self.callback_hint(width, Some(info.backend)) {
            place(labels, edge, Label::plain(text, COLOR_GUTTER_USER));
        }
    }

    fn callback_hint(&self, width: usize, backend: Option<Backend>) -> Option<String> {
        let count = self.client.session(&self.session).map_or(0, |replica| {
            replica
                .approvals()
                .iter()
                .filter(|card| !card.submitted)
                .count()
        });
        if self.selected_approval().is_some() || count == 0 {
            return None;
        }
        Some(if width >= 60 && backend == Some(Backend::Codex) {
            format!("F4: Codex request waiting ({count})")
        } else if width >= 30 {
            format!("F4: request waiting ({count})")
        } else if width >= 16 {
            format!("F4 request ({count})")
        } else {
            "F4".into()
        })
    }

    fn status_mode(&self, info: &SessionInfo) -> Option<&'static str> {
        let queued = if info.backend == Backend::Claude {
            self.tasks.view.queue_mode
        } else {
            self.view.codex_queue.mode
        };
        if queued {
            Some("Queue")
        } else if self
            .view
            .viewport
            .steer_notice_until
            .is_some_and(|until| until > Instant::now())
        {
            Some("Steer")
        } else {
            None
        }
    }

    fn status_queue_notice(&self, width: usize, info: &SessionInfo) -> Option<String> {
        if info.backend != Backend::Codex {
            return None;
        }
        let compact = width < 32;
        if self.queue_pending() {
            Some(if compact { "Q…" } else { "Queue pending" }.into())
        } else if self.view.codex_queue.recovery.is_some() {
            Some(if compact { "Draft" } else { "Unsent draft" }.into())
        } else if self.view.codex_queue.error.is_some()
            || self.queue.reads.error().is_some()
            || self
                .queue_facts()
                .is_some_and(|facts| facts.read_error.is_some())
        {
            Some(if compact { "Q!" } else { "Queue error" }.into())
        } else {
            self.queue_facts()
                .filter(|facts| facts.known && facts.count > 0)
                .map(|facts| format!("{} queued", facts.count))
        }
    }

    fn status_running(&self) -> bool {
        self.client.session(&self.session).is_some_and(|replica| {
            replica.info().current_turn.is_some()
                || replica
                    .info()
                    .status
                    .as_ref()
                    .and_then(|status| status.activity.as_ref())
                    .is_some_and(|activity| activity.state == TaskMainState::Running)
                || replica.tasks().is_some_and(|tasks| tasks.counts.active > 0)
        })
    }

    pub(super) fn tick_status_clock(&mut self) -> bool {
        let running = self.status_running();
        let changed = self.status.tick(Instant::now(), animation_tick(), running);
        !self.tasks.view.show && changed
    }
}

fn activity_label(activity: &carlos_proto::status::MainActivity) -> Option<String> {
    activity
        .transient
        .as_ref()
        .map(|label| safe_label(&label.text))
        .or_else(|| {
            activity
                .thinking_tokens
                .map(|tokens| format!("Thinking ~{tokens} tokens"))
        })
        .or_else(|| match activity.state {
            TaskMainState::Idle => None,
            TaskMainState::Running => Some("Running".into()),
            TaskMainState::RequiresAction => Some("Requires action".into()),
        })
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
