//! Keep automatic-work markers, toggles and human queue order in the session owner.

use crate::session::models::Role;
use crate::session::state::SessionState;

#[test]
fn ralph_turn_completion_queues_continuation_when_not_blocked_or_complete() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.enable_ralph_mode(crate::session::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        waiting_marker: "@@AWAITING_HUMAN@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.set_turn_start_for_test(Some(0));
    app.append_message(Role::Assistant, "still working");

    app.handle_ralph_turn_completed(false);

    assert!(app.has_pending_ralph_continuation());
    assert!(app.dequeue_turn_input(std::time::Instant::now()).is_none());
}

#[test]
fn ralph_turn_completion_disables_ralph_on_human_gate() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.enable_ralph_mode(crate::session::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        waiting_marker: "@@AWAITING_HUMAN@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.set_turn_start_for_test(Some(0));
    app.append_message(Role::Assistant, "@@AWAITING_HUMAN@@");

    app.handle_ralph_turn_completed(false);

    assert!(!app.ralph_enabled());
    assert!(app.queued_turn_inputs_is_empty());
    assert!(
        app.messages().last().is_some_and(
            |msg| msg.role == Role::System && msg.text == "Ralph waiting for human input"
        )
    );
}

#[test]
fn ralph_turn_completion_disables_ralph_mode_on_blocked_marker() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.enable_ralph_mode(crate::session::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        waiting_marker: "@@AWAITING_HUMAN@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.set_turn_start_for_test(Some(0));
    app.append_message(Role::Assistant, "@@BLOCKED@@");

    app.handle_ralph_turn_completed(false);

    assert!(app.queued_turn_inputs_is_empty());
    assert!(!app.ralph_enabled());
}

#[test]
fn commentary_blocked_marker_disables_ralph_mode_immediately() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.enable_ralph_mode(crate::session::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        waiting_marker: "@@AWAITING_HUMAN@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.set_turn_start_for_test(Some(0));
    app.append_message(Role::Commentary, "@@BLOCKED@@");

    app.maybe_disable_ralph_on_blocked_marker();

    assert!(!app.ralph_enabled());
    assert!(app.messages().last().is_some_and(
        |msg| msg.role == Role::System && msg.text == "Ralph blocked: waiting for input"
    ));
}

#[test]
fn ralph_turn_completion_disables_ralph_mode() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.enable_ralph_mode(crate::session::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        waiting_marker: "@@AWAITING_HUMAN@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.set_turn_start_for_test(Some(0));
    app.append_message(
        Role::Assistant,
        "@@COMPLETE@@ @@COMPLETE_SUMMARY_START@@".to_string(),
    );

    app.handle_ralph_turn_completed(false);

    assert!(!app.ralph_enabled());
    assert!(app.queued_turn_inputs_is_empty());
}

#[test]
fn ralph_interrupted_turn_does_not_queue_continuation() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.enable_ralph_mode(crate::session::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        waiting_marker: "@@AWAITING_HUMAN@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.set_turn_start_for_test(Some(0));
    app.append_message(Role::Assistant, "still working");

    app.handle_ralph_turn_completed(true);

    assert!(app.queued_turn_inputs_is_empty());
    assert!(app.ralph_enabled());
    assert!(!app.ralph_waiting_for_user());
}

#[test]
fn request_ralph_toggle_enables_and_disables_when_idle() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    let tmp = std::env::temp_dir().join(format!(
        "carlos-ralph-toggle-{}.md",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos()
    ));
    std::fs::write(&tmp, "auto prompt").expect("write prompt");
    app.configure_ralph_options(
        std::env::current_dir().expect("cwd"),
        Some(tmp.to_string_lossy().to_string()),
        None,
        None,
        None,
    );

    app.request_ralph_toggle().expect("toggle on");
    assert!(app.ralph_enabled());
    assert!(app.dequeue_turn_input(std::time::Instant::now()).is_some());

    app.request_ralph_toggle().expect("toggle off");
    assert!(!app.ralph_enabled());
    assert!(app.queued_turn_inputs_is_empty());

    let _ = std::fs::remove_file(tmp);
}

#[test]
fn request_ralph_toggle_defers_while_turn_active() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.set_active_turn_id(Some("turn-1".to_string()));

    app.request_ralph_toggle().expect("defer");
    assert!(app.ralph_toggle_pending());

    app.request_ralph_toggle().expect("cancel");
    assert!(!app.ralph_toggle_pending());
}

#[test]
fn pending_ralph_continuation_becomes_ready_after_deadline() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.queue_ralph_continuation("continue");
    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");

    assert!(app
        .dequeue_turn_input(deadline - std::time::Duration::from_millis(1))
        .is_none());
    let queued = app
        .dequeue_turn_input(deadline)
        .expect("queued continuation");
    assert_eq!(queued.text, "continue");
    assert!(!queued.record_input_history);
    assert!(!app.has_pending_ralph_continuation());
}

#[test]
fn queued_user_turns_record_history_but_ralph_turns_do_not() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.queue_turn_input("follow up");
    let user_turn = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("queued user turn");
    assert_eq!(user_turn.text, "follow up");
    assert!(user_turn.record_input_history);

    app.queue_ralph_continuation("continue");
    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");
    let ralph_turn = app.dequeue_turn_input(deadline).expect("queued Ralph turn");
    assert_eq!(ralph_turn.text, "continue");
    assert!(!ralph_turn.record_input_history);
}

#[test]
fn delayed_ralph_continuation_does_not_block_ready_user_turns() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.queue_turn_input("first");
    app.queue_ralph_continuation("continue");
    app.queue_turn_input("second");

    let first = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("first queued turn");
    assert_eq!(first.text, "first");

    let second = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("second queued turn");
    assert_eq!(second.text, "second");

    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");
    let continuation = app
        .dequeue_turn_input(deadline)
        .expect("queued continuation");
    assert_eq!(continuation.text, "continue");
}

#[test]
fn ready_ralph_continuation_stays_ahead_of_later_user_turns() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.queue_ralph_continuation("continue");
    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");

    app.promote_ready_continuation(deadline);
    app.queue_turn_input("later");

    let continuation = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("continuation");
    assert_eq!(continuation.text, "continue");
    let later = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("later user turn");
    assert_eq!(later.text, "later");
}

#[test]
fn promoted_ralph_continuation_still_counts_as_pending_until_sent() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.queue_ralph_continuation("continue");
    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");

    app.promote_ready_continuation(deadline);
    assert!(app.has_pending_ralph_continuation());

    let continuation = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("continuation");
    assert_eq!(continuation.text, "continue");
    assert!(!app.has_pending_ralph_continuation());
}
