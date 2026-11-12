//! Compare submitted edits, recovery notices, and human versus automatic queue order.

use std::time::Instant;

use serde_json::json;

use super::claude_edit::{accepted, bind_submission, dispatching, send_failed, waiting};
use super::claude_edit_parity_support::{advance, app, event, load, notices, Peer};
use crate::backend::{PromptOrigin, PromptPriority};

#[test]
fn edit_failure_at_every_stage_recovers_once_and_never_replaces_newer_typing() {
    for stage in 0..5 {
        for newer in [false, true] {
            let mut app = app();
            advance(&mut app, stage);
            if newer {
                app.set_input_text("new composer");
            }
            super::claude_notifications::fail_session(&mut app);
            assert!(!waiting(&app));
            assert!(app.view.claude_edit.failed);
            if newer {
                assert_eq!(app.input_text(), "new composer");
                let prefix = if stage == 2 { "queued" } else { "edited" };
                assert_eq!(notices(&app), [format!("Unsent {prefix} draft (F5 loads it when the composer is empty):\nedited text")]);
                load(&mut app);
                assert_eq!(app.input_text(), "new composer");
                assert_eq!(
                    notices(&app).last().copied(),
                    Some("Copy or clear the current draft before pressing F5")
                );
                app.clear_input();
                load(&mut app);
            }
            assert_eq!(app.input_text(), "edited text", "stage {stage}");
            app.clear_input();
            super::claude_notifications::fail_session(&mut app);
            load(&mut app);
            assert!(app.input_is_empty());
        }
    }
}

#[test]
fn native_reset_at_every_edit_stage_drops_progress_without_inventing_recovery() {
    for stage in 0..5 {
        let mut app = app();
        advance(&mut app, stage);
        app.set_input_text("new composer");
        event(
            &mut app,
            "claude/conversationReset",
            json!({"new_conversation_id":"reset","user_message_uuid":"clear"}),
        );
        assert!(!waiting(&app));
        assert!(!app.view.claude_edit.failed);
        assert_eq!(app.input_text(), "new composer");
        app.clear_input();
        load(&mut app);
        assert!(app.input_is_empty());
        let queued = app.dequeue_turn_input(Instant::now());
        assert_eq!(
            queued.as_ref().map(|input| input.text.as_str()),
            (stage == 2).then_some("edited text")
        );
        assert!(app.dequeue_turn_input(Instant::now()).is_none());
    }
}

#[test]
fn switch_at_every_stage_carries_only_saved_branch_text_and_retains_queued_humans() {
    for stage in 0..5 {
        let mut app = app();
        advance(&mut app, stage);
        app.set_input_text("new composer");
        super::claude_session_history::switch_session(&mut app, 3, "other");
        assert_eq!(waiting(&app), stage < 2);
        assert_eq!(app.input_text(), "new composer");
        super::claude_models::apply_capabilities(&mut app, &json!({"models":[]}));
        let next = app.dequeue_turn_input(Instant::now());
        assert_eq!(
            next.as_ref().map(|input| input.text.as_str()),
            (stage < 2).then_some("edited text")
        );
        app.clear_input();
        load(&mut app);
        assert_eq!(
            app.input_text(),
            if stage == 2 { "edited text" } else { "" }
        );
    }
}

#[test]
fn bind_requires_matching_dispatched_human_and_uses_nested_then_flat_uuid() {
    for response in [
        json!({"result":{"turn":{"id":"nested"}},"turn":{"id":"flat"}}),
        json!({"result":{"turn":{"id":3}},"turn":{"id":"nested"}}),
    ] {
        let mut app = app();
        advance(&mut app, 2);
        bind_submission(&mut app, "edited text", PromptOrigin::Human, &response);
        accepted(&mut app, "nested");
        assert!(waiting(&app), "queued text cannot bind");
        dispatching(&mut app, "edited text", PromptOrigin::Automatic);
        dispatching(&mut app, "other", PromptOrigin::Human);
        bind_submission(&mut app, "edited text", PromptOrigin::Human, &response);
        accepted(&mut app, "nested");
        assert!(waiting(&app));
        dispatching(&mut app, "edited text", PromptOrigin::Human);
        bind_submission(&mut app, "other", PromptOrigin::Human, &response);
        bind_submission(&mut app, "edited text", PromptOrigin::Automatic, &response);
        accepted(&mut app, "nested");
        assert!(waiting(&app));
        bind_submission(&mut app, "edited text", PromptOrigin::Human, &response);
        accepted(&mut app, "flat");
        assert!(waiting(&app));
        accepted(&mut app, "nested");
        assert!(!waiting(&app));
        assert_eq!(notices(&app), ["Claude started the edited branch"]);
        accepted(&mut app, "nested");
        assert_eq!(notices(&app).len(), 1);
    }
}

