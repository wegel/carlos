//! Keep native item IDs exact across turns, display parts and fresh settings catalogs.

use serde_json::json;

use super::codex_question_support::app;
use super::codex_reconnect_support::{ready, receive, snapshot, Peer};
use crate::app::item_history::replace_history_from_thread;
use crate::app::models::Role;
use crate::app::notifications::handle_server_message_line;

fn message(app: &mut crate::app::state::AppState, turn: &str, kind: &str, text: &str) {
    handle_server_message_line(
        app,
        &json!({"method":"item/completed","params":{"threadId":"root","turnId":turn,
        "item":{"id":if kind=="userMessage" {"same-user"} else {"same-answer"},"type":kind,"text":text,"content":[{"type":"text","text":text}]}}})
        .to_string(),
    );
}

#[test]
fn repeated_native_item_and_user_ids_stay_distinct_in_live_history_and_reconnect() {
    let mut app = app();
    app.input_history.enable_native();
    for turn in ["one", "two"] {
        message(&mut app, turn, "userMessage", turn);
        message(&mut app, turn, "agentMessage", &format!("answer-{turn}"));
    }
    assert_eq!(app.messages.len(), 4);
    let history = json!({"id":"root","status":{"type":"idle"},"turns":[
        {"id":"one","status":"completed","items":[{"id":"same-user","type":"userMessage","content":[{"type":"text","text":"identical"}]},{"id":"same-answer","type":"agentMessage","text":"answer-one"}]},
        {"id":"two","status":"completed","items":[{"id":"same-user","type":"userMessage","content":[{"type":"text","text":"identical"}]},{"id":"same-answer","type":"agentMessage","text":"answer-two"}]}]});
    replace_history_from_thread(&mut app, &history);
    assert_eq!(app.messages.len(), 4);
    assert_eq!(app.messages[1].text, "answer-one");
    assert_eq!(app.messages[3].text, "answer-two");
    app.codex.item_turn = Some("outside".into());
    handle_server_message_line(&mut app,&json!({"method":"item/agentMessage/delta","params":{"threadId":"root","turnId":"one","itemId":"same-answer","delta":"LATE_OLD"}}).to_string());
    assert_eq!(app.codex.item_turn.as_deref(), Some("outside"));
    assert_eq!(app.messages[3].text, "answer-two");
    app.codex.item_turn = None;
    app.enter_rewind_mode();
    let selected = app
        .input_history
        .selected_native_request()
        .expect("native user anchor");
    assert_eq!(selected.item_id, "same-user");
    assert_eq!(app.rewind_selected_turn_id(), Some("two"));
    let peer = Peer::default();
    app.codex.closed = true;
    app.codex.connection.generation = 2;
    app.codex.connection.rebuilding = true;
    let mut current = snapshot("unused", "idle");
    current.thread = history;
    ready(&peer, &mut app, current);
    assert_eq!(
        app.messages
            .iter()
            .filter(|row| row.role == Role::Assistant)
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>(),
        ["answer-one", "answer-two"]
    );
}

#[test]
fn native_suffix_and_unicode_ids_cannot_collide_with_display_parts() {
    let mut app = app();
    let item = json!({"type":"toolOutput","id":"x","files":[
        {"path":"a.rs","kind":{"type":"update"},"diff":"@@ -1 +1 @@\n-old-a\n+new-a\n"},
        {"path":"b.rs","kind":{"type":"update"},"diff":"@@ -1 +1 @@\n-old-b\n+new-b\n"}]});
    replace_history_from_thread(
        &mut app,
        &json!({"id":"root","turns":[{"id":"日本語:","status":"completed","items":[
        item,{"id":"x#part-1","type":"agentMessage","text":"NATIVE_SUFFIX"}]}]}),
    );
    assert_eq!(app.messages.len(), 3);
    assert!(app.messages[0].text.contains("new-a"));
    assert!(app.messages[1].text.contains("new-b"));
    assert_eq!(app.messages[2].text, "NATIVE_SUFFIX");
    app.codex.item_turn = Some("a:b".into());
    let first = app.item_key("日本語:c");
    app.codex.item_turn = Some("a".into());
    assert_ne!(first, app.item_key("b:日本語:c"));
    assert_eq!(app.codex.item_part, 0);
}

#[test]
fn fresh_catalog_preserves_editor_values_and_drops_old_direct_override_fallback() {
    let peer = Peer::default();
    let mut app = super::codex_reconnect_support::app();
    let mut first = snapshot("one", "idle");
    first.catalog =
        json!({"data":[{"model":"a","displayName":"A"},{"model":"b","displayName":"B"}]});
    ready(&peer, &mut app, first);
    app.toggle_model_settings();
    app.runtime.model_settings_model_index = 1;
    assert_eq!(app.model_settings_model_value(), "b");
    app.codex.settings.legacy_overrides = true;
    app.codex.closed = true;
    app.codex.connection.rebuilding = true;
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
    assert_eq!(app.runtime.current_model.as_deref(), Some("a"));
    assert!(!app.codex.settings.legacy_overrides);
    assert!(app.runtime.pending_model.is_none());
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
    assert!(app.thread_name.is_none());
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
    assert_eq!(app.thread_name.as_deref(), Some("newer 日本語"));
    assert_eq!(app.runtime.current_model.as_deref(), Some("fresh-model"));
    assert!(app.codex.connection.runtime_refresh);
}

#[test]
fn a_reused_process_id_with_a_different_native_item_never_retargets_stop() {
    let mut app = app();
    app.codex
        .panel
        .processes
        .push(crate::app::codex_panel_state::Process {
            thread: "child".into(),
            data: json!({"processId":"7","itemId":"old-item"}),
        });
    app.codex.panel.selected = app.codex.panel.ids().into_iter().next();
    app.codex.panel.processes[0].data["itemId"] = json!("new-item");
    assert!(!app.codex.panel.selected_exists());
    let peer = super::codex_question_support::Peer::default();
    crate::app::codex_panel::stop(&mut app);
    crate::app::codex_panel::tick(&peer, &mut app);
    assert!(peer.controls.borrow().is_empty());
    app.codex.panel.select(true);
    assert!(app.codex.panel.selected_exists());
}

#[test]
fn socket_option_requires_codex_conversations_and_resume_hints_quote_exact_paths() {
    use crate::app::cli::{parse_cli_args, resume_hint_with_socket, Backend};
    use std::path::Path;
    let args = |values: &[&str]| {
        values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
    };
    let parsed = parse_cli_args(args(&[
        "--backend",
        "codex",
        "--codex-socket",
        "socket with spaces",
        "resume",
        "root",
    ]))
    .expect("explicit socket");
    assert!(parsed.codex_socket.expect("socket").is_absolute());
    assert!(parse_cli_args(args(&["--codex-socket", "/tmp/rpc", "--backend", "claude"])).is_err());
    assert!(parse_cli_args(args(&[
        "perf-session",
        "--synthetic",
        "--codex-socket",
        "/tmp/rpc"
    ]))
    .is_err());
    let hint = resume_hint_with_socket(
        Backend::Codex,
        "root' id",
        Some(Path::new("/tmp/a' socket")),
    );
    let words = shlex::split(hint.lines().nth(1).expect("command")).expect("shell-safe hint");
    assert_eq!(
        words,
        [
            "carlos",
            "--backend",
            "codex",
            "--codex-socket",
            "/tmp/a' socket",
            "resume",
            "root' id"
        ]
    );
}
