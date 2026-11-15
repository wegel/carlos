//! Reject foreign history before it can replace the current conversation's rows or keys.

use crate::backend::BackendKind;
use crate::grok_backend::{translate_grok_notification_for_test, GrokTranslationState};
use serde_json::{json, Value};

use super::feed as handle_server_message_line;
use super::identity_support::{assert_entries, session_for as app};
use crate::session::item_history::load_history_from_start_or_resume;
use crate::session::state::SessionState;

fn foreign_history() -> String {
    json!({"result":{"thread":{"id":"other","name":"Foreign","turns":[
        {"id":"turn","items":[{"id":"x","type":"toolCall","name":"Read",
            "input":{"file_path":"a.rs"}}]}]}}})
    .to_string()
}

fn read(app: &mut SessionState, backend: BackendKind, path: &str) {
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
        app.set_summary_enabled_for_test(false);
        if existing {
            read(&mut app, backend, "old.rs");
            app.set_thread_name(Some("Current".into()));
            app.record_turn_started("active");
        }
        let session = format!("{:?}", app);
        let turn = app.codex_item_turn().clone();
        let ids: Vec<_> = app.messages().iter().map(|row| row.id).collect();
        let error = load_history_from_start_or_resume(&mut app, &foreign_history())
            .err()
            .expect("foreign history must be rejected");
        assert_eq!(error.to_string(), "History belongs to another thread");
        assert_eq!(format!("{:?}", app), session);
        assert_eq!(app.codex_item_turn(), &turn);
        read(&mut app, backend, "b.rs");
        assert_eq!(app.messages().len(), 1);
        let expected = if backend == BackendKind::Grok {
            "◇ Read File b.rs"
        } else {
            "→ Read b.rs"
        };
        assert_eq!(app.messages()[0].text, expected);
        if existing {
            assert_eq!(app.messages()[0].id, ids[0]);
        }
        assert_entries(&app);
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
    assert_eq!(app.thread_id(), "thread");
    assert_entries(&app);
}
