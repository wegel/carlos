//! Keep asynchronous edit progress visible and retain drafts across failure orderings.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{json, Value};

use super::codex_question_support::Peer;
use super::{AppState, Role};
use crate::app::claude_edit::{accepted, begin, failed, send_failed, waiting, EditState};
use crate::app::claude_state::ClaudePhase;
use crate::app::notifications::handle_server_message_line;

fn app() -> AppState {
    let mut app = AppState::new("original".into());
    app.claude.enabled = true;
    for index in 0..80 {
        app.append_message(Role::Assistant, format!("Retained row {index}"));
    }
    app
}

fn event(app: &mut AppState, method: &str, generation: u64, params: Value) {
    let mut params = params;
    params["generation"] = json!(generation);
    let _ = handle_server_message_line(app, &json!({"method":method,"params":params}).to_string());
}

#[test]
fn failed_fork_keeps_newer_composer_and_recovers_saved_edit_without_erasing_old_history() {
    let mut app = app();
    begin(&mut app, "saved edit 日本語".into());
    app.set_input_text("newer unsent composer");
    event(
        &mut app,
        "claude/forkFailed",
        1,
        json!({"message":"The exact selected UUID is unavailable"}),
    );
    assert!(app.claude.phase == ClaudePhase::Ready);
    assert_eq!(app.thread_id, "original");
    assert_eq!(app.input_text(), "newer unsent composer");
    assert!(app
        .messages
        .iter()
        .any(|message| message.text == "Retained row 79"));
    assert!(app
        .messages
        .iter()
        .any(|message| message.text.contains("exact selected UUID")));
    assert!(app.viewport.auto_follow_bottom);
    app.clear_input();
    key(&mut app, KeyCode::F(5));
    assert_eq!(app.input_text(), "saved edit 日本語");
}

#[test]
fn closed_helper_cannot_be_revived_by_late_fork_failure_or_old_generation_errors() {
    let mut app = app();
    begin(&mut app, "recover closed edit".into());
    event(
        &mut app,
        "claude/controller",
        1,
        json!({"frame":{"type":"event","event":"closed","reason":"timeout"}}),
    );
    event(
        &mut app,
        "claude/forkFailed",
        1,
        json!({"message":"late timeout"}),
    );
    assert!(app.claude.phase == ClaudePhase::Closed);
    assert_eq!(app.input_text(), "recover closed edit");
    event(
        &mut app,
        "claude/sessionChanged",
        2,
        json!({"sessionId":"branch"}),
    );
    event(
        &mut app,
        "claude/forkFailed",
        1,
        json!({"message":"old branch failure"}),
    );
    assert!(app.claude.phase == ClaudePhase::Starting);
    assert_eq!(app.thread_id, "branch");
    assert!(!app
        .messages
        .iter()
        .any(|message| message.text.contains("old branch failure")));
}

#[test]
fn preparing_edit_ignores_duplicate_enter_and_only_the_matching_accepted_prompt_finishes_it() {
    let mut app = app();
    let peer = Peer::default();
    begin(&mut app, "saved edit".into());
    app.set_input_text("newer composer");
    for _ in 0..3 {
        crate::app::input_events::handle_terminal_event(
            &peer,
            &mut app,
            crossterm::event::Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            super::TerminalSize {
                width: 36,
                height: 12,
            },
        );
    }
    assert_eq!(app.input_text(), "newer composer");
    assert_eq!(
        app.claude.history.edited_prompt.as_deref(),
        Some("saved edit")
    );
    assert!(peer.calls.borrow().is_empty());
    app.claude.history.edit = Some(EditState::Sending {
        text: "saved edit".into(),
        queued: false,
        uuid: None,
    });
    accepted(&mut app, "different-uuid");
    assert!(waiting(&app));
    crate::app::claude_edit::bind_submission(
        &mut app,
        "saved edit",
        crate::backend::PromptOrigin::Human,
        &json!({"turn":{"id":"exact-sdk-uuid"}}),
    );
    accepted(&mut app, "different-uuid");
    assert!(waiting(&app));
    accepted(&mut app, "exact-sdk-uuid");
    assert!(!waiting(&app));
    assert_eq!(app.input_text(), "newer composer");
}

#[test]
fn failed_dequeued_edit_does_not_replace_newer_composer_or_submit_recovery_automatically() {
    let mut app = app();
    app.claude.history.edit = Some(EditState::Sending {
        text: "old edit".into(),
        queued: false,
        uuid: None,
    });
    app.set_input_text("new typing");
    assert!(send_failed(&mut app, "old edit", "prompt rejected"));
    assert_eq!(app.input_text(), "new typing");
    assert!(!waiting(&app));
    app.clear_input();
    crate::app::claude_session_history::load_unsent(&mut app);
    assert_eq!(app.input_text(), "old edit");
}

#[test]
fn phone_draws_progress_and_failure_even_when_an_older_rewind_row_is_selected() {
    let mut app = app();
    app.push_input_history("original prompt");
    app.enter_rewind_mode();
    begin(&mut app, "edited".into());
    for width in [36, 18] {
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 12))
            .expect("terminal");
        terminal
            .draw(|frame| crate::app::render::render_main_view(frame, &mut app))
            .expect("waiting draw");
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains(if width == 18 {
            "Editing"
        } else {
            "Preparing edited branch"
        }));
        failed(&mut app, "source unavailable");
        terminal
            .draw(|frame| crate::app::render::render_main_view(frame, &mut app))
            .expect("failure draw");
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("Edit failed"));
        begin(&mut app, "edited".into());
    }
}

