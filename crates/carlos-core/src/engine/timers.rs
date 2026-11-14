//! Wake native reads and automatic prompts at their existing deadlines.

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
            let (_, request) = self.session.prepare_claude_context(now);
            if let Some(params) = request {
                self.action(ServerRequestAction::ClaudeControl {
                    method: "context".into(),
                    params,
                });
            }
        } else if self.client.kind() == BackendKind::Codex {
            let scope = self.session.codex_scope();
            reconnect_streams::tick(self.client.as_ref(), &mut self.session, now);
            reconnect_runtime::tick(self.client.as_ref(), &mut self.session);
            let panel_scope = self.task_read_scope();
            crate::session::codex::panel::tick(
                self.client.as_ref(),
                &mut self.session,
                panel_scope,
                &mut Default::default(),
            );
            goals::tick(self.client.as_ref(), &mut self.session, scope, now, true);
            queue::tick(
                self.client.as_ref(),
                &mut self.session,
                scope,
                &mut Default::default(),
                false,
            );
        }
    }

    pub(super) fn wait(&self) -> Duration {
        let now = Instant::now();
        [
            self.session.claude_context_wait(now),
            self.session.pending_ralph_continuation_wait(now),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(Duration::from_secs(1))
        .min(Duration::from_secs(1))
    }
}