#[test]
fn synchronous_send_success_and_failure_keep_the_composer_and_exact_notices() {
    for fail in [false, true] {
        let mut app = app();
        advance(&mut app, 2);
        app.set_input_text("new composer");
        let input = app.dequeue_turn_input(Instant::now()).expect("edit");
        let peer = Peer::default();
        peer.fail.set(fail);
        super::turn_submit::submit_dequeued_turn_text(
            &peer,
            &mut app,
            input.text,
            input.record_input_history,
            input.origin,
        );
        assert_eq!(app.input_text(), "new composer");
        assert!(!waiting(&app));
        assert_eq!(peer.calls.borrow().len(), 1);
        assert_eq!(peer.calls.borrow()[0].0, "edited text");
        if fail {
            assert_eq!(
                notices(&app),
                [
                    "Unsent edited draft (F5 loads it when the composer is empty):\nedited text",
                    "Could not edit Claude message: fixed send failure. Draft kept.",
                    "Could not send message: fixed send failure"
                ]
            );
            app.clear_input();
            load(&mut app);
            assert_eq!(app.input_text(), "edited text");
        } else {
            assert_eq!(
                notices(&app),
                ["edited text", "Claude started the edited branch"]
            );
        }
    }
}

#[test]
fn async_fork_and_start_failures_keep_exact_recovery_and_failure_wording() {
    for (stage, method, reason) in [
        (0, "claude/forkFailed", "Could not fork Claude session"),
        (
            1,
            "claude/sessionStartFailed",
            "Claude could not start the edited branch",
        ),
    ] {
        let mut app = app();
        advance(&mut app, stage);
        app.set_input_text("new composer");
        event(&mut app, method, json!({}));
        assert_eq!(app.input_text(), "new composer");
        assert_eq!(
            notices(&app),
            [
                "Unsent edited draft (F5 loads it when the composer is empty):\nedited text"
                    .to_string(),
                format!("Could not edit Claude message: {reason}. Draft kept.")
            ]
        );
        assert!(!send_failed(&mut app, "unrelated", "ignored"));
    }
}

#[test]
fn identical_composer_needs_no_recovery_notice_and_later_accept_dismisses_failure_only() {
    let mut app = app();
    advance(&mut app, 0);
    app.set_input_text("edited text");
    super::claude_edit::failed(&mut app, "unavailable");
    assert_eq!(
        notices(&app),
        ["Could not edit Claude message: unavailable. Draft kept."]
    );
    assert!(!super::claude_session_history::has_unsent(&app));
    accepted(&mut app, "later");
    assert!(!app.view.claude_edit.failed);
    assert_eq!(app.input_text(), "edited text");
}

#[test]
fn queued_recovery_preserves_human_order_and_discards_automatic_continuations() {
    let mut app = app();
    for (text, origin) in [
        ("first", PromptOrigin::Human),
        ("automatic", PromptOrigin::Automatic),
        ("second", PromptOrigin::Human),
    ] {
        app.session.ralph_enqueue_prompt(text.into(), true, origin);
        app.push_input_history(text);
    }
    app.queue_ralph_continuation("delayed automatic");
    app.set_input_text("new composer");
    super::claude_edit::retire_queue(&mut app);
    assert_eq!(
        notices(&app),
        [
            "Unsent queued draft (F5 loads it when the composer is empty):\nfirst",
            "Unsent queued draft (F5 loads it when the composer is empty):\nsecond"
        ]
    );
    assert!(app.session.queued_turn_inputs_is_empty());
    assert!(!app.has_pending_ralph_continuation());
    assert_eq!(app.input_text(), "new composer");
    for expected in ["first", "second", ""] {
        app.clear_input();
        load(&mut app);
        assert_eq!(app.input_text(), expected);
    }
}

