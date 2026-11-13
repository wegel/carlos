//! Check named Claude failures and clears without constructing a terminal client.

use carlos_proto::submissions::SubmissionOutcome;

use serde_json::json;

use super::receive_update::ClaudeEffect;
use super::ClaudePhase;
use crate::session::state::SessionState;

#[test]
fn failed_start_returns_each_submitted_draft_once_and_preserves_newer_typing() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.begin_claude_edit("edited text".into());
    session.queue_turn_input_with_history("queued first", true);
    session.queue_turn_input_with_history("queued second", true);
    let lines = ["newer text".to_string()];
    let frame = json!({"generation":1,"message":"fixed startup error"});
    let update = session
        .receive_claude_message("claude/sessionStartFailed", &frame)
        .expect("named failure consumed");
    let returned: Vec<_> = update
        .effects
        .iter()
        .filter_map(|effect| match effect {
            ClaudeEffect::Submission(SubmissionOutcome::Recovered { draft, .. }) => {
                Some(draft.text.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(returned, ["edited text", "queued first", "queued second"]);
    assert_eq!(lines, ["newer text"]);
    assert_eq!(session.claude().phase(), ClaudePhase::Closed);
    assert!(session.claude_edit_progress().is_none());
    assert_eq!(session.messages().len(), 4);
    let second = session
        .receive_claude_message("claude/sessionStartFailed", &frame)
        .expect("repeated failure consumed");
    assert!(second.effects.iter().all(|effect| !matches!(
        effect,
        ClaudeEffect::Submission(SubmissionOutcome::Recovered { .. })
    )));
}

#[test]
fn native_clear_keeps_callback_identity_until_its_exact_cancellation() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let callback = json!({"generation":1,"callbackId":"request","kind":"permission",
        "payload":{"toolName":"Bash","defaultToNo":true}});
    session
        .receive_claude_message("claude/callback", &callback)
        .expect("callback accepted");
    let card = session
        .approval()
        .pending()
        .expect("live callback")
        .entry_id();
    let reset = json!({"generation":1,"epoch":1,"new_conversation_id":"native-clear"});
    session
        .receive_claude_message("claude/conversationReset", &reset)
        .expect("clear accepted");
    assert_eq!(
        session
            .approval()
            .pending()
            .expect("callback survives")
            .entry_id(),
        card
    );
    let update = session
        .receive_claude_message(
            "claude/callbackCancelled",
            &json!({"generation":1,"epoch":0,"callbackId":"request"}),
        )
        .expect("process callback survives epoch fence");
    assert!(update.effects.iter().any(
        |effect| matches!(effect, ClaudeEffect::Cancelled { card: Some(id), .. } if *id == card)
    ));
    assert!(session.approval().pending().is_none());
    assert!(
        session.approval_answer(card).is_none(),
        "retired card cannot answer"
    );
}

#[test]
fn failed_edit_and_queued_text_keep_different_client_operation_owners() {
    use carlos_proto::submissions::{ClientOperation, SubmittedText};
    let operation = |digit: char| ClientOperation {
        client: digit.to_string().repeat(32).parse().expect("client"),
        id: digit.to_string().repeat(32).parse().expect("operation"),
    };
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let first = operation('a');
    let second = operation('b');
    session.begin_claude_edit_for("edited".into(), Some(first.clone()));
    session.queue_submitted_input(
        SubmittedText {
            text: "queued".into(),
            operation: Some(second.clone()),
        },
        true,
    );
    let update = session
        .receive_claude_message("claude/sessionStartFailed", &json!({"generation":1}))
        .expect("failure");
    let drafts: Vec<_> = update
        .effects
        .into_iter()
        .filter_map(|effect| match effect {
            ClaudeEffect::Submission(SubmissionOutcome::Recovered { draft, .. }) => Some(draft),
            _ => None,
        })
        .collect();
    assert_eq!(drafts.len(), 2);
    assert_eq!(drafts[0].operation.as_ref(), Some(&first));
    assert_eq!(drafts[1].operation.as_ref(), Some(&second));
    assert_eq!(drafts[0].text, "edited");
    assert_eq!(drafts[1].text, "queued");
}
