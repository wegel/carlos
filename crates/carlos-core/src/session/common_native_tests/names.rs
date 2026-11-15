//! Keep native title metadata, thread identity and safe published names inside the core reducer.

use serde_json::json;

use super::{feed, session};
use crate::session::item_history::load_history_from_start_or_resume;

#[test]
fn titles_load_from_history_and_clear_only_when_metadata_says_so() {
    let mut app = session("root");
    let mut response = json!({"result":{"thread":{"id":"root", "name":"Saved title", "turns":[]}}});
    load_history_from_start_or_resume(&mut app, &response.to_string()).expect("load named thread");
    assert_eq!(app.thread_name().as_deref(), Some("Saved title"));
    response["result"]["thread"]
        .as_object_mut()
        .expect("thread")
        .remove("name");
    load_history_from_start_or_resume(&mut app, &response.to_string())
        .expect("load transcript page");
    assert_eq!(app.thread_name().as_deref(), Some("Saved title"));
    response["result"]["thread"]["name"] = json!(null);
    load_history_from_start_or_resume(&mut app, &response.to_string()).expect("load cleared title");
    assert_eq!(*app.thread_name(), None);
}

#[test]
fn foreign_native_name_events_cannot_change_the_current_title() {
    let mut app = session("root");
    app.enable_codex();
    app.observe_name(Some("Original"));
    for thread_id in ["other", "root"] {
        feed(
            &mut app,
            &json!({"method":"thread/name/updated", "params":{
            "threadId":thread_id,"threadName":"External rename"}})
            .to_string(),
        );
        assert_eq!(
            app.thread_name().as_deref(),
            Some(if thread_id == "other" {
                "Original"
            } else {
                "External rename"
            })
        );
    }
    feed(
        &mut app,
        &json!({"method":"thread/name/updated","params":{
        "threadId":"root","threadName":null}})
        .to_string(),
    );
    assert_eq!(*app.thread_name(), None);
}

#[test]
fn switching_or_forking_threads_clears_the_old_title() {
    let mut app = session("root");
    app.enable_codex();
    app.observe_name(Some("Original"));
    app.adopt_native_thread("root".into());
    assert_eq!(app.thread_name().as_deref(), Some("Original"));
    app.adopt_native_thread("other".into());
    assert_eq!(*app.thread_name(), None);
    app.observe_name(Some("Other title"));
    app.adopt_codex_fork(&json!({"result":{"thread":{"id":"fork","turns":[]}}}).to_string())
        .expect("checked native fork");
    assert_eq!(*app.thread_name(), None);
}

#[test]
fn external_controls_are_removed_before_a_unicode_title_is_published() {
    let mut app = session("root");
    app.observe_thread_name(&json!({"id":"root","name":"Résumé 漢字 🦀\n\u{1b}[31msecond line\t"}));
    let title = app.thread_name().as_deref().expect("display name");
    assert!(!title.chars().any(char::is_control));
    assert_eq!(title, "Résumé 漢字 🦀 second line");
    assert_eq!(app.input_history().history_len(), 0);
    assert!(app.messages().is_empty());
    assert_eq!(
        app.publish().snapshot(1).info.name.as_deref(),
        Some("Résumé 漢字 🦀 second line")
    );
}
