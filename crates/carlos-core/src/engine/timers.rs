//! Wake native reads and automatic prompts at their existing deadlines.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::owner::Owner;
use crate::backend::BackendKind;
use crate::session::actions::ServerRequestAction;
use crate::session::codex::{goals, queue, reconnect_runtime, reconnect_streams};

impl Owner {
    pub(super) fn tick(&mut self) {
        if self.native.closed {
            return;
        }
        let now = Instant::now();
        if self.client.kind() == BackendKind::Claude {
            self.tick_claude_reads(now);
        } else if self.client.kind() == BackendKind::Codex {
            let scope = self.session.codex_scope();
            reconnect_streams::tick(self.client.as_ref(), &mut self.session, now);
            reconnect_runtime::tick(self.client.as_ref(), &mut self.session);
            let panel_scope = self.task_read_scope();
            crate::session::codex::task_status::tick(
                self.client.as_ref(),
                &mut self.session,
                panel_scope,
                now,
            );
            crate::session::codex::panel::tick(
                self.client.as_ref(),
                &mut self.session,
                panel_scope,
                &mut Default::default(),
            );
            let goals_visible = self.channels.goal_interest.load(Ordering::SeqCst);
            goals::tick(
                self.client.as_ref(),
                &mut self.session,
                scope,
                now,
                goals_visible,
            );
            queue::tick(
                self.client.as_ref(),
                &mut self.session,
                scope,
                &mut Default::default(),
                false,
            );
        }
    }

    fn tick_claude_reads(&mut self, now: Instant) {
        let (_, request) = self.session.prepare_claude_context(now);
        if let Some(params) = request {
            self.action(ServerRequestAction::ClaudeControl {
                method: "context".into(),
                params,
            });
        }
        if let Some(params) = self.session.prepare_claude_quota(now) {
            self.quota_action(params);
        }
    }

    pub(super) fn wait(&self) -> Duration {
        let now = Instant::now();
        [
            self.session.claude_context_wait(now),
            crate::session::codex::panel::wait(&self.session, self.task_read_scope(), now),
            crate::session::codex::task_status::wait(&self.session, self.task_read_scope(), now),
            self.session.pending_ralph_continuation_wait(now),
            (!self.native.closed)
                .then(|| {
                    goals::wait(
                        &self.session,
                        self.session.codex_scope(),
                        now,
                        self.channels.goal_interest.load(Ordering::SeqCst),
                    )
                })
                .flatten(),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(Duration::from_secs(1))
        .min(Duration::from_secs(1))
    }
}