struct EditPeer;
impl crate::backend::BackendClient for EditPeer {
    fn kind(&self) -> crate::backend::BackendKind {
        crate::backend::BackendKind::Claude
    }
    fn call(&self, _: &str, _: Value, _: std::time::Duration) -> anyhow::Result<String> {
        panic!("no native mutation expected")
    }
    fn respond(&self, _: &Value, _: Value) -> anyhow::Result<()> {
        unreachable!()
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> anyhow::Result<()> {
        unreachable!()
    }
    fn take_events_rx(&mut self) -> anyhow::Result<std::sync::mpsc::Receiver<String>> {
        unreachable!()
    }
    fn stop(&mut self) {}
}
fn key(app: &mut AppState, code: KeyCode) {
    crate::app::input_events::handle_terminal_event(
        &EditPeer,
        app,
        crossterm::event::Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        super::TerminalSize {
            width: 36,
            height: 12,
        },
    );
}

#[test]
fn enter_during_initial_startup_does_not_reopen_the_helper_or_submit_an_edit() {
    let mut app = app();
    app.push_input_history("old prompt");
    app.enter_rewind_mode();
    app.set_input_text("edit while starting");
    app.claude.phase = ClaudePhase::Starting;
    key(&mut app, KeyCode::Enter);
    assert!(app.claude.phase == ClaudePhase::Starting);
    assert_eq!(app.input_text(), "edit while starting");
    assert!(app.claude.history.edited_prompt.is_none());
}

#[test]
fn close_recovers_sending_edits_once_whether_the_queue_still_owns_them_or_dispatch_started() {
    for queued in [false, true] {
        let mut app = app();
        app.set_input_text("new composer");
        app.claude.history.edit = Some(EditState::Sending {
            text: "saved outgoing edit".into(),
            queued,
            uuid: None,
        });
        if queued {
            app.queue_turn_input_with_history("saved outgoing edit", true);
        }
        crate::app::claude_notifications::fail_session(&mut app);
        assert_eq!(app.input_text(), "new composer");
        assert!(app.claude.phase == ClaudePhase::Closed);
        app.clear_input();
        key(&mut app, KeyCode::F(5));
        assert_eq!(app.input_text(), "saved outgoing edit");
        app.clear_input();
        key(&mut app, KeyCode::F(5));
        assert!(
            app.input_is_empty(),
            "one edit must have one recovery owner"
        );
    }
}

#[test]
fn branch_start_recovers_old_and_late_humans_and_never_replays_automatic_work_before_the_edit() {
    let mut app = app();
    app.set_input_text("saved edit");
    app.queue_turn_input_with_history("saved edit", true);
    app.ralph_runtime.enqueue_prompt(
        "old automatic".into(),
        false,
        crate::backend::PromptOrigin::Automatic,
    );
    begin(&mut app, "saved edit".into());
    app.set_input_text("newer composer");
    event(
        &mut app,
        "claude/sessionChanged",
        2,
        json!({"sessionId":"branch"}),
    );
    app.queue_turn_input_with_history("late startup human", true);
    event(
        &mut app,
        "claude/controller",
        2,
        json!({"requestMethod":"start","frame":{"type":"reply","ok":true,"result":{"models":[]}}}),
    );
    let saved = app
        .dequeue_turn_input(std::time::Instant::now())
        .expect("saved edit only");
    assert_eq!(saved.text, "saved edit");
    assert!(saved.origin == crate::backend::PromptOrigin::Human);
    assert!(app.dequeue_turn_input(std::time::Instant::now()).is_none());
    assert_eq!(app.input_text(), "newer composer");
    app.clear_input();
    key(&mut app, KeyCode::F(5));
    assert_eq!(
        app.input_text(),
        "saved edit",
        "old identical human is recoverable, not a dispatch match"
    );
    app.clear_input();
    key(&mut app, KeyCode::F(5));
    assert_eq!(app.input_text(), "late startup human");
}

#[test]
fn a_successful_later_send_dismisses_the_failure_badge_but_keeps_saved_drafts() {
    let mut app = app();
    begin(&mut app, "saved failed edit".into());
    app.set_input_text("later composer");
    failed(&mut app, "fork unavailable");
    accepted(&mut app, "later-native-uuid");
    assert!(app.claude.history.edit.is_none());
    assert!(crate::app::claude_session_history::has_unsent(&app));
    app.clear_input();
    key(&mut app, KeyCode::F(5));
    assert_eq!(app.input_text(), "saved failed edit");
}

#[test]
fn a_later_failed_attempt_adds_its_specific_reason_at_the_current_transcript_tail() {
    let mut app = app();
    begin(&mut app, "first edit".into());
    failed(&mut app, "OLD_REASON");
    app.append_message(Role::Assistant, "later normal chat");
    begin(&mut app, "second edit".into());
    failed(&mut app, "NEW_REASON");
    assert!(app
        .messages
        .last()
        .expect("current failure")
        .text
        .contains("NEW_REASON"));
    assert!(app.viewport.auto_follow_bottom);
    assert_eq!(
        app.messages
            .iter()
            .filter(|row| row.text.contains("OLD_REASON"))
            .count(),
        1
    );
}
