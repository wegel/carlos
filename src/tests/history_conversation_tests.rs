//! Reject foreign history before it can replace the current conversation's rows or keys.

use carlos_core::backend::BackendKind;
use carlos_core::grok_backend::{translate_grok_notification_for_test, GrokTranslationState};
use serde_json::{json, Value};

use super::entry_identity_tests::app;
use crate::app::item_history::load_history_from_start_or_resume;
use crate::app::notifications::handle_server_message_line;
use crate::app::state::AppState;

fn foreign_history() -> String {
    json!({"result":{"thread":{"id":"other","name":"Foreign","turns":[
        {"id":"turn","items":[{"id":"x","type":"toolCall","name":"Read",
            "input":{"file_path":"a.rs"}}]}]}}})
    .to_string()
}

fn read(app: &mut AppState, backend: BackendKind, path: &str) {
    if backend == BackendKind::Grok {
        let output = translate_grok_notification_for_test(
            &mut GrokTranslationState::default(),
            "session/update",
            json!({"update":{"sessionUpdate":"tool_call","toolCallId":"x",
                "title":"read_file","kind":"read","rawInput":{"path":path}}}),
        );
        for line in output.lines {
            handle_server_message_line(app, &line);
        }
    } else {
        let source = (backend == BackendKind::Claude)
            .then(|| json!({"kind":"tool","id":"native-x","part":"call"}));
        let event = json!({"method":"item/completed","params":{"turnId":"turn",
            "sourceKey":source,"item":{"id":"x","type":"toolCall","name":"Read",
                "input":{"file_path":path}}}});
        handle_server_message_line(app, &event.to_string());
    }
}

fn check_foreign_history(backend: BackendKind) {
    for existing in [false, true] {
        let mut app = app(backend);
        app.session.set_summary_enabled_for_test(false);
        if existing {
            read(&mut app, backend, "old.rs");
            app.set_thread_name(Some("Current"));
            app.record_turn_started("active");
        }
        let session = format!("{:?}", app.session);
        let turn = app.session.codex_item_turn().clone();
        let ids: Vec<_> = app.session.messages().iter().map(|row| row.id).collect();
        let error = load_history_from_start_or_resume(&mut app, &foreign_history())
            .expect_err("foreign history must be rejected");
        assert_eq!(error.to_string(), "History belongs to another thread");
        assert_eq!(format!("{:?}", app.session), session);
        assert_eq!(app.session.codex_item_turn(), &turn);
        read(&mut app, backend, "b.rs");
        assert_eq!(app.session.messages().len(), 1);
        let expected = if backend == BackendKind::Grok {
            "◇ Read File b.rs"
        } else {
            "→ Read b.rs"
        };
        assert_eq!(app.session.messages()[0].text, expected);
        if existing {
            assert_eq!(app.session.messages()[0].id, ids[0]);
        }
        app.assert_entry_invariants();
    }
}

#[test]
fn grok_foreign_history_then_live_read_keeps_the_current_conversation() {
    check_foreign_history(BackendKind::Grok);
}

#[test]
fn codex_foreign_history_then_live_read_keeps_the_current_conversation() {
    check_foreign_history(BackendKind::Codex);
}

#[test]
fn claude_foreign_history_then_live_read_keeps_the_current_conversation() {
    check_foreign_history(BackendKind::Claude);
}

#[test]
fn omitted_history_thread_id_still_means_the_current_conversation() {
    let mut app = app(BackendKind::Codex);
    let response: Value = json!({"result":{"thread":{"turns":[]}}});
    load_history_from_start_or_resume(&mut app, &response.to_string())
        .expect("legacy history without an id");
    assert_eq!(app.session.thread_id(), "thread");
    app.assert_entry_invariants();
}
