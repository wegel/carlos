//! Keep native item IDs exact across turns, display parts and fresh settings catalogs.

use serde_json::json;

use super::codex_question_support::app;
use super::codex_reconnect_support::{ready, receive, snapshot, Peer};

#[test]
fn fresh_catalog_preserves_editor_values_and_drops_old_direct_override_fallback() {
    let peer = Peer::default();
    let mut app = super::codex_reconnect_support::app();
    let mut first = snapshot("one", "idle");
    first.catalog =
        json!({"data":[{"model":"a","displayName":"A"},{"model":"b","displayName":"B"}]});
    ready(&peer, &mut app, first);
    app.toggle_model_settings();
    app.view.runtime_settings.model_settings_model_index = 1;
    assert_eq!(app.model_settings_model_value(), "b");
    app.session.set_codex_settings_legacy_for_test(true);
    app.session.close_codex();
    app.session.set_codex_rebuilding_for_test(true);
    let mut second = snapshot("two", "idle");
    second.runtime =
        json!({"result":{"thread":{"id":"root"},"model":"a","reasoningEffort":"low"}}).to_string();
    second.catalog =
        json!({"data":[{"model":"b","displayName":"B"},{"model":"a","displayName":"A"}]});
    receive(
        &peer,
        &mut app,
        2,
        json!({"method":"thread/settings/updated","params":{"threadId":"root","threadSettings":{"model":"OLD_BUFFERED"}}}),
    );
    ready(&peer, &mut app, second);
    assert_eq!(app.model_settings_model_value(), "b");
    assert_eq!(
        app.session.runtime_settings().current_model().as_deref(),
        Some("a")
    );
    assert!(!app.session.codex_settings().legacy_overrides());
    assert!(app.session.runtime_settings().pending_model().is_none());
}

#[test]
fn fresh_metadata_clears_old_names_and_a_newer_rename_fences_only_the_delayed_name() {
    let peer = Peer::default();
    let mut app = super::codex_reconnect_support::app();
    app.set_thread_name(Some("old"));
    let mut fresh = snapshot("one", "idle");
    fresh.thread["name"] = json!(null);
    receive(
        &peer,
        &mut app,
        2,
        json!({"method":"thread/name/updated","params":{"threadId":"root","threadName":"BUFFERED_OLD"}}),
    );
    ready(&peer, &mut app, fresh);
    assert!(app.session.thread_name().is_none());
    crate::app::codex_reconnect_runtime::tick(&peer, &mut app);
    let context = peer.native.contexts.borrow()[0].clone();
    receive(
        &peer,
        &mut app,
        2,
        json!({"method":"thread/name/updated","params":{"threadId":"root","threadName":"newer 日本語"}}),
    );
    receive(
        &peer,
        &mut app,
        2,
        json!({"method":"carlos/codex/controlReply","params":{
        "requestContext":context,"requestParams":{"threadId":"root"},"response":{"result":{"thread":{
            "id":"root","name":"STALE_NAME","model":"fresh-model","reasoningEffort":"low"}}}}}),
    );
    assert_eq!(app.session.thread_name().as_deref(), Some("newer 日本語"));
    assert_eq!(
        app.session.runtime_settings().current_model().as_deref(),
        Some("fresh-model")
    );
    assert!(app.session.codex_connection().runtime_refresh());
}

#[test]
fn a_reused_process_id_with_a_different_native_item_never_retargets_stop() {
    let mut app = app();
    app.session.push_codex_process_for_test(
        carlos_core::session::codex::panel_state::Process::new(
            "child".into(),
            json!({"processId":"7","itemId":"old-item"}),
        ),
    );
    app.view.codex_panel.selected = app.session.codex_panel().ids().into_iter().next();
    app.session
        .set_codex_process_item_for_test(0, json!("new-item"));
    assert!(!app
        .session
        .codex_panel()
        .contains_target(app.view.codex_panel.selected.as_ref()));
    let peer = super::codex_question_support::Peer::default();
    crate::app::codex_panel::stop(&mut app);
    crate::app::codex_panel::tick(&peer, &mut app);
    assert!(peer.controls.borrow().is_empty());
    crate::app::codex_panel::select(&mut app, true);
    assert!(app
        .session
        .codex_panel()
        .contains_target(app.view.codex_panel.selected.as_ref()));
}
