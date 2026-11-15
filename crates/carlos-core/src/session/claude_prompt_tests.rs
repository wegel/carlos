//! Enumerate native accepted-input orders, authors, bounds and UUID echo rules in Core.

use serde_json::{json, Value};

use super::backend_test_support::{action, context};
use super::models::{MessageKind, Role, TurnStatus};
use super::state::SessionState;

fn new_session() -> SessionState {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    core.enable_claude();
    core
}

fn fact(core: &mut SessionState, method: &str, params: Value) {
    let update = core.handle_backend_line(
        &json!({"method":method,"params":params}).to_string(),
        &context(core),
    );
    assert!(
        action(update).is_none(),
        "fixed prompt facts need no native read"
    );
}

fn accepted(core: &mut SessionState, id: &str, text: &str, origin: &str) {
    fact(
        core,
        "claude/promptSubmitted",
        json!({"uuid":id,"text":text,"origin":origin}),
    );
}

fn consume(core: &mut SessionState, ids: &[String]) {
    fact(core, "claude/promptsConsumed", json!({"ids":ids}));
}

fn reset(core: &mut SessionState, clear: Option<&str>) {
    core.reset_claude_prompts(clear);
}

#[test]
fn all_six_accept_reject_consume_orders_preserve_only_observed_rows() {
    for (order, rows, pending) in [
        ([0, 1, 2], 0, false),
        ([0, 2, 1], 1, false),
        ([1, 0, 2], 1, true),
        ([1, 2, 0], 0, true),
        ([2, 0, 1], 0, false),
        ([2, 1, 0], 0, true),
    ] {
        let mut core = new_session();
        for step in order {
            match step {
                0 => accepted(&mut core, "a", "human text", "human"),
                1 => fact(&mut core, "claude/promptRejected", json!({"uuid":"a"})),
                _ => consume(&mut core, &["a".into()]),
            }
        }
        assert_eq!(core.messages().len(), rows, "{order:?}");
        assert_eq!(core.claude_prompts().next().is_some(), pending, "{order:?}");
        if rows == 1 {
            assert_eq!(core.messages()[0].text, "human text");
            assert_eq!(core.input_history().text(0), "human text");
        }
    }
}

#[test]
fn duplicate_accept_and_consume_keep_first_text_author_and_priority() {
    let mut core = new_session();
    accepted(&mut core, "a", "first text", "human");
    fact(
        &mut core,
        "claude/promptSubmitted",
        json!({"uuid":"a","text":"replacement","origin":"automatic","priority":"now"}),
    );
    consume(&mut core, &["missing".into(), "a".into(), "a".into()]);
    let (_, prompt) = core.claude_prompts().next().expect("accepted");
    assert_eq!(
        (
            prompt.text(),
            prompt.human(),
            prompt.priority(),
            prompt.consumed()
        ),
        ("first text", true, "later", true)
    );
    assert_eq!(core.messages().len(), 1);
    assert_eq!(core.messages()[0].role, Role::User);
    assert_eq!(core.input_history().history_len(), 1);
}

#[test]
fn malformed_accepts_do_nothing_and_unknown_origin_defaults_to_automatic() {
    let mut core = new_session();
    for params in [
        json!({}),
        json!({"uuid":"a"}),
        json!({"uuid":3,"text":"bad"}),
        json!({"uuid":"a","text":null}),
    ] {
        core.register_claude_prompt(&params)
            .expect("malformed optional input is ignored");
    }
    assert!(core.claude_prompts().next().is_none());
    accepted(&mut core, "", "unknown source", "unknown");
    consume(&mut core, &["".into()]);
    assert_eq!(core.messages()[0].text, "Automatic prompt: unknown source");
    assert_eq!(core.messages()[0].role, Role::System);
    assert_eq!(core.input_history().history_len(), 0);
}

#[test]
fn consumed_order_wins_over_uuid_order_and_preserves_steering_and_automatic_roles() {
    let mut core = new_session();
    core.record_turn_started("running");
    for (id, origin) in [("z", "human"), ("a", "automatic"), ("m", "human")] {
        fact(
            &mut core,
            "claude/promptSubmitted",
            json!({"uuid":id,"text":id,"origin":origin,"inputKind":"steer","priority":"now"}),
        );
    }
    assert!(core.messages().is_empty());
    consume(&mut core, &["m".into(), "a".into(), "z".into()]);
    let rows: Vec<_> = core
        .messages()
        .iter()
        .map(|m| (m.text.as_str(), m.role, m.kind))
        .collect();
    assert_eq!(
        rows,
        [
            ("m", Role::User, MessageKind::UserSteer),
            ("Automatic prompt: a", Role::System, MessageKind::Plain),
            ("z", Role::User, MessageKind::UserSteer)
        ]
    );
    assert_eq!(core.active_turn_id().as_deref(), Some("z"));
    assert_eq!(core.input_history().history_len(), 2);
}

