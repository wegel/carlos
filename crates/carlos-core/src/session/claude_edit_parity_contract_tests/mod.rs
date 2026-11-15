//! Exercise native Claude edit stages without a terminal or backend-client recorder.

mod binding;
mod queue;
pub(super) mod send;
mod stages;

use std::time::Instant;

use carlos_proto::session::Backend;
use carlos_proto::submissions::{RecoveryKind, SubmissionOutcome};
use serde_json::{json, Value};

use super::backend_test_support::context;
use super::backend_update::BackendChange;
use super::claude::receive_update::{ClaudeEffect, ClaudeUpdate};
use super::state::SessionState;
use crate::backend::PromptOrigin;

fn core() -> SessionState {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    core.enable_claude();
    core.set_client_identity(Backend::Claude, "fixture".into());
    core
}

fn event(core: &mut SessionState, method: &str, mut params: Value) -> ClaudeUpdate {
    params["generation"] = json!(core.claude().generation());
    let update = core.handle_backend_line(
        &json!({"method":method,"params":params}).to_string(),
        &context(core),
    );
    match update.change {
        BackendChange::Claude(update) => update,
        _ => panic!("native edit fact"),
    }
}

fn advance(core: &mut SessionState, stage: usize) {
    core.begin_claude_edit("edited text".into());
    core.retire_claude_queue(&mut Default::default());
    if stage >= 1 {
        core.switch_claude_session(2, "branch", &mut Default::default());
    }
    if stage >= 2 {
        core.receive_claude_capabilities(&json!({"models":[]}), &mut Default::default());
    }
    if stage >= 3 {
        let input = core
            .dequeue_turn_input(Instant::now())
            .expect("queued edit");
        core.dispatch_claude_edit(&input.text, input.origin, None);
    }
    if stage >= 4 {
        core.bind_claude_edit(
            "edited text",
            PromptOrigin::Human,
            None,
            &json!({"result":{"turn":{"id":"edit-uuid"}}}),
        );
    }
}

/// Borrow only submitted text and source from a native update.
pub(super) fn recovered(update: &ClaudeUpdate) -> Vec<(&str, RecoveryKind)> {
    update
        .effects
        .iter()
        .filter_map(|effect| match effect {
            ClaudeEffect::Submission(SubmissionOutcome::Recovered { draft, kind }) => {
                Some((draft.text.as_str(), *kind))
            }
            _ => None,
        })
        .collect()
}

fn notices(core: &SessionState) -> Vec<&str> {
    core.messages()
        .iter()
        .map(|message| message.text.as_str())
        .collect()
}

/// Apply one native edit failure without a client-side draft.
pub(super) fn failed(core: &mut SessionState, text: &str) -> ClaudeUpdate {
    let mut update = ClaudeUpdate::default();
    core.fail_claude_edit(&mut update, text);
    update
}

fn accepted(core: &mut SessionState, id: &str) -> ClaudeUpdate {
    let mut update = ClaudeUpdate::default();
    core.receive_claude_edit_acceptance(id, &mut update);
    update
}
