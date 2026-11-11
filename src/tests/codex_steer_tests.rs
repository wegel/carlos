//! Keeps Codex steer labels and exact rewind anchors consistent across live and loaded views.

use serde_json::{json, Value};

use super::{
    handle_notification_line, load_history_from_start_or_resume, AppState, MessageKind, Role,
    Selection,
};
use crate::app::layout_state::TranscriptTheme;

fn user(app: &mut AppState, method: &str, turn: &str, id: &str, text: &str) {
    handle_notification_line(app,&json!({"method":method,"params":{"threadId":app.thread_id,
        "turnId":turn,"item":{"type":"userMessage","id":id,"content":[{"type":"text","text":text}]}}}).to_string());
}
fn enabled() -> AppState {
    let mut app = AppState::new("thread".into());
    app.input_history.enable_native();
    app
}
fn snapshot() -> String {
    json!({"result":{"thread":{"id":"thread","turns":[{"id":"shared","status":"completed","items":[
        {"type":"userMessage","id":"first","content":[{"type":"text","text":"same 日本語"}]},
        {"type":"userMessage","id":"steer","content":[{"type":"text","text":"same 日本語"}]}]}]}}})
    .to_string()
}

#[test]
fn live_started_and_completed_events_count_each_exact_user_item_once() {
    let mut app = enabled();
    for method in ["item/started", "item/completed"] {
        user(&mut app, method, "shared", "first", "same 日本語");
    }
    for method in ["item/started", "item/completed", "item/completed"] {
        user(&mut app, method, "shared", "steer", "same 日本語");
    }
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].kind, MessageKind::Plain);
    assert_eq!(app.messages[1].kind, MessageKind::UserSteer);
    assert!(app
        .messages
        .iter()
        .all(|message| message.role == Role::User));
    app.enter_rewind_mode();
    let selected = app
        .input_history
        .selected_native_request()
        .expect("exact anchor");
    assert_eq!(selected.item_id, "steer");
    assert_eq!(selected.position, 1);
    app.navigate_input_history_up();
    assert_eq!(
        app.input_history
            .selected_native_request()
            .expect("first anchor")
            .item_id,
        "first"
    );
}

#[test]
fn busy_first_message_is_normal_and_hidden_earlier_inputs_still_count() {
    let mut app = enabled();
    app.record_turn_started("new-turn");
    user(&mut app, "item/completed", "new-turn", "normal", "normal");
    assert_eq!(app.messages[0].kind, MessageKind::Plain);
    user(&mut app, "item/completed", "other-turn", "empty", "");
    user(
        &mut app,
        "item/completed",
        "other-turn",
        "after-empty",
        "visible",
    );
    assert_eq!(
        app.messages.last().expect("visible").kind,
        MessageKind::UserSteer
    );
}

#[test]
fn replacement_and_repeated_fork_resets_keep_the_ledger_enabled_and_anchors_exact() {
    let mut app = enabled();
    for _ in 0..3 {
        app.reset_for_forked_thread("thread");
        load_history_from_start_or_resume(&mut app, &snapshot()).expect("loaded native users");
        assert!(app.input_history.native_enabled());
        assert_eq!(app.messages[1].kind, MessageKind::UserSteer);
        app.enter_rewind_mode();
        assert_eq!(
            app.input_history
                .selected_native_request()
                .expect("selected")
                .item_id,
            "steer"
        );
    }
}

#[test]
fn other_backends_keep_plain_user_messages_and_existing_grouping() {
    let mut app = AppState::new("thread".into());
    for id in ["first", "second"] {
        user(&mut app, "item/completed", "shared", id, "same");
    }
    assert!(app
        .messages
        .iter()
        .all(|message| message.kind == MessageKind::Plain));
    assert!(!app.input_history.native_enabled());
}

#[test]
fn steer_labels_do_not_change_copy_text_and_count_equals_draw_at_phone_widths() {
    let mut app = enabled();
    load_history_from_start_or_resume(&mut app, &snapshot()).expect("history");
    for theme in [TranscriptTheme::Gutter, TranscriptTheme::Flow] {
        app.layout.active = theme;
        for width in [8, 12, 18, 36, 80, 140] {
            app.ensure_rendered_lines(width, None);
            let count = app.rendered_line_count();
            let lines = app.snapshot_rendered_lines();
            assert_eq!(count, lines.len());
            assert!(lines.iter().all(|line| line.cells <= width));
            assert!(
                lines
                    .iter()
                    .any(|line| line.separator && line.text.contains("steer")),
                "width {width}"
            );
            let copied = app.selected_text(Selection {
                anchor_x: 1,
                anchor_line_idx: 0,
                focus_x: width,
                focus_line_idx: count.saturating_sub(1),
                dragging: false,
            });
            assert!(!copied.contains("steer"));
            assert_eq!(app.messages[1].text, "same 日本語");
            assert!(copied.contains("same"));
        }
    }
}

#[test]
fn edited_draft_does_not_replace_the_original_canonical_text_in_the_request() {
    let mut app = enabled();
    load_history_from_start_or_resume(&mut app, &snapshot()).expect("history");
    app.enter_rewind_mode();
    app.set_input_text("changed draft");
    let selected = app
        .input_history
        .selected_native_request()
        .expect("selected");
    assert_eq!(selected.text, "same 日本語");
    assert_eq!(selected.item_id, "steer");
    let expected: Value = json!({"type":"text","text":selected.text});
    assert!(!expected.to_string().contains("steer"));
}
