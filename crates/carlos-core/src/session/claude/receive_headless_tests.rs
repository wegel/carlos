//! Check named Claude failures and clears without constructing a terminal client.

use serde_json::json;

use super::receive_update::{ClaudeDraftContext, ClaudeEffect};
use super::ClaudePhase;
use crate::session::state::SessionState;

#[test]
fn failed_start_returns_each_submitted_draft_once_and_preserves_newer_typing() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.begin_claude_edit("edited text".into());
    session.queue_turn_input_with_history("queued first", true);
    session.queue_turn_input_with_history("queued second", true);
    let lines = ["newer text".to_string()];
    let draft = ClaudeDraftContext {
        composer: &lines,
        confirmation: None,
    };
    let frame = json!({"generation":1,"message":"fixed startup error"});
    let update = session
        .receive_claude_message("claude/sessionStartFailed", &frame, &draft)
        .expect("named failure consumed");
    let returned: Vec<_> = update
        .effects
        .iter()
        .filter_map(|effect| match effect {
            ClaudeEffect::RetainDraft(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(returned, ["edited text", "queued first", "queued second"]);
    assert!(update.effects.iter().all(|effect| !matches!(
        effect,
        ClaudeEffect::SetInput(_) | ClaudeEffect::ClearInput | ClaudeEffect::LoadRetained
    )));
    assert_eq!(lines, ["newer text"]);
    assert_eq!(session.claude().phase(), ClaudePhase::Closed);
    assert!(session.claude_edit_progress().is_none());
    assert_eq!(session.messages().len(), 4);
    let second = session
        .receive_claude_message("claude/sessionStartFailed", &frame, &draft)
        .expect("repeated failure consumed");
    assert!(second
        .effects
        .iter()
        .all(|effect| !matches!(effect, ClaudeEffect::RetainDraft(_))));
}

#[test]
fn native_clear_keeps_callback_identity_until_its_exact_cancellation() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let draft = ClaudeDraftContext {
        composer: &[],
        confirmation: None,
    };
    let callback = json!({"generation":1,"callbackId":"request","kind":"permission",
        "payload":{"toolName":"Bash","defaultToNo":true}});
    session
        .receive_claude_message("claude/callback", &callback, &draft)
        .expect("callback accepted");
    let card = session
        .approval()
        .pending()
        .expect("live callback")
        .entry_id();
    let reset = json!({"generation":1,"epoch":1,"new_conversation_id":"native-clear"});
    session
        .receive_claude_message("claude/conversationReset", &reset, &draft)
        .expect("clear accepted");
    assert_eq!(
        session
            .approval()
            .pending()
            .expect("callback survives")
            .entry_id(),
        card
    );
    let draft = ClaudeDraftContext {
        confirmation: Some(card),
        ..draft
    };
    let update = session
        .receive_claude_message(
            "claude/callbackCancelled",
            &json!({"generation":1,"epoch":0,"callbackId":"request"}),
            &draft,
        )
        .expect("process callback survives epoch fence");
    assert!(update.effects.iter().any(|effect| matches!(
        effect,
        ClaudeEffect::Cancelled {
            clear_confirmation: true,
            ..
        }
    )));
    assert!(session.approval().pending().is_none());
    assert!(
        session.approval_answer(card).is_none(),
        "retired card cannot answer"
    );
}
