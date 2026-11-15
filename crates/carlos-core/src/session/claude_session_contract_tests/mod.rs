//! Keep native session identity and exact private recovery effects in Core without a terminal owner.

mod identity;
mod recovery;

use carlos_proto::submissions::{ClientOperation, RecoveryKind, SubmissionOutcome};
use serde_json::{json, Value};

use super::backend_test_support::normalize_updates;
use super::backend_update::BackendChange;
use super::claude::receive_update::ClaudeEffect;
use super::state::SessionState;
use crate::claude_backend::FixtureNormalizer;

/// Reduce a native fixture through its real normalizer and preserve submission effects.
pub(super) fn send(
    core: &mut SessionState,
    native: &mut FixtureNormalizer,
    method: &str,
    generation: u64,
    mut params: Value,
) -> Vec<SubmissionOutcome> {
    params["generation"] = json!(generation);
    normalize_updates(core, native, &json!({"method":method,"params":params}))
        .into_iter()
        .flat_map(|update| match update.change {
            BackendChange::Claude(update) => update.effects,
            _ => Vec::new(),
        })
        .filter_map(|effect| match effect {
            ClaudeEffect::Submission(outcome) => Some(outcome),
            _ => None,
        })
        .collect()
}

fn frame(
    core: &mut SessionState,
    native: &mut FixtureNormalizer,
    generation: u64,
    frame: Value,
    method: Option<&str>,
) -> Vec<SubmissionOutcome> {
    send(
        core,
        native,
        "claude/controller",
        generation,
        json!({"frame":frame,"requestMethod":method}),
    )
}

fn switch(core: &mut SessionState, native: &mut FixtureNormalizer) -> Vec<SubmissionOutcome> {
    send(
        core,
        native,
        "claude/sessionChanged",
        2,
        json!({"sessionId":"new-session"}),
    )
}

/// Borrow exact recovery text, source and operation from native submission effects.
pub(super) fn recovered(
    outcomes: &[SubmissionOutcome],
) -> Vec<(String, RecoveryKind, Option<ClientOperation>)> {
    outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            SubmissionOutcome::Recovered { draft, kind } => {
                Some((draft.text.clone(), *kind, draft.operation.clone()))
            }
            _ => None,
        })
        .collect()
}

fn core(root: &str) -> SessionState {
    SessionState::new(root.into(), Default::default(), false)
}

fn operation(id: &str) -> ClientOperation {
    ClientOperation {
        client: "a".repeat(32).parse().expect("client"),
        id: id.parse().expect("operation"),
    }
}
