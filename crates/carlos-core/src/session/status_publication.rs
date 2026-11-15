//! Export bounded native activity and automatic-work guards for every session subscriber.

use carlos_proto::bytes::ByteLength;
use carlos_proto::status::{MainActivity, RalphStatus, StatusFacts, MAX_STATUS_LABEL_BYTES};
use carlos_proto::task_overview::{TaskLabel, TaskMainState};

use super::claude::NativeState;
use super::codex::task_badge;
use super::state::SessionState;

impl SessionState {
    pub(super) fn export_status(&self) -> StatusFacts {
        let activity = self.claude.enabled().then(|| {
            let activity = self.claude.activity();
            MainActivity {
                state: match activity.native_state() {
                    NativeState::Idle => TaskMainState::Idle,
                    NativeState::Running => TaskMainState::Running,
                    NativeState::RequiresAction => TaskMainState::RequiresAction,
                },
                thinking_tokens: activity.thinking(),
                transient: activity.transient().map(status_label),
            }
        });
        StatusFacts {
            context_fresh: !self.claude.enabled() || self.claude.context_fresh(),
            active_reasoning: self
                .active_reasoning_step()
                .map(|index| self.messages()[index].id.to_wire()),
            activity,
            callbacks_pending: self
                .approval
                .live()
                .filter(|card| !card.submitted())
                .count()
                .min(u32::MAX as usize) as u32,
            tasks_uncertain: self.codex_scope().enabled()
                && !self.codex_scope().closed()
                && self
                    .codex_task_status()
                    .errors()
                    .keys()
                    .any(|owner| task_badge::owns(self, self.backend_context().panel, owner)),
            ralph: RalphStatus {
                toggle_pending: self.ralph_runtime.toggle_pending(),
                waiting_for_user: self.ralph_runtime.is_waiting_for_user(),
                continuation_pending: self.has_pending_ralph_continuation(),
            },
        }
    }
}

fn status_label(text: &str) -> TaskLabel {
    let complete = text.len() <= MAX_STATUS_LABEL_BYTES;
    let value = if complete {
        text.to_owned()
    } else {
        let mut end = MAX_STATUS_LABEL_BYTES - '…'.len_utf8();
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &text[..end])
    };
    TaskLabel {
        text: value,
        bytes: ByteLength::new(text.len() as u64),
        complete,
    }
}

#[cfg(test)]
#[path = "status_publication_tests.rs"]
mod tests;
