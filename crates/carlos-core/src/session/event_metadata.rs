//! Compare borrowed header facts before allocating a new wire description.

use carlos_proto::session::{Availability, Backend, Context, Phase, Quota, SessionInfo};

use super::state::SessionState;
use super::usage::unix_now;

impl SessionState {
    pub(super) fn refresh_wire_turn(&mut self) {
        if self.active_turn_id.as_deref()
            != self
                .events
                .current_turn
                .as_ref()
                .map(|(native, _)| native.as_str())
        {
            self.events.current_turn = self.active_turn_id.as_ref().map(|native| {
                (
                    native.clone(),
                    uuid::Uuid::new_v4()
                        .simple()
                        .to_string()
                        .parse()
                        .expect("turn id"),
                )
            });
        }
    }

    fn event_availability(&self) -> Availability {
        if self.events.backend_closed {
            return Availability::Closed;
        }
        match self.events.backend {
            Backend::Codex if self.codex_scope().closed() => Availability::Closed,
            Backend::Claude => match self.claude.phase {
                super::claude::ClaudePhase::Ready => Availability::Ready,
                super::claude::ClaudePhase::Closed => Availability::Closed,
                _ => Availability::Starting,
            },
            _ => Availability::Ready,
        }
    }

    pub(super) fn event_phase(&self) -> Phase {
        if self.approval.pending().is_some() {
            Phase::Approval
        } else if self.active_turn_id.is_some() {
            Phase::Running
        } else {
            Phase::Idle
        }
    }

    pub(super) fn event_quota(&self) -> Option<Quota> {
        if self.events.backend != Backend::Codex {
            return None;
        }
        let now = unix_now();
        self.rate_limits.indicator_at(now).map(|quota| Quota {
            remaining_pct: quota.remaining() as f64,
            resets_at: Some(now + quota.remaining_seconds()),
        })
    }

    pub(super) fn info_changed(&self) -> bool {
        let Some(info) = self.events.info.as_ref() else {
            return true;
        };
        info.name != self.thread_name
            || info.model != self.runtime.current_model
            || info.backend != self.events.backend
            || info.host != self.events.host
            || info.cwd.as_deref() != self.summaries.cwd.to_str().filter(|cwd| !cwd.is_empty())
            || info.phase != self.event_phase()
            || info.availability != self.event_availability()
            || info.current_turn.as_ref() != self.events.current_turn.as_ref().map(|(_, id)| id)
            || info.ralph != self.ralph_enabled()
            || info.turn_started_at != self.event_turn_start()
            || info.context
                != self.context_usage.map(|c| Context {
                    used: c.used,
                    max: c.max,
                })
            || info.quota != self.event_quota()
    }

    fn event_turn_start(&self) -> Option<i64> {
        self.active_turn_id
            .as_ref()
            .and(self.events.turn_started_at)
    }

    pub(super) fn export_info(&self) -> SessionInfo {
        SessionInfo {
            id: self.events.id.clone(),
            name: self.thread_name.clone(),
            backend: self.events.backend,
            model: self.runtime.current_model.clone(),
            host: self.events.host.clone(),
            cwd: self
                .summaries
                .cwd
                .to_str()
                .filter(|cwd| !cwd.is_empty())
                .map(str::to_owned),
            phase: self.event_phase(),
            availability: self.event_availability(),
            current_turn: self.events.current_turn.as_ref().map(|(_, id)| id.clone()),
            ralph: self.ralph_enabled(),
            turn_started_at: self.event_turn_start(),
            context: self.context_usage.map(|c| Context {
                used: c.used,
                max: c.max,
            }),
            quota: self.event_quota(),
        }
    }
}
