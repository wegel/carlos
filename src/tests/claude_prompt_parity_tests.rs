//! Enumerate accepted, rejected, consumed, and reset prompt orders through the app bridge.

use serde_json::{json, Value};

use super::claude_prompts::{consume, reset};

use super::state::AppState;
use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;

fn app() -> AppState {
    let mut app = AppState::new("session".into());
    app.session.set_claude_enabled_for_test(true);
    app
}

fn fact(app: &mut AppState, method: &str, params: Value) {
    let _ = super::notifications::handle_server_message_line(
        app,
        &json!({"method":method,"params":params}).to_string(),
    );
}

fn accepted(app: &mut AppState, id: &str, text: &str, origin: &str) {
    fact(
        app,
        "claude/promptSubmitted",
        json!({"uuid":id,"text":text,"origin":origin}),
    );
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
        let mut app = app();
        for step in order {
            match step {
                0 => accepted(&mut app, "a", "human text", "human"),
                1 => fact(&mut app, "claude/promptRejected", json!({"uuid":"a"})),
                _ => consume(&mut app, &["a".into()]),
            }
        }
        assert_eq!(app.session.messages().len(), rows, "{order:?}");
        assert_eq!(
            app.session.claude_prompts().next().is_some(),
            pending,
            "{order:?}"
        );
        if rows == 1 {
            assert_eq!(app.session.messages()[0].text, "human text");
            assert_eq!(app.rewind_selected_turn_id(), None);
            assert_eq!(app.session.input_history().text(0), "human text");
        }
    }
}

#[test]
fn duplicate_accept_and_consume_keep_first_text_author_and_priority() {
    let mut app = app();
    accepted(&mut app, "a", "first text", "human");
    fact(
        &mut app,
        "claude/promptSubmitted",
        json!({"uuid":"a","text":"replacement","origin":"automatic","priority":"now"}),
    );
    consume(&mut app, &["missing".into(), "a".into(), "a".into()]);
    let (_, prompt) = app.session.claude_prompts().next().expect("accepted");
    assert_eq!(
        (
            prompt.text(),
            prompt.human(),
            prompt.priority(),
            prompt.consumed()
        ),
        ("first text", true, "later", true)
    );
    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.session.messages()[0].role, Role::User);
    assert_eq!(app.submitted_turn_count(), 1);
}

#[test]
fn malformed_accepts_do_nothing_and_unknown_origin_defaults_to_automatic() {
    let mut app = app();
    for params in [
        json!({}),
        json!({"uuid":"a"}),
        json!({"uuid":3,"text":"bad"}),
        json!({"uuid":"a","text":null}),
    ] {
        app.session
            .register_claude_prompt(&params)
            .expect("malformed optional input is ignored");
    }
    assert!(app.session.claude_prompts().next().is_none());
    accepted(&mut app, "", "unknown source", "unknown");
    consume(&mut app, &["".into()]);
    assert_eq!(
        app.session.messages()[0].text,
        "Automatic prompt: unknown source"
    );
    assert_eq!(app.session.messages()[0].role, Role::System);
    assert_eq!(app.submitted_turn_count(), 0);
}

#[test]
fn consumed_order_wins_over_uuid_order_and_preserves_steering_and_automatic_roles() {
    let mut app = app();
    app.record_turn_started("running");
    for (id, origin) in [("z", "human"), ("a", "automatic"), ("m", "human")] {
        fact(
            &mut app,
            "claude/promptSubmitted",
            json!({"uuid":id,"text":id,"origin":origin,"inputKind":"steer","priority":"now"}),
        );
    }
    assert!(app.session.messages().is_empty());
    consume(&mut app, &["m".into(), "a".into(), "z".into()]);
    let rows: Vec<_> = app
        .session
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
    assert_eq!(app.session.active_turn_id().as_deref(), Some("z"));
    assert_eq!(app.submitted_turn_count(), 2);
}

#[test]
fn reset_retains_unconsumed_and_exact_clear_without_recreating_clear_text() {
    let mut app = app();
    for (id, text) in [
        ("old", "old prompt"),
        ("clear", "/clear"),
        ("next", "next prompt"),
    ] {
        accepted(&mut app, id, text, "human");
    }
    consume(&mut app, &["old".into(), "clear".into()]);
    reset(&mut app, Some("clear"));
    app.reset_preserving_approvals("session");
    consume(&mut app, &["clear".into(), "next".into()]);
    assert_eq!(
        app.session
            .claude_prompts()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        ["clear", "next"]
    );
    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.session.messages()[0].text, "next prompt");
    assert_eq!(app.submitted_turn_count(), 1);
    reset(&mut app, None);
    assert!(app.session.claude_prompts().next().is_none());
}

#[test]
fn local_echo_is_last_row_only_and_submitted_uuid_respects_active_turn() {
    for (active, expected) in [
        (None, "accepted"),
        (Some("claude-turn-pending"), "accepted"),
        (Some("other"), "other"),
    ] {
        let mut app = app();
        if let Some(id) = active {
            app.record_turn_started(id);
        }
        app.append_message(Role::User, "human");
        super::claude_notifications::record_submitted_prompt(
            &mut app,
            &json!({"result":{"turn":{"id":"accepted"},"inputKind":"steer"}}),
        );
        assert_eq!(app.session.active_turn_id().as_deref(), Some(expected));
        assert_eq!(app.session.messages()[0].kind, MessageKind::UserSteer);
        assert_eq!(app.item_message_index("claude-prompt:accepted"), Some(0));
    }
    let mut app = app();
    super::claude_notifications::record_submitted_prompt(
        &mut app,
        &json!({"turn":{"id":"ignored"}}),
    );
    assert!(app.session.active_turn_id().is_none());
    super::claude_prompts::attach_echo(&mut app, "empty", Some("steer"));
    app.append_message(Role::System, "last system");
    super::claude_prompts::attach_echo(&mut app, "system", Some("steer"));
    assert_eq!(app.session.messages()[0].kind, MessageKind::Plain);
    assert_eq!(app.item_message_index("claude-prompt:system"), Some(0));
}

#[test]
fn accepted_registry_bounds_reject_new_entries_but_allow_exact_duplicates() {
    for (count, text) in [(1024, "x".to_string()), (1, "x".repeat(8 * 1024 * 1024))] {
        let mut app = app();
        for index in 0..count {
            app.session
                .register_claude_prompt(&json!({"uuid":index.to_string(),"text":text}))
                .expect("within bounds");
        }
        app.session
            .register_claude_prompt(&json!({"uuid":"0","text":"duplicate"}))
            .expect("duplicate skips bounds");
        assert_eq!(
            app.session
                .register_claude_prompt(&json!({"uuid":"extra","text":"x"})),
            Err("Claude accepted-input history exceeded its bounded queue")
        );
        assert_eq!(app.session.claude_prompts().count(), count);
    }
}

#[test]
fn helper_failure_fails_registered_and_unregistered_active_turns_without_new_rows() {
    for active in ["a", "unregistered"] {
        let mut app = app();
        for id in ["z", "a"] {
            accepted(&mut app, id, id, "human");
        }
        app.record_turn_started(active);
        super::claude_notifications::fail_session(&mut app);
        assert_eq!(app.session.status(), "turn failed");
        assert!(app.session.active_turn_id().is_none());
        assert!(app.session.claude_prompts().next().is_none());
        for id in ["z", "a", active] {
            assert_eq!(
                app.session.turn_status_by_id().get(id),
                Some(&carlos_core::session::models::TurnStatus::Failed)
            );
        }
        assert!(app.session.messages().is_empty());
    }
}
