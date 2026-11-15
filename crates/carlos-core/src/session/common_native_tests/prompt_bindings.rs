//! Keep prompt links exact when an owner truncates rows or binds a pending echo.

use super::identity_support::truncate;
use super::session;
use crate::session::models::Role;

#[test]
fn rewind_fork_drops_selected_message_and_newer_history() {
    let mut app = session("thread-1");
    let u1 = app.append_message(Role::User, "first");
    app.record_prompt_history("first", Some(app.messages()[u1].id));
    app.append_message(Role::Assistant, "reply one");
    let u2 = app.append_message(Role::User, "second");
    app.record_prompt_history("second", Some(app.messages()[u2].id));
    app.append_message(Role::Assistant, "reply two");
    truncate(&mut app, u2);
    assert_eq!(app.messages().len(), u2);
    assert!(app.messages().iter().all(|row| row.text != "second"));
    assert!(app.messages().iter().all(|row| row.text != "reply two"));
    let positions: Vec<_> = app
        .input_history()
        .message_ids()
        .iter()
        .map(|id| id.and_then(|id| app.messages().position(id)))
        .collect();
    assert_eq!(positions, [Some(u1), None]);
}

#[test]
fn record_input_history_backfills_pending_message_index() {
    let mut app = session("thread-1");
    app.record_prompt_history("prompt", None);
    assert_eq!(app.input_history().message_ids(), &[None]);
    let prompt = app.input_history().prompt_id(0);
    for _ in 0..42 {
        app.append_message(Role::System, "earlier");
    }
    let index = app.append_message(Role::User, "prompt");
    let id = app.messages()[index].id;
    app.record_prompt_history("prompt", Some(id));
    assert_eq!(app.input_history().history_len(), 1);
    assert_eq!(app.input_history().prompt_id(0), prompt);
    assert_eq!(app.input_history().message_ids(), &[Some(id)]);
    assert_eq!(app.messages().position(id), Some(42));
}
