//! Keep native callback policy facts free of terminal keys and retire old helper authority.

use serde_json::{json, Value};

use super::state::SessionState;

/// Keep the original reference checks on the owner rather than a terminal forwarder.
pub(super) fn assert_owner_invariants(core: &SessionState) {
    core.messages().assert_invariants();
    core.input_history().assert_invariants(core.messages());
    core.assert_summary_invariants();
    for prompt in core.claude_history().unknown() {
        assert!(core.messages().entry(prompt.entry_id()).is_some());
    }
}

fn payloads() -> Vec<(&'static str, Value)> {
    vec![
        (
            "permission",
            json!({"toolName":"Bash","input":{"command":"echo old"}}),
        ),
        (
            "permission",
            json!({"toolName":"ExitPlanMode","input":{"plan":"Plan","planFilePath":"plan.txt"}}),
        ),
        (
            "permission",
            json!({"toolName":"AskUserQuestion","input":{"questions":[{
            "question":"Color?","header":"Color","options":[{"label":"Blue"}]}]}}),
        ),
        (
            "elicitation",
            json!({"serverName":"server","requestedSchema":{"type":"object","properties":{}}}),
        ),
        (
            "elicitation",
            json!({"serverName":"server","requestedSchema":{"type":"object",
            "properties":{"count":{"type":"integer"}},"required":["count"]}}),
        ),
        (
            "elicitation",
            json!({"serverName":"server","mode":"url","url":"https://example.com/path","urlHost":"example.com"}),
        ),
        (
            "elicitation",
            json!({"serverName":"server","requestedSchema":{"type":"object",
            "properties":{"nested":{"type":"object"}}}}),
        ),
    ]
}

#[test]
fn callback_facts_never_store_carlos_keyboard_or_browser_instructions() {
    for (kind, mut payload) in payloads() {
        payload["defaultToNo"] = json!(true);
        let mut core = SessionState::new("thread".into(), Default::default(), false);
        core.receive_claude_callback(&json!({"callbackId":"one","kind":kind,"payload":payload}));
        let pending = core.approval().pending().expect("native callback");
        for line in &pending.facts().detail_lines {
            assert!(
                !["Press y", "Ctrl+S", "Ctrl+Y", "Accept consents"]
                    .iter()
                    .any(|key| line.contains(key)),
                "{kind}: {line}"
            );
        }
        assert_eq!(
            pending
                .facts()
                .requested_permissions
                .as_ref()
                .expect("original payload")["defaultToNo"],
            true
        );
        assert!(core.export_approvals()[0].default_to_no);
        assert_owner_invariants(&core);
    }
}

#[test]
fn codex_question_facts_keep_policy_without_local_key_names() {
    for (method, params) in [
        (
            "item/tool/requestUserInput",
            json!({"threadId":"thread","turnId":"turn","itemId":"item",
            "isBlocking":false,"questions":[{"id":"q","header":"Header","question":"Color?","options":[]}]}),
        ),
        (
            "mcpServer/elicitation/request",
            json!({"threadId":"thread","serverName":"server","mode":"url",
            "url":"https://example.com/path"}),
        ),
    ] {
        let mut core = SessionState::new("thread".into(), Default::default(), false);
        core.enable_codex();
        core.accept_codex_callback(method, &json!({"id":"one","params":params}));
        let pending = core.approval().pending().expect("native question");
        assert!(pending
            .facts()
            .detail_lines
            .iter()
            .all(|line| !line.contains("Ctrl+Y") && !line.contains("Esc")));
        assert_eq!(pending.facts().request_id, "one");
        assert_eq!(core.export_approvals().len(), 1);
        assert_owner_invariants(&core);
    }
}

#[test]
fn a_new_helper_cannot_reuse_the_original_cards_authority_for_a_reused_native_id() {
    let mut core = SessionState::new("thread".into(), Default::default(), false);
    core.enable_claude();
    core.set_claude_generation_for_test(1);
    core.set_claude_phase_for_test(super::claude::ClaudePhase::Ready);
    core.receive_claude_message(
        "claude/callback",
        &json!({"generation":1,"callbackId":"reused-id","kind":"permission",
        "payload":{"toolName":"Bash","input":{"command":"echo first"},"defaultToNo":true}}),
    )
    .expect("first helper callback");
    let original = core.approval().pending().expect("original card").entry_id();
    core.receive_claude_message(
        "claude/sessionChanged",
        &json!({"generation":2,"sessionId":"second-thread"}),
    )
    .expect("helper replacement");
    assert!(core.approval_answer(original).is_none());
    core.receive_claude_message(
        "claude/callback",
        &json!({"generation":2,"callbackId":"reused-id","kind":"permission",
        "payload":{"toolName":"Bash","input":{"command":"echo different"},"defaultToNo":true}}),
    )
    .expect("new helper callback");
    let current = core.approval().pending().expect("replacement card");
    assert_ne!(current.entry_id(), original);
    assert_eq!(current.facts().request_id, "reused-id");
    assert!(core.export_approvals()[0].default_to_no);
    assert!(core.approval_answer(original).is_none());
    assert_owner_invariants(&core);
}
