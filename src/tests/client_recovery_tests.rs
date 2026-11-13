//! One client's rejected submission cannot change a different client's local drafts.

use carlos_core::session::claude::receive_update::{ClaudeEffect, ClaudeUpdate};
use carlos_proto::submissions::{RecoveryKind, SubmissionOutcome, SubmittedText};

use super::claude_edit_parity_support::app;

#[test]
fn two_clients_apply_the_same_correlated_recovery_without_overwriting_newer_text() {
    let mut first = app();
    let mut second = app();
    let operation = first.view.new_operation();
    first
        .session
        .begin_claude_edit_for("submitted edit".into(), Some(operation.clone()));
    first.set_input_text("typed ahead");
    second.set_input_text("second device draft");
    let mut update = ClaudeUpdate::default();
    first.session.fail_claude_edit(&mut update, "failed fork");
    let recovery = update
        .effects
        .iter()
        .find_map(|effect| match effect {
            ClaudeEffect::Submission(outcome @ SubmissionOutcome::Recovered { .. }) => {
                Some(outcome.clone())
            }
            _ => None,
        })
        .expect("correlated recovery");
    first.apply_claude_update(update);
    let mut remote = ClaudeUpdate::default();
    remote.effects.push(ClaudeEffect::Submission(recovery));
    second.apply_claude_update(remote);
    assert_eq!(first.input_text(), "typed ahead");
    assert_eq!(second.input_text(), "second device draft");
    assert_eq!(
        first
            .view
            .claude_edit
            .unsent_drafts
            .iter()
            .collect::<Vec<_>>(),
        ["submitted edit"]
    );
    assert!(second.view.claude_edit.unsent_drafts.is_empty());
}

#[test]
fn empty_originating_editor_recovers_but_empty_other_editor_stays_empty() {
    let mut first = app();
    let mut second = app();
    let draft = SubmittedText {
        text: "submitted".into(),
        operation: Some(first.view.new_operation()),
    };
    for client in [&mut first, &mut second] {
        let outcome = SubmissionOutcome::Recovered {
            draft: draft.clone(),
            kind: RecoveryKind::Prompt,
        };
        let mut update = ClaudeUpdate::default();
        update.effects.push(ClaudeEffect::Submission(outcome));
        client.apply_claude_update(update);
    }
    assert_eq!(first.input_text(), "submitted");
    assert!(second.input_is_empty());
}