#[test]
fn human_steer_and_queue_choices_reach_backend_without_changing_automatic_source() {
    for queue in [false, true] {
        let mut app = app();
        app.claude.prompts.queue = queue;
        app.session
            .record_claude_activity(&json!({"state":"running"}));
        let peer = Peer::default();
        super::turn_submit::submit_turn_text(&peer, &mut app, "human text".into());
        let call = peer.calls.borrow();
        assert!(call[0].1 == PromptOrigin::Human);
        assert!(
            call[0].2
                == if queue {
                    PromptPriority::Queue
                } else {
                    PromptPriority::Steer
                }
        );
    }
}

#[test]
fn due_ralph_continuations_stay_behind_humans_and_keep_their_actual_sources() {
    let mut app = app();
    app.queue_ralph_continuation("automatic");
    let due = app
        .session
        .ralph_pending_continuation_deadline()
        .expect("deadline");
    for text in ["first human", "second human"] {
        app.queue_turn_input(text);
    }
    for (text, origin) in [
        ("first human", PromptOrigin::Human),
        ("second human", PromptOrigin::Human),
        ("automatic", PromptOrigin::Automatic),
    ] {
        let input = app.dequeue_turn_input(due).expect("ordered prompt");
        assert_eq!(input.text, text);
        assert!(input.origin == origin);
    }
    assert!(app.dequeue_turn_input(due).is_none());
}

#[test]
fn missing_uuid_unbinds_a_dispatched_edit_and_same_text_is_still_the_send_failure_match() {
    let mut app = app();
    advance(&mut app, 4);
    bind_submission(&mut app, "edited text", PromptOrigin::Human, &json!({}));
    accepted(&mut app, "edit-uuid");
    assert!(waiting(&app));
    assert!(!send_failed(&mut app, "other", "wrong text"));
    app.set_input_text("new composer");
    assert!(send_failed(&mut app, "edited text", "fixed failure"));
    assert!(!waiting(&app));
    assert_eq!(app.input_text(), "new composer");
    assert_eq!(
        notices(&app),
        [
            "Unsent edited draft (F5 loads it when the composer is empty):\nedited text",
            "Could not edit Claude message: fixed failure. Draft kept."
        ]
    );
}

#[test]
fn slash_clear_obeys_startup_and_queue_guards_at_every_edit_stage() {
    for stage in 0..5 {
        let mut app = app();
        advance(&mut app, stage);
        app.set_input_text("/clear");
        let peer = Peer::default();
        super::turn_submit::submit_turn_text(&peer, &mut app, "/clear".into());
        assert!(
            waiting(&app),
            "clear submission alone does not settle the edit"
        );
        assert_eq!(peer.calls.borrow().len(), usize::from(stage >= 3));
        assert_eq!(app.input_text(), "/clear");
        if stage >= 3 {
            assert_eq!(app.session.active_turn_id().as_deref(), Some("clear-uuid"));
            assert_eq!(notices(&app), ["/clear"]);
        }
        event(
            &mut app,
            "claude/conversationReset",
            json!({"new_conversation_id":"reset","user_message_uuid":"clear-uuid"}),
        );
        assert!(!waiting(&app));
        assert!(app.session.messages().is_empty());
        let mut queued = Vec::new();
        while let Some(input) = app.dequeue_turn_input(Instant::now()) {
            queued.push(input.text);
        }
        assert_eq!(
            queued,
            match stage {
                0 | 1 => vec!["/clear"],
                2 => vec!["edited text", "/clear"],
                _ => Vec::new(),
            }
        );
    }
}