#[test]
fn reset_retains_unconsumed_and_exact_clear_without_recreating_clear_text() {
    let mut core = new_session();
    for (id, text) in [
        ("old", "old prompt"),
        ("clear", "/clear"),
        ("next", "next prompt"),
    ] {
        accepted(&mut core, id, text, "human");
    }
    consume(&mut core, &["old".into(), "clear".into()]);
    reset(&mut core, Some("clear"));
    core.reset_for_forked_thread();
    core.reset_prompt_history();
    consume(&mut core, &["clear".into(), "next".into()]);
    assert_eq!(
        core.claude_prompts().map(|(id, _)| id).collect::<Vec<_>>(),
        ["clear", "next"]
    );
    assert_eq!(core.messages().len(), 1);
    assert_eq!(core.messages()[0].text, "next prompt");
    assert_eq!(core.input_history().history_len(), 1);
    reset(&mut core, None);
    assert!(core.claude_prompts().next().is_none());
}

#[test]
fn local_echo_is_last_row_only_and_submitted_uuid_respects_active_turn() {
    for (active, expected) in [
        (None, "accepted"),
        (Some("claude-turn-pending"), "accepted"),
        (Some("other"), "other"),
    ] {
        let mut core = new_session();
        if let Some(id) = active {
            core.record_turn_started(id);
        }
        core.append_message(Role::User, "human");
        core.receive_claude_submission(
            &json!({"result":{"turn":{"id":"accepted"},"inputKind":"steer"}}),
        );
        assert_eq!(core.active_turn_id().as_deref(), Some(expected));
        assert_eq!(core.messages()[0].kind, MessageKind::UserSteer);
        assert_eq!(
            core.item_message_index(&core.backend_entry_key("claude-prompt:accepted")),
            Some(0)
        );
    }
    let mut core = new_session();
    core.receive_claude_submission(&json!({"turn":{"id":"ignored"}}));
    assert!(core.active_turn_id().is_none());
    core.attach_claude_prompt_echo(
        "empty",
        &core.backend_entry_key("claude-prompt:empty"),
        Some("steer"),
    );
    core.append_message(Role::System, "last system");
    core.attach_claude_prompt_echo(
        "system",
        &core.backend_entry_key("claude-prompt:system"),
        Some("steer"),
    );
    assert_eq!(core.messages()[0].kind, MessageKind::Plain);
    assert_eq!(
        core.item_message_index(&core.backend_entry_key("claude-prompt:system")),
        Some(0)
    );
}

#[test]
fn accepted_registry_bounds_reject_new_entries_but_allow_exact_duplicates() {
    for (count, text) in [(1024, "x".to_string()), (1, "x".repeat(8 * 1024 * 1024))] {
        let mut core = new_session();
        for index in 0..count {
            core.register_claude_prompt(&json!({"uuid":index.to_string(),"text":text}))
                .expect("within bounds");
        }
        core.register_claude_prompt(&json!({"uuid":"0","text":"duplicate"}))
            .expect("duplicate skips bounds");
        assert_eq!(
            core.register_claude_prompt(&json!({"uuid":"extra","text":"x"})),
            Err("Claude accepted-input history exceeded its bounded queue")
        );
        assert_eq!(core.claude_prompts().count(), count);
    }
}

#[test]
fn helper_failure_fails_registered_and_unregistered_active_turns_without_new_rows() {
    for active in ["a", "unregistered"] {
        let mut core = new_session();
        for id in ["z", "a"] {
            accepted(&mut core, id, id, "human");
        }
        core.record_turn_started(active);
        core.fail_claude(&mut Default::default());
        assert_eq!(core.status(), "turn failed");
        assert!(core.active_turn_id().is_none());
        assert!(core.claude_prompts().next().is_none());
        for id in ["z", "a", active] {
            assert_eq!(core.turn_status_by_id().get(id), Some(&TurnStatus::Failed));
        }
        assert!(core.messages().is_empty());
    }
}
