//! Session titles remain display-only and follow the current thread's metadata.

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Modifier;
use ratatui::Terminal;
use serde_json::json;

use super::{
    handle_notification_line, load_history_from_start_or_resume, render_main_view, AppState,
};
use crate::theme::COLOR_STEP8;

fn render(app: &mut AppState, width: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, 16)).expect("terminal");
    terminal
        .draw(|frame| render_main_view(frame, app))
        .expect("draw title");
    terminal.backend().buffer().clone()
}

#[test]
fn name_placeholder_does_not_change_input_layout_cursor_or_history() {
    let mut app = AppState::new("root".into());
    app.set_thread_name(Some("Cleanup monitor"));
    let buffer = render(&mut app, 80);
    assert_eq!(buffer[(2, 15)].symbol(), " ");
    assert_eq!(buffer[(64, 15)].symbol(), "C");
    assert_eq!(buffer[(78, 15)].symbol(), "r");
    assert_eq!(buffer[(79, 15)].symbol(), " ");
    assert_eq!(buffer[(64, 15)].fg, COLOR_STEP8);
    assert!(!buffer[(64, 15)].modifier.contains(Modifier::DIM));
    assert_eq!(app.input_text(), "");
    assert_eq!(app.input.cursor(), (0, 0));
    assert_eq!(app.submitted_turn_count(), 0);
    assert!(app.messages.is_empty());
    app.set_input_text("x");
    let buffer = render(&mut app, 80);
    assert_eq!(buffer[(1, 15)].symbol(), "x");
    assert!(!buffer[(2, 15)].modifier.contains(Modifier::DIM));
}

#[test]
fn right_aligned_names_clip_at_whole_unicode_characters_when_resizing() {
    let mut app = AppState::new("root".into());
    app.set_thread_name(Some("漢🦀x"));
    let buffer = render(&mut app, 20);
    assert_eq!(buffer[(14, 15)].symbol(), "漢");
    assert_eq!(buffer[(16, 15)].symbol(), "🦀");
    assert_eq!(buffer[(18, 15)].symbol(), "x");
    assert_eq!(buffer[(19, 15)].symbol(), " ");
    let buffer = render(&mut app, 7);
    assert_eq!(buffer[(1, 15)].symbol(), "漢");
    assert_eq!(buffer[(3, 15)].symbol(), "🦀");
    assert_eq!(buffer[(5, 15)].symbol(), "x");
    assert_eq!(buffer[(6, 15)].symbol(), " ");
    let buffer = render(&mut app, 6);
    assert_eq!(buffer[(1, 15)].symbol(), "漢");
    assert_eq!(buffer[(3, 15)].symbol(), "🦀");
    assert_eq!(buffer[(5, 15)].symbol(), " ");
    let buffer = render(&mut app, 4);
    assert_eq!(buffer[(1, 15)].symbol(), "漢");
    assert_eq!(buffer[(3, 15)].symbol(), " ");
    assert!(app.input_is_empty());
}

#[test]
fn titles_load_from_history_and_clear_only_when_metadata_says_so() {
    let mut app = AppState::new("root".into());
    let mut response = json!({"result":{"thread":{"id":"root", "name":"Saved title", "turns":[]}}});
    load_history_from_start_or_resume(&mut app, &response.to_string()).expect("load named thread");
    assert_eq!(app.thread_name.as_deref(), Some("Saved title"));
    response["result"]["thread"]
        .as_object_mut()
        .expect("thread")
        .remove("name");
    load_history_from_start_or_resume(&mut app, &response.to_string())
        .expect("load transcript page");
    assert_eq!(app.thread_name.as_deref(), Some("Saved title"));
    response["result"]["thread"]["name"] = json!(null);
    load_history_from_start_or_resume(&mut app, &response.to_string()).expect("load cleared title");
    assert_eq!(app.thread_name, None);
}

#[test]
fn foreign_name_events_cannot_change_the_title_or_draft() {
    let mut app = AppState::new("root".into());
    app.set_thread_name(Some("Original"));
    app.set_input_text("keep my draft");
    for thread_id in ["other", "root"] {
        handle_notification_line(
            &mut app,
            &json!({"method":"thread/name/updated", "params":{
            "threadId":thread_id, "threadName":"External rename"}})
            .to_string(),
        );
        assert_eq!(
            app.thread_name.as_deref(),
            Some(if thread_id == "other" {
                "Original"
            } else {
                "External rename"
            })
        );
        assert_eq!(app.input_text(), "keep my draft");
    }
    handle_notification_line(
        &mut app,
        &json!({"method":"thread/name/updated", "params":{
        "threadId":"root", "threadName":null}})
        .to_string(),
    );
    assert_eq!(app.thread_name, None);
}

#[test]
fn switching_or_forking_threads_clears_the_old_title() {
    let mut app = AppState::new("root".into());
    app.set_thread_name(Some("Original"));
    app.set_thread_id("root");
    assert_eq!(app.thread_name.as_deref(), Some("Original"));
    app.set_thread_id("other");
    assert_eq!(app.thread_name, None);
    app.set_thread_name(Some("Other title"));
    app.reset_for_forked_thread("fork");
    assert_eq!(app.thread_name, None);
}

#[test]
fn unicode_and_external_control_characters_cannot_expand_the_composer() {
    let mut app = AppState::new("root".into());
    app.set_thread_name(Some("Résumé 漢字 🦀\n\u{1b}[31msecond line\t"));
    assert!(!app
        .thread_name
        .as_deref()
        .expect("display name")
        .chars()
        .any(char::is_control));
    for width in [1, 3, 12, 80] {
        let _ = render(&mut app, width);
        assert_eq!(app.input_text(), "");
    }
}
